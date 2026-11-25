use chrono::Local;

use crate::models::MailItem;

/// 内存里最多保留的模拟邮件数，超出后丢弃最老的，避免长时间运行无限增长
const MAX_MAILS: usize = 500;
/// 模拟邮件 mailId 的起始序号下限
pub const BASE_SEQ: u64 = 21598;
/// 模拟邮件的序号空间前缀：与真实邮箱的序号各自独立比较
pub const SOURCE: &str = "mock";
/// 每轮轮询生成的邮件条数：同一发件人 + 同一标题，用于验证「合并同名的高频邮件」
pub const BATCH: u64 = 2;
/// 模拟策略使用的固定发件人与标题关键词：与真实业务邮件刻意区分，避免误命中
pub const EMAIL: &str = "mock@loudmail.dev";
pub const SENDER: &str = "\"LoudMail Mock\" <mock@loudmail.dev>";
pub const KEYWORD: &str = "[MOCK] 模拟测试告警";

/// 模拟邮箱：每轮轮询追加 2 封发件人、标题完全相同的模拟告警，
/// 便于联调「匹配 → 合并同名高频邮件 → 播报 → 确认」整条链路
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

    /// 每轮轮询调用一次：生成一批同一发件人、同一标题的 SOC 告警
    pub fn tick(&mut self) {
        self.emit_soc_batch();
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

    fn emit_soc_batch(&mut self) {
        let seq = self.next_seq;
        self.next_seq += BATCH;
        let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        // 同一批次共用一个标题（只带批次的起始序号），发件人也相同：
        // 「合并同名的高频邮件」开启时，这一批只会产生一条播报记录
        let subject = format!("{KEYWORD} #{seq}");
        let summary = format!("这是 LoudMail 模拟数据，不来自真实邮箱。模拟时间 {now}");
        for offset in 0..BATCH {
            self.mails.push(MailItem {
                mail_id: format!("{SOURCE}:{}", seq + offset),
                sender: SENDER.into(),
                subject: subject.clone(),
                summary: summary.clone(),
            });
        }
        if self.mails.len() > MAX_MAILS {
            let overflow = self.mails.len() - MAX_MAILS;
            self.mails.drain(..overflow);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{MockSource, BATCH, BASE_SEQ, KEYWORD, SOURCE};

    fn search(source: &MockSource, pattern: &str, limit: u32) -> Vec<super::MailItem> {
        source.search(pattern, "", "", limit).unwrap()
    }

    #[test]
    fn starts_after_the_given_cursor() {
        let mut source = MockSource::new(900_000);
        source.tick();
        let items = search(&source, KEYWORD, 10);
        assert_eq!(items.len(), BATCH as usize);
        assert_eq!(items[0].mail_id, format!("{SOURCE}:900000"));
    }

    #[test]
    fn never_starts_below_base_seq() {
        let mut source = MockSource::new(10);
        source.tick();
        let items = search(&source, KEYWORD, 10);
        assert_eq!(items[0].mail_id, format!("{SOURCE}:{BASE_SEQ}"));
    }

    /// 每轮生成的一批邮件发件人、标题完全相同，序号各自唯一（用于合并播报）
    #[test]
    fn one_tick_produces_a_batch_of_identical_mails() {
        let mut source = MockSource::new(900_000);
        assert!(search(&source, KEYWORD, 10).is_empty());
        source.tick();
        let items = search(&source, KEYWORD, 10);
        assert_eq!(items.len(), BATCH as usize);
        assert_eq!(items[0].mail_id, format!("{SOURCE}:900000"));
        assert_eq!(items[1].mail_id, format!("{SOURCE}:900001"));
        assert!(items[0].sender.contains("mock@loudmail.dev"));
        assert!(items[0].subject.contains(KEYWORD));
        assert_eq!(items[0].sender, items[1].sender);
        assert_eq!(items[0].subject, items[1].subject);
    }

    #[test]
    fn search_returns_the_newest_items() {
        let mut source = MockSource::new(900_000);
        for _ in 0..5 {
            source.tick();
        }
        let items = search(&source, KEYWORD, 3);
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].mail_id, format!("{SOURCE}:900007"));
        assert_eq!(items[2].mail_id, format!("{SOURCE}:900009"));
    }
}
