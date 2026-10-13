use pulse_core::platform::NativePlatform;
use pulse_core::storage::Storage;
use pulse_core::workflow::engine::CancelToken;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutStatus {
    pub requested: String,
    pub active: Option<String>,
    pub fallback_used: bool,
    pub error: Option<String>,
}

#[derive(Clone)]
pub struct AppState {
    pub storage: Arc<Storage>,
    pub platform: Arc<NativePlatform>,
    pub data_dir: PathBuf,
    pub cancel: Arc<Mutex<Option<CancelToken>>>,
    pub confirm: Arc<Mutex<Option<mpsc::Sender<bool>>>>,
    pub shortcut: Arc<Mutex<ShortcutStatus>>,
    pub log_path: PathBuf,
}

pub fn data_url(bytes: &[u8], mime: &str) -> String {
    format!("data:{mime};base64,{}", base64::Engine::encode(&base64::engine::general_purpose::STANDARD, bytes))
}

pub fn append_log(state: &AppState, line: &str) {
    use std::io::Write;
    let stamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    let text = pulse_core::security::redact(line);
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&state.log_path) {
        let _ = writeln!(f, "[{stamp}] {text}");
    }
}
