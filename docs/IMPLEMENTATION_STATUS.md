# Implementation status

Status words mean:

- **DONE** — implemented and covered by an automated test that ran in this workspace
- **PARTIAL** — implemented, with a stated gap
- **NOT VERIFIED ON WINDOWS** — the Windows-only code compiled; it was not executed on Windows
- **NOT STARTED** — intentionally absent

Host for the tests below: Ubuntu, Rust 1.99, Node 22. `cargo check --target x86_64-pc-windows-gnu` completed successfully. That check does not launch a Windows process.

| Area | Status | Evidence |
| --- | --- | --- |
| SQLite schema, preferences, collections, history | DONE | `storage::tests` |
| App reconcile, manual registration, hide vs delete | DONE | `storage::apps::tests` |
| Fuzzy search, usage ranking, modes, calculator | DONE | `search::tests`, `search::calc::tests` |
| URL, URI, path, quoting, redaction | DONE | `security::tests` |
| MS-SHLLINK parser | DONE | `discovery::lnk` tests, including mutation cases |
| File index exclusions and cancel | DONE | `search::files` tests |
| Workflow engine, 12 templates, readiness, confirm, trust | DONE | `workflows::tests` against a fake action host |
| Notes that do not overwrite | DONE | `workflows::notes::tests` |
| Captured command without a shell | DONE | `/bin/echo` and `/bin/sleep` on this Linux host |
| PE version info and icon bytes | DONE | `metadata::tests` built a PE with mingw on this host |
| Ollama disabled / PulseFlow non-loopback | DONE | `integrations::tests` |
| Windows `ShellExecuteExW`, known folders, packaged apps | NOT VERIFIED ON WINDOWS | `platform/windows.rs` compiled for `x86_64-pc-windows-gnu` |
| Global hotkey, tray, single instance, autostart | PARTIAL | Registered in `shell/mod.rs`. Not exercised against a Windows session or a Linux desktop session in CI |
| Five UI views | PARTIAL | React views and Vitest navigation. On this Linux desktop the WebKit window showed all five views: empty scan with an explicit Windows-only message, a rejected non-.exe path, a real `.exe` registration, details with "No preview available", workflow readiness (Ready / Needs setup / Unavailable), Copy Today's Date succeeding with the machine's date, and the calculator result 144. A later session reordered two registered apps with the grip's arrow key (True moved ahead of Second), kept notices in the header, and declined Check Then Launch after it reported that `/tmp/pulse-true.exe` exists. A synthetic pointer drag did not start a drag in WebKit. Global hotkey and Windows shell launch were not exercised |
| Reorder | DONE | Move up, move down, arrow keys, and a drag handle call `reorder_apps`. A filtered list reorders only the visible apps (`reorderVisible`). On the Linux desktop, Arrow Down on the grip swapped two apps. The drag handle is visible; a synthetic mouse drag did not start an HTML5 drag in WebKit |
| Window screenshot capture | NOT STARTED | Details tells the user to import an image. No capture API exists |
| NSIS installer | NOT STARTED | `tauri.conf.json` targets NSIS per-user with the WebView2 bootstrapper. No installer file was produced |
| Code signing | NOT STARTED | The binary would be unsigned. SmartScreen must not be bypassed |
| Linux as a product target | NOT STARTED | Fallback launches are labeled `linux-dev` |

## Templates

All twelve are seeded by stable `template_key` and are not duplicated on restart (`templates_seed_once_and_edits_survive`).

| Template | Readiness without extra setup |
| --- | --- |
| Open Downloads | Ready when the OS known folder resolves. The unit test asserts that path when it exists, and returns without an assertion when the folder is absent |
| Open Documents | Same, for Documents |
| Open Windows Settings | Unavailable when `is_windows` is false |
| Open Task Manager | Needs setup / unavailable when `taskmgr.exe` is absent |
| Open Terminal | Needs setup until a real terminal path is chosen |
| Open Project Folder | Needs setup until a folder is chosen |
| Launch Development Workspace | Needs setup until at least two configured items validate |
| Copy Today's Date | Ready. The test checks the local ISO date |
| Timestamped Quick Note | Needs setup until a folder is chosen. The test creates a unique `.txt` |
| Open Project Website | Needs setup until an http(s) URL is saved. `javascript:` is rejected |
| Launch ZeroPulse Stack | Needs setup while the app list is empty |
| Check Then Launch | The path may be missing. The missing branch notifies and does not launch. Decline on confirm cancels |

## Production readiness

**NOT READY.** The engine and the interface are implemented and tested on Linux. Windows launch, Start Menu discovery, the global hotkey, the tray session, and the installer have not been run on Windows.
