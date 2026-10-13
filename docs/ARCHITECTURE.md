# Architecture

PulseLauncher is one desktop process with two windows and one SQLite database.

## Crates

- `crates/core` (`pulslauncher-core`) has no Tauri dependency. Discovery, launch, search, workflows, and storage are tested here on any host.
- `src-tauri` (`pulslauncher`) is the Tauri 2 shell: windows, tray, global shortcut, and typed commands. The webview cannot call a general execute function.

## Windows

| Label | Role |
| --- | --- |
| `dashboard` | Apps, details, commands, settings, design gallery |
| `launcher` | Borderless search palette, hidden until the hotkey or the Launcher button |

Both load the same frontend. The launcher window opens `index.html?view=launcher`.

Release builds enable the `custom-protocol` Cargo feature and serve the Vite `dist/` bundle from the `tauri://` asset protocol. Without that feature the webview loads `devUrl` (`http://127.0.0.1:1420`) and shows a connection error when Vite is not running. `127.0.0.1` is used instead of `localhost` because some hosts resolve `localhost` to IPv6 only.

Linux `.desktop` files may contain `Version=1.0`. That value is the desktop-entry specification revision. PulseLauncher records an application version only from `X-App-Version`.

Clipboard writes stay on one background thread. Dropping an X11 clipboard handle clears the selection, so a short-lived handle would report success and then leave the clipboard empty.

## Data

SQLite uses WAL at the Tauri app-data path (`pulslauncher.db`). Schema version is 1. Preferences are one JSON row. Apps are deduplicated by `identity_key` (`path:` plus a canonical path, or `uri:`). A rescan keeps favorites, pins, tags, and user-edited names. Discovered apps that disappear are marked missing rather than deleted.

## Launch

Executables start with `std::process::Command` and an argument array. Shortcuts and URLs go through the `open` crate (`ShellExecute` / `xdg-open`), not `cmd /c`. The result means the OS accepted the request. It does not claim the process is still running unless a wait was requested and the exit status was observed.

`ms-settings:` is allowed and fails with a platform error off Windows. `javascript:`, `file:`, and URLs with embedded credentials are rejected.

## Discovery

- Windows: user and common Start Menu `.lnk` files, parsed by an MS-SHLLINK reader. A COM `IShellLink` resolver exists behind `cfg(windows)` and has not been executed in this environment.
- Linux: XDG `.desktop` files. `Hidden` and `NoDisplay` entries are skipped. Discovery does not execute the program.
- Desktop shortcuts are included only when the user opts in.
- Packaged AppX enumeration is not implemented. Packaged apps that appear as Start Menu shortcuts can still be recorded as shortcuts.

## Workflows

The engine runs steps in order: launch app, trusted tool, folder, known folder, URL, clipboard, delay, notification, note, existence check, confirmation, developer command, and collection launch. A branch target runs once and then stops. Developer commands require `developerCommandsEnabled`. Twelve templates are seeded once by `template_key`.

## Optional integrations

Ollama suggestions call a user-enabled local endpoint and return text with `executed: false`. PulseFlow is tested only when the user supplies an executable or endpoint. OpenPulse launches only a user-supplied path or `http(s)` URL. No default port, token, or product screenshot is invented.
