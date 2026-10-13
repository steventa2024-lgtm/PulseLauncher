# Third-party licenses

PulseLauncher is MIT licensed. Direct dependencies below are MIT, Apache-2.0, ISC, or dual MIT/Apache-2.0. None of them forbid redistributing this application as MIT source plus an unsigned Windows build.

Pinned versions are `Cargo.lock` and `package-lock.json`. Those files are the dependency inventory. Review them when you add a crate or npm package.

## Rust (direct)

| Crate | License |
| --- | --- |
| serde, serde_json, thiserror, rusqlite, uuid, chrono, regex, dirs, sha2, once_cell, url, png, base64, ureq, arboard, tempfile | MIT or MIT OR Apache-2.0 |
| windows, tauri, tauri-build, tauri-plugin-autostart, tauri-plugin-dialog, tauri-plugin-global-shortcut, tauri-plugin-notification, tauri-plugin-single-instance | MIT OR Apache-2.0 |

`rusqlite` is built with the bundled SQLite library (public domain). The `windows` crate is used only on Windows.

## JavaScript (direct)

| Package | License |
| --- | --- |
| react, react-dom, zustand, vite, vitest, tailwindcss, typescript, eslint, jsdom, @testing-library/react, @tauri-apps/api, @tauri-apps/plugin-dialog | MIT |
| lucide-react | ISC |

## Assets

The window icon is an original three-bar mark generated in this repository (`src-tauri/icons`). It is not copied from another launcher. No screenshots, fonts, or product artwork are bundled.

## Not included

This tree does not vendor PowerToys, Raycast, Alfred, Flow Launcher, or Windows components beyond calling documented OS APIs on Windows.
