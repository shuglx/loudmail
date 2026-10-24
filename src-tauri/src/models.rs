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
    /// 语音来源：local（本地模型） / edge（微软在线语音）
    #[serde(default = "default_tts_provider")]
    pub provider: String,
    /// 在线语音音色（edge 模式使用）
    #[serde(default = "default_edge_voice")]
    pub edge_voice: String,
    /// 外置模型目录（包含 .onnx / tokens.txt / espeak-ng-data）
    pub model_dir: String,
    pub speed: f32,
}

fn default_tts_provider() -> String {
    "local".into()
}

fn default_edge_voice() -> String {
    "zh-CN-XiaoxiaoNeural".into()
}

impl Default for TtsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: default_tts_provider(),
            edge_voice: default_edge_voice(),
            model_dir: String::new(),
            speed: 1.0,
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

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Strategy {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub sender: String,
    pub keyword: String,
    #[serde(default = "default_true")]
    pub filter_replies: bool,
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
    #[serde(default = "default_true")]
    pub filter_replies: bool,
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
    #[serde(default = "default_true")]
    pub filter_replies: bool,
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
