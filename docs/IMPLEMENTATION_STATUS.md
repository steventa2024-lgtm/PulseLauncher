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
| Five UI views | PARTIAL | React views and Vitest navigation. The dark-blue studio layout is in the dashboard and the launcher. Captures are in `docs/design/visual-regression/`. On this Linux desktop the WebKit window showed the real catalog (True, Second), Settings appearance controls, and a pointer reorder. Global hotkey and Windows shell launch were not exercised |
| Reorder | PARTIAL | Move up, move down, arrow keys, and a pointer grip call `reorder_apps`. A filtered list reorders only the visible apps (`reorderVisible`). On Ubuntu WebKit, a real pointer drag of the Second grip onto True placed Second first. That is not a Windows WebView2 result. Vitest covers the same pointerup path with a mocked `elementFromPoint` |
| Window screenshot capture | NOT STARTED | Details tells the user to import an image. No capture API exists |
| NSIS installer | NOT STARTED | `tauri.conf.json` targets NSIS per-user with the WebView2 bootstrapper. No installer file was produced |
| Code signing | NOT STARTED | The binary would be unsigned. SmartScreen must not be bypassed |
| Linux as a product target | NOT STARTED | Fallback launches are labeled `linux-dev` |

## Phase 3 visual pass

Reference crops are in `docs/design/`. The mapping is `docs/design/REFERENCE_IMPLEMENTATION.md`. `?fixture=studio` draws labeled sample rows in the page only. It does not write SQLite. `npm run dev` without that query stays empty when the bridge is down.

This phase ran, on Ubuntu:

- `npm test`: 21 Vitest tests passed
- `npx tsc --noEmit`: passed
- `npm run lint`: passed
- `cargo test --lib --offline`: 61 passed
- `cargo clippy --all-targets --offline -- -D warnings`: passed
- `cargo fmt --all -- --check`: passed

Windows GUI, Start Menu discovery, `ShellExecuteExW`, global hotkey latency, tray session, and the NSIS installer were not run. They stay **UNVERIFIED**. No hotkey median or P95 is reported because none was measured.

## Templates

All twelve are seeded by stable `template_key` and are not duplicated on restart (`templates_seed_once_and_edits_survive`). Windows GUI for every template is **NOT TESTED**. The Linux column is this phase's `cargo test --lib`.

| Template | Linux unit test | Windows GUI |
| --- | --- | --- |
| Open Downloads | Covered when the known folder resolves | NOT TESTED |
| Open Documents | Same pattern as Downloads | NOT TESTED |
| Open Windows Settings | Unavailable when `is_windows` is false | NOT TESTED |
| Open Task Manager | Needs setup / unavailable when `taskmgr.exe` is absent | NOT TESTED |
| Open Terminal | Needs setup until a real terminal path is chosen | NOT TESTED |
| Open Project Folder | Needs setup until a folder is chosen | NOT TESTED |
| Launch Development Workspace | Needs setup until at least two configured items validate | NOT TESTED |
| Copy Today's Date | Ready. The test checks the local ISO date | NOT TESTED |
| Timestamped Quick Note | Needs setup until a folder is chosen | NOT TESTED |
| Open Project Website | Needs setup until an http(s) URL is saved | NOT TESTED |
| Launch ZeroPulse Stack | Needs setup while the app list is empty | NOT TESTED |
| Check Then Launch | Missing path does not launch. Decline cancels | NOT TESTED |

## Production readiness

**NOT READY.** The engine and the studio UI are implemented and tested on Linux. Windows launch, Start Menu discovery, the global hotkey, the tray session, and the installer have not been run on Windows.
