use std::io::{BufRead, BufReader};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use reqwest::blocking::Client;
use serde_json::{json, Value};

use crate::logfile;
use crate::mcp::mock;
use crate::models::{
    DebugSearchResult, HandshakeResult, MailItem, ServerInfo, StepResult, TestConnectionResult,
};

enum SseEvent {
    Endpoint(String),
    Message(Value),
    Closed(String),
}

struct SseSession {
    rx: Receiver<SseEvent>,
    endpoint: String,
}

pub struct CoremailClient {
    http: Client,
    sse: Option<SseSession>,
    next_id: u64,
    server_info: Option<ServerInfo>,
}

impl CoremailClient {
    pub fn connect(host: &str, port: u16) -> Result<Self, String> {
        let base = format!("http://{host}:{port}");
        let sse_client = Client::builder()
            .connect_timeout(Duration::from_secs(2))
            .build()
            .map_err(|e| e.to_string())?;
        let response = sse_client
            .get(format!("{base}/sse"))
            .header("Accept", "text/event-stream")
            .header("Cache-Control", "no-cache")
            .send()
            .map_err(|e| format!("连接 SSE 失败: {e}"))?;
        if !response.status().is_success() {
            return Err(format!("SSE 返回 HTTP {}", response.status()));
        }

        let (tx, rx) = mpsc::channel();
        thread::spawn(move || read_sse(response, tx));

        let deadline = Instant::now() + Duration::from_secs(5);
        let endpoint = loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            match rx.recv_timeout(remaining) {
                Ok(SseEvent::Endpoint(raw)) => break resolve_endpoint(&base, &raw),
                Ok(SseEvent::Closed(err)) => return Err(err),
                Ok(SseEvent::Message(_)) => continue,
                Err(_) => return Err("等待 SSE endpoint 超时".into()),
            }
        };

        logfile::info(format!("[MCP] SSE endpoint: {endpoint}"));

        let http = Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .map_err(|e| e.to_string())?;
        let mut client = Self {
            http,
            sse: Some(SseSession { rx, endpoint }),
            next_id: 1,
            server_info: None,
        };
        client.initialize()?;
        Ok(client)
    }

    /// SSE 会话是否仍然可用。
    /// 服务端会按空闲把 SSE 连接回收，这是正常现象，调用方据此静默重建即可。
    pub fn session_alive(&mut self) -> bool {
        let Some(session) = self.sse.as_ref() else {
            return false;
        };
        match session.rx.try_recv() {
            Ok(SseEvent::Closed(_)) => false,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => false,
            _ => true,
        }
    }

    pub fn server_info(&self) -> Option<ServerInfo> {
        self.server_info.clone()
    }

    pub fn search(
        &mut self,
        pattern: &str,
        start_time: &str,
        end_time: &str,
        limit: u32,
    ) -> Result<Vec<MailItem>, String> {
        let id = self.next_id;
        self.next_id += 1;
        let params = json!({
            "name": "mail_search",
            "arguments": {
                "pattern": pattern,
                "start_time": start_time,
                "end_time": end_time,
                "limit": limit
            }
        });
        let result = self.call(id, "tools/call", params)?;
        if result.get("isError").and_then(Value::as_bool).unwrap_or(false) {
            return Err(format!("mail_search 返回错误: {result}"));
        }
        let mut mails = Vec::new();
        if let Some(content) = result.get("content").and_then(Value::as_array) {
            for item in content {
                if let Some(text) = item.get("text").and_then(Value::as_str) {
                    match serde_json::from_str::<MailItem>(text) {
                        Ok(mail) => mails.push(mail),
                        Err(e) => logfile::warn(format!("解析邮件对象失败: {e}")),
                    }
                }
            }
        }
        Ok(mails)
    }

    /// 调试用：发一次 mail_search，返回（发送报文, 返回报文）
    pub fn search_debug(
        &mut self,
        pattern: &str,
        start_time: &str,
        end_time: &str,
        limit: u32,
    ) -> Result<(String, String), String> {
        let id = self.next_id;
        self.next_id += 1;
        let params = mail_search_params(pattern, start_time, end_time, limit);
        let request = serde_json::to_string_pretty(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/call",
            "params": params
        }))
        .unwrap_or_default();
        let envelope = self.call_envelope(id, "tools/call", params)?;
        let response = serde_json::to_string_pretty(&envelope).unwrap_or_default();
        Ok((request, response))
    }

    fn initialize(&mut self) -> Result<(), String> {
        let params = json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "loudmail",
                "version": env!("CARGO_PKG_VERSION")
            }
        });
        let result = self.call(0, "initialize", params)?;
        if let Some(info) = result.get("serverInfo") {
            let name = info
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("CoremailAir")
                .to_string();
            let version = info
                .get("version")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_string();
            self.server_info = Some(ServerInfo { name, version });
        }
        self.notify("notifications/initialized")?;
        Ok(())
    }

    fn notify(&self, method: &str) -> Result<(), String> {
        let Some(session) = self.sse.as_ref() else {
            return Err("未建立 SSE 会话".into());
        };
        let body = json!({ "jsonrpc": "2.0", "method": method });
        self.http
            .post(&session.endpoint)
            .json(&body)
            .send()
            .map_err(|e| format!("发送通知失败: {e}"))?;
        Ok(())
    }

    fn call(&mut self, id: u64, method: &str, params: Value) -> Result<Value, String> {
        let envelope = self.call_envelope(id, method, params)?;
        if let Some(err) = envelope.get("error") {
            return Err(format!("MCP 返回错误: {err}"));
        }
        Ok(envelope.get("result").cloned().unwrap_or(Value::Null))
    }

    /// 返回完整的 JSON-RPC 报文（含 id/result/error），供调试展示用
    fn call_envelope(&mut self, id: u64, method: &str, params: Value) -> Result<Value, String> {
        // 先看看 SSE 会话是不是已经被服务端关闭（空闲回收），避免白等超时
        if let Some(session) = self.sse.as_ref() {
            match session.rx.try_recv() {
                Ok(SseEvent::Closed(err)) => return Err(err),
                Ok(SseEvent::Endpoint(_)) | Ok(SseEvent::Message(_)) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    return Err("SSE 会话已断开".into())
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
        }
        let body = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        });
        let http = self.http.clone();
        let session = self.sse.as_mut().ok_or("未建立 SSE 会话")?;
        let response = http
            .post(&session.endpoint)
            .json(&body)
            .send()
            .map_err(|e| format!("发送 MCP 请求失败: {e}"))?;
        if !response.status().is_success() {
            return Err(format!("MCP 请求返回 HTTP {}", response.status()));
        }

        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            match session.rx.recv_timeout(remaining) {
                Ok(SseEvent::Message(value)) => {
                    if value.get("id").and_then(Value::as_u64) != Some(id) {
                        continue;
                    }
                    return Ok(value);
                }
                Ok(SseEvent::Endpoint(_)) => continue,
                Ok(SseEvent::Closed(err)) => return Err(err),
                Err(_) => return Err(format!("MCP {method} 调用超时")),
            }
        }
    }
}

/// MCP 的 SSE 传输里，`endpoint` 事件给出的地址允许是相对路径
/// （例如 `/messages?sessionId=xxx`），必须基于 SSE 自身的地址解析成绝对地址。
/// 直接拿相对路径去 reqwest 请求会报 "builder error: relative URL without a base"。
fn resolve_endpoint(base: &str, endpoint: &str) -> String {
    let endpoint = endpoint.trim();
    if endpoint.starts_with("http://") || endpoint.starts_with("https://") {
        return endpoint.to_string();
    }
    let joined = url::Url::parse(base)
        .ok()
        .and_then(|base_url| base_url.join(endpoint).ok())
        .map(|url| url.to_string());
    match joined {
        Some(url) => url,
        None => format!(
            "{}/{}",
            base.trim_end_matches('/'),
            endpoint.trim_start_matches('/')
        ),
    }
}

fn read_sse(response: reqwest::blocking::Response, tx: Sender<SseEvent>) {
    let reader = BufReader::new(response);
    let mut event = String::new();
    let mut data = String::new();

    for line in reader.lines() {
        let line = match line {
            Ok(line) => line,
            Err(e) => {
                let _ = tx.send(SseEvent::Closed(format!("SSE 读取失败: {e}")));
                return;
            }
        };
        if line.is_empty() {
            if !data.is_empty() {
                if event == "endpoint" {
                    let _ = tx.send(SseEvent::Endpoint(data.clone()));
                } else {
                    match serde_json::from_str::<Value>(&data) {
                        Ok(value) => {
                            let _ = tx.send(SseEvent::Message(value));
                        }
                        Err(e) => logfile::warn(format!("解析 SSE 数据失败: {e}")),
                    }
                }
            }
            event.clear();
            data.clear();
        } else if let Some(rest) = line.strip_prefix("event:") {
            event = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("data:") {
            if !data.is_empty() {
                data.push('\n');
            }
            data.push_str(rest.trim_start());
        }
    }
    let _ = tx.send(SseEvent::Closed("SSE 连接已关闭".into()));
}

pub fn probe_tcp(host: &str, port: u16) -> Result<u64, String> {
    let started = Instant::now();
    let addrs = (host, port)
        .to_socket_addrs()
        .map_err(|e| format!("解析地址失败: {e}"))?;
    let mut last_error = String::from("没有可用地址");
    for addr in addrs {
        match TcpStream::connect_timeout(&addr, Duration::from_secs(1)) {
            Ok(_) => return Ok(started.elapsed().as_millis() as u64),
            Err(e) => last_error = e.to_string(),
        }
    }
    Err(last_error)
}

fn mail_search_params(pattern: &str, start_time: &str, end_time: &str, limit: u32) -> Value {
    json!({
        "name": "mail_search",
        "arguments": {
            "pattern": pattern,
            "start_time": start_time,
            "end_time": end_time,
            "limit": limit
        }
    })
}

/// 「测试接口」：真实连一次 MCP 并做一次 mail_search，把收发报文原样带回去
pub fn search_debug(
    host: &str,
    port: u16,
    pattern: &str,
    start_time: &str,
    end_time: &str,
    limit: u32,
    mock: bool,
) -> DebugSearchResult {
    let request = serde_json::to_string_pretty(&json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": mail_search_params(pattern, start_time, end_time, limit)
    }))
    .unwrap_or_default();

    if mock {
        let mut source = mock::MockSource::new();
        let response = match source.search(pattern, start_time, end_time, limit) {
            Ok(items) => {
                let content: Vec<Value> = items
                    .iter()
                    .map(|item| {
                        json!({
                            "type": "text",
                            "text": serde_json::to_string(item).unwrap_or_default()
                        })
                    })
                    .collect();
                json!({ "jsonrpc": "2.0", "id": 1, "result": { "content": content } })
            }
            Err(e) => json!({ "jsonrpc": "2.0", "id": 1, "error": { "message": e } }),
        };
        return DebugSearchResult {
            ok: response.get("error").is_none(),
            ms: 0,
            request,
            response: serde_json::to_string_pretty(&response).unwrap_or_default(),
            error: response
                .get("error")
                .and_then(|e| e.get("message"))
                .and_then(Value::as_str)
                .map(str::to_string),
        };
    }

    let started = Instant::now();
    let result = CoremailClient::connect(host, port).and_then(|mut client| {
        client.search_debug(pattern, start_time, end_time, limit)
    });
    let ms = started.elapsed().as_millis() as u64;
    match result {
        Ok((req, resp)) => DebugSearchResult {
            ok: true,
            ms,
            request: req,
            response: resp,
            error: None,
        },
        Err(e) => {
            logfile::error(format!("[测试接口] {e}"));
            DebugSearchResult {
                ok: false,
                ms,
                request,
                response: String::new(),
                error: Some(e),
            }
        }
    }
}

pub fn test_connection(host: &str, port: u16, mock: bool) -> TestConnectionResult {
    if mock {
        return TestConnectionResult {
            tcp: StepResult {
                ok: true,
                ms: 1,
                error: None,
            },
            handshake: HandshakeResult {
                ok: true,
                ms: 1,
                server_info: Some(ServerInfo {
                    name: "Mock MailBox".into(),
                    version: String::new(),
                }),
                error: None,
            },
            overall: true,
        };
    }

    let tcp = match probe_tcp(host, port) {
        Ok(ms) => StepResult {
            ok: true,
            ms,
            error: None,
        },
        Err(e) => StepResult {
            ok: false,
            ms: 0,
            error: Some(e),
        },
    };
    if !tcp.ok {
        return TestConnectionResult {
            tcp,
            handshake: HandshakeResult {
                ok: false,
                ms: 0,
                server_info: None,
                error: Some("TCP 不可达，未进行握手".into()),
            },
            overall: false,
        };
    }

    let started = Instant::now();
    match CoremailClient::connect(host, port) {
        Ok(client) => TestConnectionResult {
            tcp,
            handshake: HandshakeResult {
                ok: true,
                ms: started.elapsed().as_millis() as u64,
                server_info: client.server_info(),
                error: None,
            },
            overall: true,
        },
        Err(e) => TestConnectionResult {
            tcp,
            handshake: HandshakeResult {
                ok: false,
                ms: started.elapsed().as_millis() as u64,
                server_info: None,
                error: Some(e),
            },
            overall: false,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::resolve_endpoint;

    #[test]
    fn resolves_relative_endpoint_against_base() {
        let base = "http://127.0.0.1:11328";
        assert_eq!(
            resolve_endpoint(base, "/messages?sessionId=abc"),
            "http://127.0.0.1:11328/messages?sessionId=abc"
        );
        assert_eq!(
            resolve_endpoint(base, "messages?sessionId=abc"),
            "http://127.0.0.1:11328/messages?sessionId=abc"
        );
        assert_eq!(
            resolve_endpoint(base, "  /messages  "),
            "http://127.0.0.1:11328/messages"
        );
    }

    #[test]
    fn debug_search_builds_request_and_response_in_mock_mode() {
        let result = super::search_debug(
            "127.0.0.1",
            11328,
            "SOC",
            "20260101000000",
            "20260101235959",
            3,
            true,
        );
        assert!(result.ok, "mock 模式应当成功: {:?}", result.error);
        assert!(result.request.contains("\"mail_search\""));
        assert!(result.request.contains("\"limit\": 3"));
        assert!(result.response.contains("\"content\""));
    }

    #[test]
    fn keeps_absolute_endpoint() {
        assert_eq!(
            resolve_endpoint("http://127.0.0.1:11328", "http://10.0.0.5:11328/messages"),
            "http://10.0.0.5:11328/messages"
        );
    }
}
