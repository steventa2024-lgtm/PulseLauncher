# Implementation status

Checked on Linux (Ubuntu 24.04) with Rust stable and Node 22. This machine is not Windows. Nothing in this file claims a Win32 API was executed.

Legend: **DONE tested** means an automated test passed here. **PARTIAL** means the code exists and compiles or is covered only on the non-Windows path. **BLOCKED** means a Windows session is required and was not available. **NOT STARTED** means the behavior is absent.

## Slice that is in the tree

| Area | Status | Evidence |
| --- | --- | --- |
| SQLite schema, migrations, app identity, history | DONE tested | `cargo test -p pulse-core` storage tests |
| `.lnk` parser (no shell resolve) | DONE tested | `lnk` tests, including a generated fixture |
| Start Menu scan of a fixture directory | DONE tested | `discovery` tests; the directory is not a real Windows Start Menu |
| Search, prefixes, calculator | DONE tested | `search` and `calc` tests. A 1000-entry debug-build search finished under 50 ms on this Linux host. That is not a Windows hotkey measurement |
| Twelve workflow templates and the runner | DONE tested | `crates/pulse-core/tests/workflow_engine.rs` |
| Import/export validation | DONE tested | round-trip test; history is not exported; imports cannot turn on developer commands or autostart |
| Security checks (URL, path, redaction, args) | DONE tested | `security` tests |
| Win32 platform module | PARTIAL | `platform/win32.rs` type-checks for `x86_64-pc-windows-gnu`. It was not run |
| Generic platform | PARTIAL | Used by engine tests to spawn `pulse-testapp`. It is not a Windows launcher |
| Tauri shell and commands | PARTIAL | `cargo check -p pulse-launcher` succeeded on Linux. Hotkey, tray, single-instance, and notifications were not exercised in a GUI session |
| Five React views | PARTIAL | Vitest covers launcher keyboard behavior, empty apps, details, workflow “Needs setup”, and settings defaults. No Windows WebView was opened |
| NSIS installer | BLOCKED | `tauri.conf.json` requests a current-user NSIS bundle. No installer file was produced |
| Real Start Menu scan, ShellExecute, packaged-app activation | BLOCKED | Needs Windows |
| Hotkey summon timing | BLOCKED | Not measured |
| Screenshot capture from the screen | NOT STARTED | Previews are images the user picks. There is no capture API |

## What the first demo is supposed to be

Hotkey, palette, scan of the real Start Menu, launch through the Windows API, history row. The palette, scan, launch, and history code paths exist. The chain has not been run on Windows, so the demo is not verified.

## Known defects

- `known_folder("home")` on Windows returns none. Downloads, Documents, Desktop, and Programs are the folders the templates use.
- Workflow tests that set `SystemRoot` and `LOCALAPPDATA` change process environment variables. Parallel test runs could interfere.
- `Accepted` is not `verified_running`. A launch the OS accepts without a process handle is recorded as accepted, not as a confirmed running process.
- Clippy reports style lints. CI does not deny warnings.
- The component gallery (`?view=gallery`) is a development page of the real components. It does not invent apps.

## Production readiness

**NOT READY.**
