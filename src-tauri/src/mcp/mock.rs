use std::time::{Duration, Instant};

use chrono::Local;

use crate::models::MailItem;

pub struct MockSource {
    mails: Vec<MailItem>,
    next_seq: u64,
    last_emit: Instant,
}

impl MockSource {
    pub fn new() -> Self {
        let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        Self {
            mails: vec![
                MailItem {
                    mail_id: "mock:21598".into(),
                    sender: "\"aqyyptsoc\" <aqyyptsoc@dc.icbc.com.cn>".into(),
                    subject: format!("【5级】SOC_安全告警_HIPS_10.9.18.20"),
                    summary: format!("模拟告警邮件，生成时间 {now}"),
                },
                MailItem {
                    mail_id: "mock:21599".into(),
                    sender: "\"DBA Alert\" <dba-alert@dc.icbc.com.cn>".into(),
                    subject: "DB_ALERT_核心库连接数超过阈值_10.9.32.14".into(),
                    summary: format!("模拟告警邮件，生成时间 {now}"),
                },
            ],
            next_seq: 21600,
            last_emit: Instant::now(),
        }
    }

    pub fn search(
        &mut self,
        pattern: &str,
        _start_time: &str,
        _end_time: &str,
        limit: u32,
    ) -> Result<Vec<MailItem>, String> {
        self.maybe_emit();
        let needle = pattern.to_lowercase();
        let mut matched: Vec<MailItem> = self
            .mails
            .iter()
            .filter(|m| {
                m.subject.to_lowercase().contains(&needle)
                    || m.sender.to_lowercase().contains(&needle)
                    || m.summary.to_lowercase().contains(&needle)
            })
            .cloned()
            .collect();
        matched.sort_by_key(|m| m.seq().unwrap_or(0));
        matched.truncate(limit.max(1) as usize);
        Ok(matched)
    }

    fn maybe_emit(&mut self) {
        if self.last_emit.elapsed() < Duration::from_secs(8) {
            return;
        }
        self.last_emit = Instant::now();
        let seq = self.next_seq;
        self.next_seq += 1;
        let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let (sender, subject, summary) = match seq % 3 {
            0 => (
                "\"aqyyptsoc\" <aqyyptsoc@dc.icbc.com.cn>",
                format!("【5级】SOC_安全告警_HIPS_10.9.{}.{}", seq % 200, seq % 90),
                format!("5级告警处置要求：15分钟内通知到应用维护部门。模拟时间 {now}"),
            ),
            1 => (
                "\"DBA Alert\" <dba-alert@dc.icbc.com.cn>",
                format!("DB_ALERT_连接数异常_节点_{}", seq % 20),
                format!("数据库连接数超过阈值。模拟时间 {now}"),
            ),
            _ => (
                "\"Ops Center\" <ops@dc.icbc.com.cn>",
                format!("变更窗口_核心交换机升级_{}", seq % 50),
                format!("变更窗口已开启。模拟时间 {now}"),
            ),
        };
        self.mails.push(MailItem {
            mail_id: format!("mock:{seq}"),
            sender: sender.into(),
            subject,
            summary,
        });
    }
}
