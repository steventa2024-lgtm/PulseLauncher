# PulseLauncher

PulseLauncher is a free, open-source, offline Windows launcher from ZeroPulse. A hotkey opens a search palette. The same app keeps a dashboard of installed and portable programs, a small workflow runner, settings, and an app-details view. There is no account, subscription, or required network service.

The core works after installation without a paid API. Optional local features (Ollama, a PulseFlow URL you type yourself) stay off until you turn them on.

## Status

The shared engine, the five views, and their tests run on this repository. A Windows installer has **not** been produced or installed from the environment that built this tree. See `docs/IMPLEMENTATION_STATUS.md` before treating a behavior as verified on Windows.

**Production readiness: NOT READY.** The Windows install, hotkey, Start Menu scan, and real process launch still need a Windows 11 run.

## Views

1. **Main launcher** — search palette (`Alt+Space`, or `Ctrl+Alt+Space` when the first shortcut cannot be registered).
2. **Apps & Tools** — Start Menu scan, portable `.exe` registration, collections.
3. **Commands & Workflows** — twelve editable templates and a sequential runner.
4. **Settings** — theme, shortcut, indexing opt-in, privacy, diagnostics.
5. **App details** — metadata the OS actually returns, launch history, and a preview only when you add an image.

## Develop

Requirements: Rust stable, Node.js 22, and a C linker. Windows is the target OS. Linux can build the core and the WebKit shell for development; it cannot validate Win32 discovery.

```bash
npm ci
cargo test --workspace
npm test
npm run lint
```

The desktop shell is Tauri 2:

```bash
npm run tauri dev
```

On Windows, after the frontend build:

```bash
npm run tauri build
```

That produces an NSIS installer for the current user. It is unsigned. Windows SmartScreen may warn. That warning is left in place; this project does not try to suppress it.

## Data

The SQLite database, icon cache, preview images, and log live in the per-user app data directory (`com.zeropulse.pulselauncher`). Quit from the tray or Settings stops the process. Uninstalling the NSIS package removes the program files. The app-data folder is kept so a reinstall can restore your registrations; delete that folder yourself if you want it gone.

## Docs

- `docs/ARCHITECTURE.md`
- `docs/DESIGN_SYSTEM.md`
- `docs/SECURITY.md`
- `docs/PRIVACY.md`
- `docs/USER_GUIDE.md`
- `docs/WINDOWS_INSTALL.md`
- `docs/FEATURE_MATRIX.md`
- `docs/IMPLEMENTATION_STATUS.md`
- `docs/RELEASE_CHECKLIST.md`
- `docs/DEVELOPER.md`

## License

MIT. Third-party notices are in `THIRD_PARTY_LICENSES.md`.
