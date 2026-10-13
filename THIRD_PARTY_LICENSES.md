# Third-party licenses

PulseLauncher is MIT licensed. Direct dependencies below were selected because their terms allow open-source redistribution. Transitive crates and npm packages are pinned in `Cargo.lock` and `package-lock.json`.

No proprietary launcher source, paid UI kit, or product screenshot is included. The window icon is original.

## Rust (direct)

| Crate | License |
| --- | --- |
| tauri, tauri-build, tauri-plugin-dialog, tauri-plugin-global-shortcut, tauri-plugin-notification, tauri-plugin-single-instance | MIT OR Apache-2.0 |
| rusqlite | MIT |
| serde, serde_json | MIT OR Apache-2.0 |
| thiserror | MIT OR Apache-2.0 |
| uuid | MIT OR Apache-2.0 |
| chrono | MIT OR Apache-2.0 |
| regex | MIT OR Apache-2.0 |
| url | MIT OR Apache-2.0 |
| walkdir | MIT OR Unlicense |
| ureq | MIT OR Apache-2.0 |
| open | MIT |
| arboard | MIT OR Apache-2.0 |
| base64 | MIT OR Apache-2.0 |
| tempfile (tests) | MIT OR Apache-2.0 |
| known-folders, winreg, windows (Windows-only) | MIT OR Apache-2.0, except winreg which is MIT |

## JavaScript (direct)

| Package | License |
| --- | --- |
| react, react-dom | MIT |
| vite, @vitejs/plugin-react | MIT |
| typescript | Apache-2.0 |
| tailwindcss, postcss, autoprefixer | MIT |
| zod | MIT |
| zustand | MIT |
| lucide-react | ISC |
| @tauri-apps/api, @tauri-apps/plugin-dialog | MIT OR Apache-2.0 |
| vitest, jsdom, testing-library | MIT |
| eslint, typescript-eslint | MIT |

WebView2 on Windows and WebKitGTK on Linux are operating-system components. This repository does not vendor them.

If a later audit finds a transitive license that blocks MIT distribution, replace that dependency before publishing a binary release.
