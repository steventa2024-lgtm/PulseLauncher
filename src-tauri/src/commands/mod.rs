//! Typed commands exposed to the interface. There is no general
//! "run this string" command.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{atomic::AtomicBool, Arc, Mutex};

use crate::models::HotkeyStatus;
use crate::storage::Db;

pub mod apps;
pub mod system;
pub mod workflows;

pub struct RunControl {
    pub cancel: AtomicBool,
    pub confirm: Mutex<Option<std::sync::mpsc::Sender<bool>>>,
}

pub struct AppState {
    pub db: Db,
    pub db_path: PathBuf,
    pub data_dir: PathBuf,
    pub icons: Mutex<HashMap<String, Option<String>>>,
    pub runs: Mutex<HashMap<String, Arc<RunControl>>>,
    pub hotkey: Mutex<HotkeyStatus>,
}

impl AppState {
    pub fn open(data_dir: PathBuf) -> Result<Self, crate::error::AppError> {
        let db_path = data_dir.join("pulselauncher.db");
        let db = Db::open(&db_path)?;
        db.ensure_system_collection("zeropulse", "ZeroPulse")?;
        db.seed_templates()?;
        Ok(Self {
            db,
            db_path,
            data_dir,
            icons: Mutex::new(HashMap::new()),
            runs: Mutex::new(HashMap::new()),
            hotkey: Mutex::new(HotkeyStatus {
                requested: String::new(),
                active: None,
                warning: None,
            }),
        })
    }
}
