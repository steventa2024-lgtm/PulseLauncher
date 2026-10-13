//! PulseLauncher core. This crate has no Tauri dependency so discovery,
//! storage, search, and the workflow engine can be tested on any host.

pub mod calculator;
pub mod discovery;
pub mod error;
pub mod files;
pub mod integrations;
pub mod launch;
pub mod lnk;
pub mod models;
pub mod platform;
pub mod search;
pub mod security;
pub mod storage;
pub mod workflows;

pub use error::{AppError, AppResult};
pub use models::*;
