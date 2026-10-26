use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use chrono::{Duration as ChronoDuration, Local};

use crate::logfile;
use crate::mcp::mock::MockSource;
use crate::mcp::CoremailClient;
use crate::models::*;
use crate::state::AppState;
use crate::store;

pub fn start(state: Arc<AppState>) {
    thread::spawn(move || {
        let mut client: Option<CoremailClient> = None;
        let mut mock: Option<MockSource> = None;
        let mut config_version = state.config_version.load(Ordering::Relaxed);
        // 连续失败次数：轮询单次失败只重连重试，不立刻把状态打红
        let mut failures: u32 = 0;
        // 记录上次的「模拟邮箱」开关，切换时立刻给出对应状态
        let mut last_mock: Option<bool> = None;
        let mut last_scan = Instant::now()
            .checked_sub(Duration::from_secs(3600))
            .unwrap_or_else(Instant::now);

        loop {
            thread::sleep(Duration::from_millis(500));
            if state.paused.load(Ordering::Relaxed) {
                continue;
            }
            let version = state.config_version.load(Ordering::Relaxed);
            if version != config_version {
                config_version = version;
                client = None;
                last_scan = Instant::now()
                    .checked_sub(Duration::from_secs(3600))
                    .unwrap_or_else(Instant::now);
            }
            let config = match state.config.read() {
                Ok(c) => c.clone(),
                Err(_) => continue,
            };
            if last_mock != Some(config.behavior.mock) {
                last_mock = Some(config.behavior.mock);
                if config.behavior.mock {
                    state.set_runtime(RuntimeStatus {
                        connected: true,
                        server_name: Some("Mock MailBox".into()),
                        server_version: None,
                        last_error: None,
                        mock: true,
                    });
                }
            }
            if last_scan.elapsed() < Duration::from_secs(config.scan.interval_sec.max(1)) {
                continue;
            }
            last_scan = Instant::now();
            match scan(&state, &config, &mut client, &mut mock) {
                Ok(()) => {
                    failures = 0;
                }
                Err(e) => {
                    failures += 1;
                    logfile::error(format!("扫描失败({failures}): {e}"));
                    // 单次失败先不动状态（多半是 SSE 被服务端回收、重连即可自愈），
                    // 连续两次都失败才认为不可达
                    if failures >= 2 {
                        state.mark_disconnected(Some(e));
                    }
                }
            }
        }
    });
}

fn scan(
    state: &AppState,
    config: &AppConfig,
    client: &mut Option<CoremailClient>,
    mock: &mut Option<MockSource>,
) -> Result<(), String> {
    let enabled: Vec<Strategy> = state
        .strategies
        .read()
        .map_err(|e| e.to_string())?
        .iter()
        .filter(|s| s.enabled)
        .cloned()
        .collect();
    if enabled.is_empty() {
        return Ok(());
    }

    let end = Local::now();
    let start = end - ChronoDuration::minutes(config.scan.lookback_min as i64);
    let start_time = start.format("%Y%m%d%H%M%S").to_string();
    let end_time = end.format("%Y%m%d%H%M%S").to_string();
    // 恢复播报后的首轮扫描：只更新游标，不补播暂停期间的邮件
    let skip_backlog = state.skip_backlog.swap(false, Ordering::Relaxed);

    if config.behavior.mock {
        let source = mock.get_or_insert_with(MockSource::new);
        state.set_runtime(RuntimeStatus {
            connected: true,
            server_name: Some("Mock MailBox".into()),
            server_version: None,
            last_error: None,
            mock: true,
        });
        for strategy in &enabled {
            match source.search(&strategy.keyword, &start_time, &end_time, 200) {
                Ok(items) => process_strategy(state, strategy, items, skip_backlog),
                Err(e) => logfile::error(format!("模拟数据查询失败: {e}")),
            }
        }
        state.save_strategies()?;
        return Ok(());
    }

    if client.is_none() {
        match CoremailClient::connect(&config.mcp.host, config.mcp.port) {
            Ok(c) => *client = Some(c),
            Err(e) => {
                logfile::error(format!("连接 e企邮 MCP 失败: {e}"));
                return Err(format!("连接 e企邮 MCP 失败: {e}"));
            }
        }
    }

    let info = client.as_ref().and_then(|c| c.server_info());
    state.set_runtime(RuntimeStatus {
        connected: true,
        server_name: info.as_ref().map(|i| i.name.clone()),
        server_version: info.as_ref().map(|i| i.version.clone()),
        last_error: None,
        mock: false,
    });

    let mut failure: Option<String> = None;
    for strategy in &enabled {
        let mut result = client
            .as_mut()
            .ok_or_else(|| "MCP 客户端未连接".to_string())
            .and_then(|c| c.search(&strategy.keyword, &start_time, &end_time, 200));

        // SSE 长连接会被服务端按空闲回收：失败时重连一次再重试，
        // 避免一次瞬时断流就把状态打回"不可达"
        if result.is_err() {
            logfile::warn(format!(
                "MCP 搜索失败，重连后重试: {:?}",
                result.as_ref().err()
            ));
            *client = None;
            match CoremailClient::connect(&config.mcp.host, config.mcp.port) {
                Ok(c) => {
                    *client = Some(c);
                    result = client
                        .as_mut()
                        .ok_or_else(|| "MCP 客户端未连接".to_string())
                        .and_then(|c| c.search(&strategy.keyword, &start_time, &end_time, 200));
                }
                Err(e) => {
                    failure = Some(format!("重连失败: {e}"));
                    break;
                }
            }
        }

        match result {
            Ok(items) => process_strategy(state, strategy, items, skip_backlog),
            Err(e) => {
                failure = Some(e);
                break;
            }
        }
    }
    state.save_strategies()?;

    if let Some(e) = failure {
        *client = None;
        logfile::error(format!("MCP 搜索失败: {e}"));
        return Err(e);
    }
    Ok(())
}

fn process_strategy(state: &AppState, strategy: &Strategy, items: Vec<MailItem>, skip_backlog: bool) {
    let max_seq = items
        .iter()
        .filter_map(|m| m.seq())
        .max()
        .unwrap_or(0);

    if skip_backlog {
        let next = max_seq.max(strategy.cursor_seq.unwrap_or(0));
        set_cursor(state, &strategy.id, next);
        logfile::info(format!(
            "策略「{}」恢复播报，跳过暂停期间的邮件，游标推进到 {}",
            strategy.name, next
        ));
        return;
    }

    let cursor = match strategy.cursor_seq {
        Some(seq) => seq,
        None => {
            set_cursor(state, &strategy.id, max_seq);
            logfile::info(format!(
                "策略「{}」初始化游标为 {}",
                strategy.name, max_seq
            ));
            return;
        }
    };

    let sender_expected = strategy.sender.trim().to_lowercase();
    let matcher = build_matcher(strategy);
    let mut candidates: Vec<MailItem> = items
        .iter()
        .filter(|mail| {
            let seq = mail.seq().unwrap_or(0);
            if seq <= cursor {
                return false;
            }
            // 发件人必填，按「包含」模糊匹配（地址已从 <...> 提取并转小写）
            if !sender_expected.is_empty() && !mail.sender_email().contains(&sender_expected) {
                return false;
            }
            if strategy.filter_replies && is_reply_or_forward(&mail.subject) {
                return false;
            }
            matcher.is_match(&mail.subject)
        })
        .cloned()
        .collect();
    candidates.sort_by_key(|m| m.seq().unwrap_or(0));

    let sound = strategy
        .sound_id
        .as_ref()
        .and_then(|id| state.sound(id));
    let sound_path = sound
        .as_ref()
        .map(|s| state.paths.sounds_dir.join(&s.file));

    for mail in candidates {
        let mail_id = mail.mail_id.clone();
        if state
            .seen
            .lock()
            .map(|seen| seen.contains(&mail_id))
            .unwrap_or(false)
        {
            continue;
        }
        let Some(sound) = sound.as_ref() else {
            logfile::warn(format!(
                "策略「{}」缺少语音，跳过邮件 {}",
                strategy.name, mail_id
            ));
            continue;
        };
        let Some(path) = sound_path.as_ref() else {
            continue;
        };
        if !path.exists() {
            logfile::error(format!("语音文件不存在: {}", path.display()));
            continue;
        }

        let play_count = strategy.play_count.clamp(1, 3);
        state.player.play(path.clone(), play_count);

        let record = HistoryRecord {
            id: store::new_id("his"),
            time: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            strategy_id: strategy.id.clone(),
            strategy_name: strategy.name.clone(),
            mail_id: mail_id.clone(),
            sender: mail.sender.clone(),
            subject: mail.subject.clone(),
            summary: mail.summary.clone(),
            sound_name: sound.name.clone(),
            play_count,
        };
        if let Ok(mut history) = state.history.write() {
            history.insert(0, record.clone());
            history.truncate(500);
        }
        if let Ok(mut seen) = state.seen.lock() {
            seen.insert(mail_id);
        }
        let _ = state.save_history();
        state.emit_history(&record);
        logfile::info(format!(
            "命中播报: [{}] {} ({})",
            strategy.name, record.subject, record.mail_id
        ));
    }

    set_cursor(state, &strategy.id, max_seq.max(cursor));
}

fn set_cursor(state: &AppState, strategy_id: &str, seq: u64) {
    if let Ok(mut strategies) = state.strategies.write() {
        if let Some(item) = strategies.iter_mut().find(|s| s.id == strategy_id) {
            item.cursor_seq = Some(seq);
            item.updated_at = store::now_iso();
        }
    }
}

/// 标题匹配器：模糊（包含）/ 以此开头 / 以此结尾（均为忽略大小写、标题先 trim）
enum Matcher {
    Contains(String),
    Prefix(String),
    Suffix(String),
}

impl Matcher {
    fn is_match(&self, subject: &str) -> bool {
        let subject = subject.trim().to_lowercase();
        match self {
            Matcher::Contains(keyword) => subject.contains(keyword),
            Matcher::Prefix(keyword) => subject.starts_with(keyword),
            Matcher::Suffix(keyword) => subject.ends_with(keyword),
        }
    }
}

fn build_matcher(strategy: &Strategy) -> Matcher {
    let keyword = strategy.keyword.trim().to_lowercase();
    match strategy.match_mode.as_str() {
        "prefix" => Matcher::Prefix(keyword),
        "suffix" => Matcher::Suffix(keyword),
        _ => Matcher::Contains(keyword),
    }
}

/// 标题是否以「转发 / 回复 + 中英文冒号」开头（允许前导空白、冒号前空格）
fn is_reply_or_forward(subject: &str) -> bool {
    let head = subject.trim_start();
    for prefix in ["转发", "回复"] {
        if let Some(rest) = head.strip_prefix(prefix) {
            let rest = rest.trim_start();
            if rest.starts_with(':') || rest.starts_with('：') {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::{is_reply_or_forward, Matcher};

    #[test]
    fn detects_reply_and_forward_prefixes() {
        assert!(is_reply_or_forward("转发: SOC_安全告警"));
        assert!(is_reply_or_forward("转发：SOC_安全告警"));
        assert!(is_reply_or_forward("回复:Re: SOC_安全告警"));
        assert!(is_reply_or_forward("回复：SOC_安全告警"));
        assert!(is_reply_or_forward("  转发 : SOC_安全告警"));
        assert!(is_reply_or_forward("转发：“SOC_安全告警”"));
        assert!(!is_reply_or_forward("转发SOC_安全告警"));
        assert!(!is_reply_or_forward("【转发】SOC_安全告警"));
        assert!(!is_reply_or_forward("Re: SOC_安全告警"));
        assert!(!is_reply_or_forward("【5级】SOC_安全告警"));
    }

    #[test]
    fn matches_subject_by_mode() {
        let keyword = "soc_安全告警";
        let contains = Matcher::Contains(keyword.into());
        assert!(contains.is_match("【5级】SOC_安全告警_HIPS"));
        assert!(contains.is_match("  SOC_安全告警  "));
        assert!(!contains.is_match("DB_ALERT_核心库连接数超过阈值"));

        let prefix = Matcher::Prefix(keyword.into());
        assert!(prefix.is_match("SOC_安全告警_HIPS"));
        assert!(prefix.is_match("  SOC_安全告警  "));
        assert!(!prefix.is_match("【5级】SOC_安全告警_HIPS"));

        let suffix = Matcher::Suffix(keyword.into());
        assert!(suffix.is_match("主机 SOC_安全告警  "));
        assert!(!suffix.is_match("SOC_安全告警_HIPS"));
    }
}
