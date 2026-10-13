# Contributing

PulseLauncher is free, local-first software. Contributions should keep it that way.

- Do not add accounts, telemetry, paid API keys, or a required network service.
- Do not copy proprietary launcher source, artwork, or branding.
- Do not add a generic "run this string" command from the webview.
- Destructive or developer command execution stays off unless the user turns it on and reviews the exact program and arguments.
- If a Windows API is not tested, label the feature as partial or blocked in `docs/IMPLEMENTATION_STATUS.md`. Do not describe it as working.

## Development

Requirements: Rust 1.90, Node.js 22, and a platform WebView (WebView2 on Windows, WebKitGTK 4.1 on Linux).

```bash
cargo test -p pulslauncher-core
npm ci
npm test
npm run build
cargo build -p pulslauncher
```

`scripts/verify.sh` runs the same checks that do not need a Windows machine.

The dashboard and the launcher overlay are two windows in one process. They share the SQLite database under the OS app-data directory.
