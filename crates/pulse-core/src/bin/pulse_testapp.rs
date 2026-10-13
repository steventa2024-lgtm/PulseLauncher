//! Harmless test fixture executable used by integration tests (never shipped in the installer).
//!
//! Usage: `pulse-testapp <marker-file> [args...] [--sleep-ms N]`
//! Writes one JSON-encoded argument per line to the marker file, optionally sleeps, exits 0.

use std::io::Write;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(marker) = args.first() else { std::process::exit(2) };
    let mut f = std::fs::File::create(marker).expect("create marker");
    for a in &args[1..] {
        writeln!(f, "{}", serde_json::to_string(a).unwrap()).unwrap();
    }
    writeln!(f, "cwd={}", std::env::current_dir().map(|p| p.display().to_string()).unwrap_or_default()).unwrap();
    f.flush().unwrap();
    if let Some(pos) = args.iter().position(|a| a == "--sleep-ms") {
        if let Some(ms) = args.get(pos + 1).and_then(|v| v.parse::<u64>().ok()) {
            std::thread::sleep(std::time::Duration::from_millis(ms));
        }
    }
}
