//! PulseLauncher core. The native shell in `run` is the only place that
//! talks to windows; everything else is ordinary, tested Rust.

pub mod discovery;
pub mod error;
pub mod launch;
pub mod metadata;
pub mod models;
pub mod platform;
pub mod search;
pub mod security;
pub mod storage;
pub mod workflows;

mod commands;
mod integrations;
mod shell;

pub fn run() {
    shell::run();
}
