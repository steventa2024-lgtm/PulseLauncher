# Contributing

PulseLauncher is free software under the MIT license. Contributions are welcome as pull requests.

## Setup

- Rust stable (1.90 or newer; Tauri 2.12 requires it)
- Node.js 22 and npm
- On Windows: the WebView2 runtime, which current Windows 11 installs already include
- On Linux, only for development of the shared core and UI: GTK 3 and WebKitGTK 4.1

```bash
npm ci
cargo test --workspace
npm test
npm run lint
```

## What to change

- Keep launch, open, and workflow actions behind the typed Rust commands. Do not add a JavaScript bridge that executes an arbitrary string.
- New workflow actions need a unit or integration test that observes a real effect in a temporary directory.
- Do not add telemetry, accounts, or a required network service.
- If you add a dependency, record its license in `THIRD_PARTY_LICENSES.md` and reject licenses that block MIT redistribution.

## Windows evidence

Linux CI does not prove Win32 behavior. A change to discovery, launch, icons, or packaging needs a note in the pull request describing what was run on Windows, or an explicit statement that it was not run.
