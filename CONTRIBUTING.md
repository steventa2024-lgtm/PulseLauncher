# Contributing

PulseLauncher is a local-first Windows launcher. Changes should keep the core useful with no account, no telemetry, and no network requirement.

## Setup

- Node.js 22 and npm
- Rust 1.90 or newer (`rust-version` in `src-tauri/Cargo.toml`)
- On Linux, WebKitGTK 4.1 and the usual Tauri build packages if you want to run the window
- On Windows, the WebView2 runtime and the Visual Studio Build Tools

```bash
npm ci
npm run check
```

`npm run check` typechecks, lints, and tests the frontend, then runs `cargo fmt --check`, Clippy with warnings denied, and `cargo test`.

## What belongs here

- Launch, search, and workflow behavior that fails closed and reports the real reason
- Tests that use temporary directories, fixture files, or the fake workflow host
- Docs that match what the code actually does

## What does not

- Telemetry, accounts, paid APIs, or a hosted backend
- A general `execute(string)` command, shell-concatenated arguments, or a workflow action that runs arbitrary text from search
- Invented catalog entries for PulseCut, PulseFlow, OpenPulse, PulseApply, or any other program that is not registered
- Bypassing UAC, SmartScreen, antivirus, or another app's permissions
- Copying another launcher's branding, artwork, or code

## Workflow changes

Built-in templates live in `src-tauri/src/workflows/templates.rs` and are inserted once by `template_key`. Editing a seeded row must survive restart. Reset restores that template's steps and keeps its id.

A step of type `runCommand` forces the workflow to untrusted. Trust is a separate action and does nothing unless developer commands are enabled in settings.

## Reporting security issues

See `docs/SECURITY.md`. Please do not open a public issue for an unfixed vulnerability if a private report is possible.
