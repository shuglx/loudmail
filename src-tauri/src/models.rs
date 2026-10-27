use serde::{Deserialize, Serialize};

fn default_true() -> bool {
    true
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub version: u32,
    pub mcp: McpConfig,
    pub scan: ScanConfig,
    pub behavior: BehaviorConfig,
    #[serde(default)]
    pub tts: TtsConfig,
    /// 界面主题：light / dark
    #[serde(default = "default_theme")]
    pub theme: String,
}

fn default_theme() -> String {
    "light".into()
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TtsConfig {
    pub enabled: bool,
    /// 语音来源：edge（微软在线语音，需联网） / intranet（内网 TTS 服务）
    #[serde(default = "default_tts_provider")]
    pub provider: String,
    /// 在线语音音色（edge 模式使用）
    #[serde(default = "default_edge_voice")]
    pub edge_voice: String,
    /// 语速倍率（内网模式写入 session.config.speed）
    #[serde(default = "default_tts_speed")]
    pub speed: f32,
    /// 内网 TTS 服务地址（ws://.../v1/audio/speech/stream）
    #[serde(default = "default_intranet_url")]
    pub intranet_url: String,
    /// 内网 TTS 认证信息（可空，非空时作为 Bearer Token）
    #[serde(default)]
    pub intranet_key: String,
    /// 内网音色：vivian（女，默认） / ryan（男）
    #[serde(default = "default_intranet_voice")]
    pub intranet_voice: String,
    /// 内网语气约束词，对正式感影响很大
    #[serde(default = "default_intranet_instructions")]
    pub intranet_instructions: String,
}

fn default_tts_provider() -> String {
    "edge".into()
}

fn default_edge_voice() -> String {
    "zh-CN-XiaoxiaoNeural".into()
}

fn default_tts_speed() -> f32 {
    1.0
}

fn default_intranet_url() -> String {
    "ws://ip:port/v1/audio/speech/stream".into()
}

fn default_intranet_voice() -> String {
    "vivian".into()
}

fn default_intranet_instructions() -> String {
    "播音腔，声音正式，精神饱满，情绪稳定".into()
}

impl Default for TtsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: default_tts_provider(),
            edge_voice: default_edge_voice(),
            speed: default_tts_speed(),
            intranet_url: default_intranet_url(),
            intranet_key: String::new(),
            intranet_voice: default_intranet_voice(),
            intranet_instructions: default_intranet_instructions(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct McpConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ScanConfig {
    pub interval_sec: u64,
    pub lookback_min: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BehaviorConfig {
    pub autostart: bool,
    pub mock: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: 1,
            mcp: McpConfig {
                host: "127.0.0.1".into(),
                port: 11328,
            },
            scan: ScanConfig {
                interval_sec: 60,
                lookback_min: 10,
            },
            behavior: BehaviorConfig {
                autostart: false,
                mock: false,
            },
            tts: TtsConfig::default(),
            theme: default_theme(),
        }
    }
}

/// 标题关键词匹配方式：contains（模糊）/ prefix（以此开头）/ suffix（以此结尾）
fn default_match_mode() -> String {
    "contains".into()
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Strategy {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub sender: String,
    pub keyword: String,
    #[serde(default = "default_match_mode")]
    pub match_mode: String,
    #[serde(default = "default_true")]
    pub filter_replies: bool,
    /// 人工确认：命中后需在播报记录中确认，未确认时每轮轮询重复播报
    #[serde(default)]
    pub require_confirm: bool,
    pub sound_id: Option<String>,
    pub play_count: u8,
    pub cursor_seq: Option<u64>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct StrategyInput {
    pub name: String,
    pub sender: String,
    pub keyword: String,
    #[serde(default = "default_match_mode")]
    pub match_mode: String,
    #[serde(default = "default_true")]
    pub filter_replies: bool,
    #[serde(default)]
    pub require_confirm: bool,
    pub sound_id: Option<String>,
    pub play_count: u8,
    pub enabled: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct StrategiesFile {
    pub version: u32,
    pub items: Vec<Strategy>,
}

fn default_play_count() -> u8 {
    1
}

/// 导出/导入用的策略条目（不含本地 id、游标等运行时字段）
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct StrategyExportItem {
    pub name: String,
    #[serde(default)]
    pub sender: String,
    pub keyword: String,
    #[serde(default = "default_match_mode")]
    pub match_mode: String,
    #[serde(default = "default_true")]
    pub filter_replies: bool,
    #[serde(default)]
    pub require_confirm: bool,
    #[serde(default)]
    pub sound_id: Option<String>,
    /// 仅便于人工查看，导入时不作为依据
    #[serde(default)]
    pub sound_name: Option<String>,
    #[serde(default = "default_play_count")]
    pub play_count: u8,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_export_version() -> u32 {
    1
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct StrategyExportFile {
    #[serde(default)]
    pub kind: String,
    #[serde(default = "default_export_version")]
    pub version: u32,
    #[serde(default)]
    pub exported_at: String,
    #[serde(default)]
    pub items: Vec<StrategyExportItem>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct StrategyImportPreview {
    pub path: String,
    pub count: usize,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct StrategyImportResult {
    /// 新增的策略条数
    pub added: usize,
    /// 按名称覆盖已有策略的条数
    pub updated: usize,
    /// 因名称或关键词为空而跳过的条数
    pub skipped: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct HistoryRecord {
    pub id: String,
    pub time: String,
    pub strategy_id: String,
    pub strategy_name: String,
    pub mail_id: String,
    pub sender: String,
    pub subject: String,
    #[serde(default)]
    pub summary: String,
    pub sound_name: String,
    pub play_count: u8,
    /// 命中时策略是否开启「人工确认」（快照：之后修改策略不影响已有记录）
    #[serde(default)]
    pub require_confirm: bool,
    /// 是否已人工确认
    #[serde(default)]
    pub confirmed: bool,
    #[serde(default)]
    pub confirmed_at: Option<String>,
    /// 命中时的语音 ID，供未确认记录重复播报时定位音频
    #[serde(default)]
    pub sound_id: Option<String>,
    /// 已重复播报的次数（0 表示还在第一时间窗口内）
    #[serde(default)]
    pub rebroadcast_count: u32,
    #[serde(default)]
    pub last_broadcast_at: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct HistoryFile {
    pub version: u32,
    pub items: Vec<HistoryRecord>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SoundItem {
    pub id: String,
    pub name: String,
    pub file: String,
    pub duration_ms: u64,
    pub source: String,
    pub created_at: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct SoundsFile {
    pub version: u32,
    pub items: Vec<SoundItem>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MailItem {
    #[serde(alias = "mailId")]
    pub mail_id: String,
    pub sender: String,
    pub subject: String,
    pub summary: String,
}

impl MailItem {
    pub fn seq(&self) -> Option<u64> {
        self.mail_id.rsplit(':').next()?.parse::<u64>().ok()
    }

    pub fn sender_email(&self) -> String {
        sender_email(&self.sender)
    }
}

pub fn sender_email(raw: &str) -> String {
    let value = raw.trim();
    if let (Some(start), Some(end)) = (value.rfind('<'), value.rfind('>')) {
        if end > start {
            return value[start + 1..end].trim().to_lowercase();
        }
    }
    value.to_lowercase()
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStatus {
    pub connected: bool,
    pub server_name: Option<String>,
    pub server_version: Option<String>,
    pub last_error: Option<String>,
    pub mock: bool,
}

impl Default for RuntimeStatus {
    fn default() -> Self {
        Self {
            connected: false,
            server_name: None,
            server_version: None,
            last_error: None,
            mock: false,
        }
    }
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct StepResult {
    pub ok: bool,
    pub ms: u64,
    pub error: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct HandshakeResult {
    pub ok: bool,
    pub ms: u64,
    pub server_info: Option<ServerInfo>,
    pub error: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TestConnectionResult {
    pub tcp: StepResult,
    pub handshake: HandshakeResult,
    pub overall: bool,
}

/// 「测试接口」的调试结果：直接展示收发报文
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DebugSearchResult {
    pub ok: bool,
    pub ms: u64,
    pub request: String,
    pub response: String,
    pub error: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PickedFile {
    pub path: String,
    pub name: String,
    pub size: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RecordingResult {
    pub path: String,
    pub duration_ms: u64,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct StorageFile {
    pub label: String,
    pub path: String,
    pub kind: String,
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackStatus {
    pub playing: bool,
    /// 队列中还未开始播放的条数
    pub pending: usize,
    pub previewing: bool,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TtsStatus {
    pub ready: bool,
    pub message: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub tauri_version: String,
    pub data_dir: String,
    pub storage_files: Vec<StorageFile>,
    pub log_path: String,
}

#[cfg(test)]
mod tests {
    use super::{HistoryRecord, Strategy};

    /// 老版本数据文件没有新字段，必须能按默认值读出来（否则会被备份重建、记录全丢）
    #[test]
    fn legacy_records_keep_working() {
        let raw = r#"{
            "id": "his_1",
            "time": "2026-10-01 10:00:00",
            "strategyId": "stg_1",
            "strategyName": "SOC",
            "mailId": "mock:1",
            "sender": "a@b.com",
            "subject": "测试",
            "summary": "",
            "soundName": "普通告警音",
            "playCount": 1
        }"#;
        let record: HistoryRecord = serde_json::from_str(raw).unwrap();
        assert!(!record.require_confirm);
        assert!(!record.confirmed);
        assert!(record.confirmed_at.is_none());
        assert!(record.sound_id.is_none());
        assert_eq!(record.rebroadcast_count, 0);
        assert!(record.last_broadcast_at.is_none());
    }

    #[test]
    fn legacy_strategies_default_to_no_confirm() {
        let raw = r#"{
            "id": "stg_1",
            "name": "SOC",
            "enabled": true,
            "sender": "a@b.com",
            "keyword": "SOC",
            "matchMode": "contains",
            "filterReplies": true,
            "soundId": "普通告警音",
            "playCount": 1,
            "cursorSeq": null,
            "createdAt": "2026-10-01T10:00:00+08:00",
            "updatedAt": "2026-10-01T10:00:00+08:00"
        }"#;
        let strategy: Strategy = serde_json::from_str(raw).unwrap();
        assert!(!strategy.require_confirm);
    }
}
