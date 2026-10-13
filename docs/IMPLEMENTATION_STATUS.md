# Implementation status

Updated for the Linux development session that introduced the foundation. Nothing below claims a Windows install was performed. Screenshots from the Linux desktop session are in `docs/screenshots/`.

## Phase 0 — Repo audit

DONE (tested). The repository started as a README. The stack is Tauri 2, Rust 1.90, React 19, TypeScript, Vite 7, Tailwind 3, SQLite via `rusqlite`.

## Phase 1 — Native foundation

PARTIAL. The core engine and the Tauri command shell build on Linux. A release binary with the `custom-protocol` feature embeds `dist/` and was opened on this desktop. It discovered 30 real `.desktop` applications. `Alt+Space` was already taken, so the shell registered `Ctrl+Alt+Space` and the Settings view shows that warning. Search for `htop` returned `/usr/bin/htop`. Windows Start Menu scanning, hotkey timing, and a packaged installer were not executed here. Shortcut registration runs on the Tauri `Ready` event so it does not block the first scan. A build without `custom-protocol` loads `devUrl` and shows a connection error when Vite is not running.

## Phase 2 — Apps & Tools

PARTIAL. Registration, favorites, pins, collections, hide versus delete, and rescan reconciliation are implemented and covered by storage tests. The dashboard UI calls those commands. A restart persistence check on Windows was not run.

## Phase 3 — App Details & Preview

PARTIAL. Metadata, launch preview, activity, and image import are implemented. On Linux, About Xfce details show the real path `/usr/bin/xfce4-about`, size 107 KB, and “Version not provided by the system” because the desktop-file `Version` key is the spec revision, not the product version. Launch started that binary and stored a launch event. The preview tab says “No preview available”. The capture button is disabled on purpose. Preview import is unit-tested. A Windows packaged-app details case was not run.

## Phase 4 — Commands & Workflows

PARTIAL. The engine and all 12 templates are implemented. Automated tests cover copy-date, quick note, conditional launch, cancellation, developer-command denial, and the empty ZeroPulse stack. On this desktop, Copy Today's Date completed and the clipboard read `2026-10-13`. The X11 clipboard handle is kept on a process thread so the selection is not dropped immediately. Templates whose prerequisites are missing stay on Needs setup. Windows-only templates were not clicked through on Windows 11.

## Phase 5 — Settings and integrations

PARTIAL. Preferences, shortcut rollback on registration failure, indexing limits, import/export, Ollama, PulseFlow, and OpenPulse are wired. Ollama and PulseFlow do nothing until configured. No live model call is required for the launcher.

## Phase 6 — Windows release

NOT STARTED as a verified release. NSIS is configured. No installer artifact was produced in this environment. CI describes a Windows test job; this session did not observe that job.

## Next actions

1. Run `cargo test -p pulslauncher-core` and `cargo check -p pulslauncher` on a Windows runner. `npm run build` must finish first so `custom-protocol` can embed `dist/`.
2. Measure hotkey summon on Windows and record median and P95. Do not copy the Linux unit-test bound into that report.
3. Walk the 12 templates on Windows 11 and write pass/fail next to each name. On Linux, only Copy Today's Date was clicked through in the GUI; the others are covered by engine tests or still need a desktop click.
4. Build the NSIS installer only on Windows, then install and uninstall it once.
5. Decide whether AppX enumeration is in scope after the Start Menu path is proven.
