# Architecture

PulseLauncher is one Tauri 2 process. The React UI and the Rust core share one SQLite database. There is no separate search service and no web server.

```
React views  --invoke-->  Tauri commands  -->  storage / search / launch / workflows
                              |                         |
                         tray, hotkey,            pulselauncher.db
                         two windows              (WAL, per user)
```

## Windows

`src-tauri/tauri.conf.json` defines two windows:

- `main` is the dashboard (Apps & Tools, Commands & Workflows, Settings, and App Details).
- `launcher` is a borderless, always-on-top overlay. Its URL is `index.html?view=launcher`.

The frontend chooses the overlay from that query string. Both windows are hidden until Rust shows them. Closing the dashboard hides it when "close to tray" is on. Closing the overlay hides it. A second process start is handled by the single-instance plugin, which shows the dashboard.

The tray menu is Open, Toggle Launcher, Rescan, and Quit. A left click opens the dashboard.

## Data

`app.path().app_data_dir()` holds `pulselauncher.db` and imported preview images under `previews/`. The schema is `src-tauri/src/migrations/0001_init.sql`, applied with SQLite `user_version`. Preferences are one JSON document.

Discovered apps are reconciled by `identity_key`. Favorites, tags, custom names, hidden flags, and collection membership survive a rescan. A vanished entry that the user customized is kept and marked missing. A vanished entry the user never touched is removed from the database only. PulseLauncher never uninstalls a program.

## Commands

Every OS effect the UI can request is a named command in `src-tauri/src/commands/`. Arguments are structured. The capability file grants the webview window show/hide/focus/close/resize, events, and the native file picker. Shortcuts, clipboard, notifications, and autostart are called from Rust.

## Search

`search` ranks apps, commands, and workflows with a small fuzzy scorer. Exact match outranks a heavily used prefix. An empty query returns favorites, then recents. Prefixes: `@` apps, `>` commands, `/` workflows, `#` files, `=` calculator, `?` web search. File search and web search are off until the user turns them on. The calculator is a recursive-descent parser. It does not evaluate JavaScript.

## Launch

`plan_app_launch` decides the target. On Windows, `ShellExecuteExW` starts it (`SEE_MASK_NOCLOSEPROCESS | SEE_MASK_FLAG_NO_UI`, verb `open`). Shortcuts are handed to Windows as the `.lnk`, so Windows applies the shortcut's own arguments. Packaged apps use `shell:AppsFolder\<AUMID>`. A missing file returns `target_missing` and is not reported as success.

On any other OS the same call is labeled `linux-dev` and is only a development fallback (`xdg-open` or `Command::new`). It is not a supported product target.

## Discovery

Start Menu locations come from Windows known folders. `.lnk` files are parsed in `discovery/lnk.rs` (MS-SHLLINK). The parser never executes the shortcut. Packaged apps, when included, come from `Get-StartApps` without changing PowerShell execution policy. Desktop shortcuts are off by default.

## Workflows

The engine in `workflows/mod.rs` walks a JSON definition. There is no `goto`. Limits: 64 steps, depth 8, 5 minutes, waits of at most 60 seconds, and at most 2 retries. `${name}` substitution is a single pass, so a value that itself looks like a placeholder is not expanded again, and substitution never splits an argument.

Readiness is computed: `ready`, `needs_setup`, or `unavailable`. A run refuses the last two before it starts. If every step is skipped, the run is failed, not successful.

Twelve templates are seeded once. Their Windows effects were unit-tested with a fake action host on Linux. They have not been executed against a Windows shell.

## Integrations

Ollama is off by default and talks only to the configured localhost endpoint. A suggestion is displayed and its `executed` flag stays false. PulseFlow is unconfigured until the user supplies a loopback URL. A probe is a GET. It does not start a workflow.

## What this repository does not include

There is no screenshot-capture of other windows, and no built installer artifact. Catalog order is changed with move up, move down, arrow keys, or a drag handle; that calls `reorder_apps`. See `IMPLEMENTATION_STATUS.md`.
