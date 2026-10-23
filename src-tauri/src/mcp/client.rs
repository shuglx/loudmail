use std::io::{BufRead, BufReader};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use reqwest::blocking::Client;
use serde_json::{json, Value};

use crate::logfile;
use crate::models::{HandshakeResult, MailItem, ServerInfo, StepResult, TestConnectionResult};

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
                Ok(SseEvent::Endpoint(endpoint)) => break endpoint,
                Ok(SseEvent::Closed(err)) => return Err(err),
                Ok(SseEvent::Message(_)) => continue,
                Err(_) => return Err("等待 SSE endpoint 超时".into()),
            }
        };

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
                    if let Some(err) = value.get("error") {
                        return Err(format!("MCP 返回错误: {err}"));
                    }
                    return Ok(value.get("result").cloned().unwrap_or(Value::Null));
                }
                Ok(SseEvent::Endpoint(_)) => continue,
                Ok(SseEvent::Closed(err)) => return Err(err),
                Err(_) => return Err(format!("MCP {method} 调用超时")),
            }
        }
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
                    name: "Mock".into(),
                    version: "0.1.0".into(),
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
