# Feature matrix

Status values: `DONE (tested)`, `PARTIAL`, `BLOCKED`, `NOT STARTED`.

| Feature | Status | Evidence |
| --- | --- | --- |
| SQLite schema, migrations, idempotent template seed | DONE (tested) | `storage::tests` on Linux |
| Fuzzy search, modes, calculator, selection stability | DONE (tested) | Rust search tests; Vitest retain-selection test |
| `.lnk` byte parser | DONE (tested) | `lnk::tests` fixtures, not live Windows shortcuts |
| Linux `.desktop` discovery without executing targets | DONE (tested) | `discovery::tests`. A live scan stored 30 real entries. Spec `Version` is not shown as the app version |
| Windows Start Menu scan | PARTIAL | Code path present; not executed on Windows in this session |
| COM `IShellLink` resolver | BLOCKED | Compiles only on Windows; not run |
| Packaged AppX enumeration | NOT STARTED | Start Menu shortcuts only |
| Launch with argument array, spaced Unicode path, missing target | DONE (tested) | `launch::tests` using a compiled marker on Linux |
| Twelve workflow templates and engine | DONE (tested) | `workflows::tests` for date copy, note, branch, cancel, empty ZeroPulse stack |
| Template OS behavior on Windows (Task Manager, ms-settings) | BLOCKED | Assessed as unavailable off Windows; not run on Windows 11 |
| Tauri commands, tray, hotkey, two windows | PARTIAL | Linux release binary with `custom-protocol` shows both windows. `Alt+Space` was taken, so the active shortcut is `Ctrl+Alt+Space`. Hotkey latency was not measured. Windows was not run |
| Five React views sharing the database | PARTIAL | Linux GUI walkthrough of Apps, Details, Commands, Settings, and the launcher palette. Vitest covers selection and empty states. Windows layout was not run |
| App details, preview import, no fabricated screenshot | PARTIAL | Linux details for About Xfce show `/usr/bin/xfce4-about`, 107 KB, and “No preview available”. Launch started `/usr/bin/xfce4-about`. Import is unit-tested. Live capture stays disabled |
| Settings, import/export, Ollama/PulseFlow honesty | PARTIAL | Validation tested. Live Ollama not required |
| NSIS installer and Windows install gate | NOT STARTED | No Windows runner was used to produce or install a package |
| Code signing / SmartScreen bypass | NOT STARTED | Unsigned builds may warn. The app does not try to bypass that |
