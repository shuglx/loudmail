use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub version: u32,
    pub mcp: McpConfig,
    pub scan: ScanConfig,
    pub behavior: BehaviorConfig,
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
                lookback_min: 60,
            },
            behavior: BehaviorConfig {
                autostart: false,
                mock: false,
            },
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
