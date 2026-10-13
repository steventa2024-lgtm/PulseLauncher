# Developer guide

`CONTRIBUTING.md` is the short policy. This page is how the tree is laid out and how to check it.

## Layout

| Path | Role |
| --- | --- |
| `crates/pulse-core` | Storage, discovery, search, workflows, security. No UI |
| `crates/pulse-core/src/bin/pulse_testapp.rs` | Test double that writes its arguments to a file. Not installed |
| `src-tauri` | Tauri 2 shell, tray, hotkey, commands |
| `src` | React views |
| `src/features/gallery` | Component gallery at `?view=gallery` |

Platform code: `platform/win32.rs` on Windows, `platform/generic.rs` otherwise. The generic module spawns a process for development tests. It does not pretend to scan a Start Menu.

## Commands

```bash
npm ci
cargo test --workspace
npm test
npm run lint
npm run build
```

Desktop:

```bash
npm run tauri dev
npm run tauri build
```

`npm run build` writes `dist/`. Tauri reads that folder. `dist/` is gitignored.

Cross-check the Windows module from Linux when `x86_64-pc-windows-gnu` is installed:

```bash
cargo check -p pulse-core --target x86_64-pc-windows-gnu
```

That check does not execute Win32.

## Rules that tests lock in

- No command takes a shell string.
- Workflow substitution does not re-parse inserted text.
- Note files are created with `create_new` and a unique suffix.
- Imports strip trusted flags on `RunCommand` and ignore `developer_command_execution` and `start_with_windows`.
- Search of 1000 apps is asserted under 50 ms in the debug test. Release timing on Windows is still unmeasured.

## CI

`.github/workflows/ci.yml` runs the Rust tests and the frontend on Ubuntu, and the Rust tests plus the frontend tests on Windows. It does not build the NSIS installer and it does not pass `-D warnings` to Clippy.
