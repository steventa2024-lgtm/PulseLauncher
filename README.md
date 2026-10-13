# PulseLauncher

PulseLauncher is a free, local-first desktop launcher from ZeroPulse. It searches applications, launches the ones you choose, and runs saved workflows. There is no account, subscription, or required network service.

The current tree builds on Linux and contains the Windows adapters. A Windows installer has not been produced or tested in the environment that created this code. See `docs/IMPLEMENTATION_STATUS.md` before treating a feature as finished.

## What it does

- A hotkey opens a search palette in the same process as the dashboard.
- Installed apps come from Start Menu shortcuts on Windows and `.desktop` files on Linux. You can also register an executable that exists on the machine.
- App details show the real target, icon when the file is a readable image, and “No preview available” when nobody imported a screenshot.
- Twelve workflow templates ship with the database. They stay on **Needs setup** until their files, apps, or platform are actually present.
- Settings, collections, and history stay in a local SQLite file.

## Develop

```bash
cargo test -p pulslauncher-core
npm ci
npm test
npm run build
cargo build --release -p pulslauncher
```

The shell’s default Cargo feature is `custom-protocol`. That embeds `dist/` into the binary. A debug run that should talk to Vite instead uses `cargo run -p pulslauncher --no-default-features` while `npm run dev` is listening on `http://127.0.0.1:1420`. Building the shell before `dist/` exists fails in that production mode.

Rust 1.90 is pinned in `rust-toolchain.toml`.

`scripts/verify.sh` runs formatting, Clippy, Rust tests, the TypeScript check, Vitest, ESLint, and the Vite build.

## Windows

Build instructions are in `docs/WINDOWS_INSTALL.md`. The NSIS target is configured and still needs a Windows build machine. Unsigned installers can show SmartScreen warnings. The app does not try to hide or bypass those warnings.

## Layout

- `crates/core` — testable launcher engine
- `src-tauri` — windows, tray, shortcut, commands
- `src` — dashboard, launcher, details, workflows, settings
- `docs` — architecture, design, security, and status

## License

MIT. See `LICENSE` and `THIRD_PARTY_LICENSES.md`.
