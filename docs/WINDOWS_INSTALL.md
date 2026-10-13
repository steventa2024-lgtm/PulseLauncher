# Windows install

PulseLauncher targets Windows 11 x64. Windows 10 is acceptable where the same WebView2 runtime and the Rust `windows` crate still build. This repository was developed on Linux, so the steps below are the build procedure, not a record of an install that already succeeded.

## What you need

- Windows 11 x64 (or Windows 10 with the same toolchain)
- Rust stable, with the `x86_64-pc-windows-msvc` target if you use the MSVC linker
- Node.js 22
- The WebView2 Evergreen runtime. Current Windows 11 images already include it. The NSIS script does not download it.
- No account and no API key

## Build

From the repository root:

```bash
npm ci
npm run tauri build
```

`src-tauri/tauri.conf.json` sets the bundle target to NSIS and `installMode` to `currentUser`. The installer is written under `src-tauri/target/release/bundle/nsis/`. That directory is gitignored. If it is empty, the installer was not built.

The binary is unsigned. SmartScreen can warn. Do not bypass that warning with a script, and do not treat a warning as a failed install until you have read it and chosen whether to continue.

## What the installer does

- Installs for the current user. It does not request an install into `Program Files` and it does not include a custom UAC bypass.
- Adds a Start Menu shortcut for PulseLauncher itself.
- Leaves `%APPDATA` / the Tauri app-data directory `com.zeropulse.pulselauncher` in place on uninstall, including `pulse.sqlite`. Delete that folder yourself to remove history and registrations.

## First launch

1. The main window opens the dashboard. The launcher window stays hidden until the hotkey.
2. Onboarding asks you to scan. The scan reads Start Menu shortcuts. It does not install or remove Windows apps.
3. `Alt+Space` toggles the palette. If registration fails, the app falls back to `Ctrl+Alt+Space` and Settings shows the shortcut that is actually registered.
4. Enter on a result calls the Windows launch API. A missing file stays on screen with the error. History records the outcome the OS returned.

## What this document cannot certify

No NSIS file was produced in the environment that added this page. Installing, uninstalling, the hotkey, a real Start Menu scan, and launching a real `.exe` still have to be done on a Windows machine and written down before the status moves off **NOT READY**.
