# Windows install

PulseLauncher targets Windows 11 x64 and should also build for Windows 10 x64 where WebView2 and the Rust `windows` crate are available.

## What you need

- Rust 1.90 with the `x86_64-pc-windows-msvc` target
- Node.js 22
- The Visual Studio C++ build tools
- WebView2 Runtime, which current Windows 11 installs already include. Windows 10 may need the Evergreen runtime from Microsoft.

## Build

```powershell
npm ci
cargo test -p pulslauncher-core
npm test
npm run build
cargo build --release -p pulslauncher
```

`npm run build` has to finish first. The default `custom-protocol` feature embeds `dist/` and does not open the Vite URL. A Vite-backed debug session is `npm run dev` plus `cargo run -p pulslauncher --no-default-features`.

Tauri's NSIS target is configured in `src-tauri/tauri.conf.json` (`currentUser`). This Linux session did not produce an `.exe` installer. Do not treat a Linux binary as a Windows release. The `tauri` CLI, when installed, runs the same production feature for `tauri build`.

## First launch

The app creates a per-user database. It does not ask for an account. The first scan reads the user and common Start Menu shortcut folders. Access denied on the common folder is reported and does not abort the user folder.

The default shortcut is `Alt+Space`. If registration fails, the shell tries `Ctrl+Alt+Space` and stores the warning.

## Unsigned binaries

An installer built without a certificate can show a SmartScreen or publisher warning. That warning is an operating-system decision. PulseLauncher does not include a bypass. Signing can be added later by a maintainer; it is not required for the source tree to build.

## Data

App data stays in the Tauri app-data directory. Export writes a JSON file the user chooses. Clearing history removes launch events and workflow runs. Uninstall retention has not been verified because no Windows package was installed here.
