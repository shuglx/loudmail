use chrono::Local;

use crate::models::MailItem;

/// 内存里最多保留的模拟邮件数，超出后丢弃最老的，避免长时间运行无限增长
const MAX_MAILS: usize = 500;
/// 模拟邮件 mailId 的起始序号下限
pub const BASE_SEQ: u64 = 21598;

/// 模拟邮箱：每轮轮询追加一封 SOC 告警，便于联调「匹配 → 播报 → 确认」整条链路
pub struct MockSource {
    mails: Vec<MailItem>,
    next_seq: u64,
}

impl MockSource {
    /// `start_seq` 应传入「现有最大游标 + 1」：序号的绝对值不重要，但必须大于游标，
    /// 否则重启后序号从头开始，新邮件会被当成「已处理过的旧邮件」全部跳过。
    /// 构造时不预置邮件，调用方每轮 `tick()` 一次即可保证「一轮一轮」。
    pub fn new(start_seq: u64) -> Self {
        Self {
            mails: Vec::new(),
            next_seq: start_seq.max(BASE_SEQ),
        }
    }

    /// 每轮轮询调用一次：生成一封新的 SOC 告警
    pub fn tick(&mut self) {
        self.emit_soc();
    }

    pub fn search(
        &self,
        pattern: &str,
        _start_time: &str,
        _end_time: &str,
        limit: u32,
    ) -> Result<Vec<MailItem>, String> {
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
        // 只返回最新的 limit 条：老邮件对游标没有意义，也不能把新邮件挤掉
        let limit = limit.max(1) as usize;
        if matched.len() > limit {
            let start = matched.len() - limit;
            matched.drain(..start);
        }
        Ok(matched)
    }

    fn emit_soc(&mut self) {
        let seq = self.next_seq;
        self.next_seq += 1;
        let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        self.mails.push(MailItem {
            mail_id: format!("mock:{seq}"),
            sender: "\"aqyyptsoc\" <aqyyptsoc@dc.xxxx.com.cn>".into(),
            subject: format!("【5级】SOC_安全告警_HIPS_10.9.{}.{}", seq % 200, seq % 90),
            summary: format!("5级告警处置要求：15分钟内通知到应用维护部门。模拟时间 {now}"),
        });
        if self.mails.len() > MAX_MAILS {
            let overflow = self.mails.len() - MAX_MAILS;
            self.mails.drain(..overflow);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{MockSource, BASE_SEQ};

    #[test]
    fn starts_after_the_given_cursor() {
        let mut source = MockSource::new(900_000);
        source.tick();
        let items = source.search("SOC", "", "", 10).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].mail_id, "mock:900000");
    }

    #[test]
    fn never_starts_below_base_seq() {
        let mut source = MockSource::new(10);
        source.tick();
        let items = source.search("SOC", "", "", 10).unwrap();
        assert_eq!(items[0].mail_id, format!("mock:{BASE_SEQ}"));
    }

    #[test]
    fn one_tick_produces_exactly_one_mail() {
        let mut source = MockSource::new(900_000);
        assert!(source.search("SOC", "", "", 10).unwrap().is_empty());
        source.tick();
        let items = source.search("SOC_安全告警", "", "", 10).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].mail_id, "mock:900000");
        assert!(items[0].sender.contains("aqyyptsoc@dc.xxxx.com.cn"));
        assert!(items[0].subject.contains("SOC_安全告警"));
    }

    #[test]
    fn search_returns_the_newest_items() {
        let mut source = MockSource::new(900_000);
        for _ in 0..5 {
            source.tick();
        }
        let items = source.search("SOC", "", "", 3).unwrap();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].mail_id, "mock:900002");
        assert_eq!(items[2].mail_id, "mock:900004");
    }
}
