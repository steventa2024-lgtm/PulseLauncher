# PulseLauncher

PulseLauncher is a free, open-source, local-first launcher and app hub by ZeroPulse. It is a Windows 11 desktop app: a search overlay, an app catalog, workflows, and settings, in one process and one SQLite database.

It does not require an account, a subscription, or a network connection after it is installed. Optional local integrations (Ollama, and a PulseFlow address you configure) stay off until you turn them on.

The license is MIT. Third-party notices are in [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md) and [NOTICE](NOTICE).

## Status

The Rust core and the five views are implemented. Automated tests pass on Linux, and `cargo check --target x86_64-pc-windows-gnu` compiles the Windows modules. Those modules have not been executed on Windows, and an installer has not been produced. See [docs/IMPLEMENTATION_STATUS.md](docs/IMPLEMENTATION_STATUS.md).

**Production readiness: NOT READY.**

## Views

1. **Launcher** — keyboard search for apps, commands, workflows, an optional file index, a calculator, and optional web search
2. **Apps & Tools** — scan, add, favorite, reorder, and open details
3. **Commands & Workflows** — twelve built-in templates with Ready / Needs setup / Unavailable
4. **Settings** — appearance, shortcut, privacy, developer commands, local integrations, backup, diagnostics
5. **App details** — target, metadata, activity, and a preview you import yourself

## Develop

Requirements: Node.js 22, Rust 1.90+, and a Tauri 2 platform toolchain. Windows is the product target. Linux can build and test the core.

```bash
npm ci
npm run check
npm run tauri dev
```

`npm run dev` serves the interface without the desktop bridge. Lists stay empty on purpose.

| Document | Contents |
| --- | --- |
| [docs/USER_GUIDE.md](docs/USER_GUIDE.md) | How to scan, launch, and configure |
| [docs/DEVELOPER.md](docs/DEVELOPER.md) | Modules, commands, tests |
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | Process, database, launch path |
| [docs/DESIGN_SYSTEM.md](docs/DESIGN_SYSTEM.md) | Tokens and components |
| [docs/SECURITY.md](docs/SECURITY.md) | Threat model |
| [docs/PRIVACY.md](docs/PRIVACY.md) | What stays on the computer |
| [docs/WINDOWS_INSTALL.md](docs/WINDOWS_INSTALL.md) | Per-user install, WebView2, SmartScreen |
| [docs/RELEASE_CHECKLIST.md](docs/RELEASE_CHECKLIST.md) | What Windows still has to prove |
| [docs/FEATURE_MATRIX.md](docs/FEATURE_MATRIX.md) | What is in this version |
| [CONTRIBUTING.md](CONTRIBUTING.md) | How to change it |

## Data

The database path is printed in Settings → Diagnostics. On Windows it is the roaming app data directory for `com.zeropulse.pulselauncher`.
