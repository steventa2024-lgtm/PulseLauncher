# Architecture

PulseLauncher is one desktop process and one SQLite database. The floating palette and the dashboard are two windows of that process.

```text
React views  --invoke-->  src-tauri commands  -->  pulse-core
                                |                      |
                                |                      +-- storage (SQLite, WAL, user_version migrations)
                                |                      +-- discovery / .lnk parser
                                |                      +-- search + calculator
                                |                      +-- workflow engine
                                |                      +-- security checks
                                +-- platform trait
                                      Windows: ShellExecuteExW, IApplicationActivationManager,
                                               ExtractIconExW, known folders, Get-StartApps
                                      other OS: development-only process spawn (not a Windows claim)
```

## Why the core is separate

`crates/pulse-core` has no Tauri or UI dependency. Search ranking, shortcut parsing, workflow execution, import validation, and the security policy are tested with `cargo test` on any OS. Windows-only Win32 calls live in `platform/win32.rs` behind `cfg(windows)`.

## Commands

The webview cannot execute a shell string. Each action is its own command, for example `launch_app`, `open_path`, `open_url`, `run_workflow`, `add_app`. Arguments are arrays. Paths must be absolute. URLs must be `http` or `https`. Settings URIs must be `ms-settings:`. Ollama and optional PulseFlow probes must be loopback, except an OpenPulse URL the user explicitly saved, which is only contacted when they press Test.

## Workflow state

A run moves `Running → Succeeded | Failed | Cancelled`. Each step moves `Pending → Running → Succeeded | Failed | Skipped | Cancelled`. Cancellation is checked between steps, during waits, while a confirmation is pending, and while a developer command process is running (that process is killed). Retries are rejected for any action that is not idempotent (`copy`, `set variable`, `notify`).

`Accepted` means the OS accepted the launch request. `verified_running` is `true` only when a process handle was still alive after a short wait. A missing value means the OS did not return a handle. Neither value is invented.

## Identity

Discovered Start Menu entries are deduplicated by the resolved executable path plus the shortcut's arguments. Manual entries are keyed by their own target. A later scan updates discovered rows and marks missing ones `stale`. Favorites, tags, renames, and collection membership are kept. Manual rows are never deleted by a scan.

## Deviations

- Packaged apps are listed through `Get-StartApps` and activated with `IApplicationActivationManager`. Apps that only have an ID-list shortcut and no filesystem target are skipped rather than guessed. Not every packaged app is claimed to be launchable.
- File search is off until the user adds folders. Depth and count are capped (`fileindex.rs`).
- The visual workflow editor is an ordered step list with parameters. It is not a node canvas. PulseFlow is not embedded. A URL is used only if the user saved one and then asked to test it.
- SQLite uses `rusqlite` with the bundled library so the app does not need a system SQLite.
