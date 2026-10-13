//! PulseLauncher core: everything that does not depend on the UI shell.
//!
//! * `storage` - SQLite persistence with versioned migrations
//! * `discovery` / `lnk` - Start Menu shortcut discovery and `.lnk` parsing
//! * `search` / `calc` - ranking, fuzzy matching, calculator
//! * `security` - validation, argument quoting, redaction
//! * `workflow` - model, executor and the twelve starter templates
//! * `platform` - Win32 integration (launch, icons, metadata) behind a trait

pub mod calc;
pub mod discovery;
pub mod error;
pub mod fileindex;
pub mod import_export;
pub mod launch;
pub mod lnk;
pub mod models;
pub mod platform;
pub mod search;
pub mod security;
pub mod storage;
pub mod workflow;

pub use error::{CoreError, Result};
