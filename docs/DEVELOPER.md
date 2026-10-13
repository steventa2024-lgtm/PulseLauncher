# Developer guide

## Layout

| Path | Role |
| --- | --- |
| `src/App.tsx` | Dashboard shell, theme attributes, catalog load |
| `src/features/launcher` | Overlay |
| `src/features/apps` | Catalog |
| `src/features/details` | One app |
| `src/features/workflows` | Templates and runs |
| `src/features/settings` | Preferences and gallery |
| `src/lib/api.ts` | Typed invoke wrappers. Command names are snake_case |
| `src-tauri/src/commands` | The only bridge |
| `src-tauri/src/storage` | SQLite |
| `src-tauri/src/search` | Ranking, calculator, file index |
| `src-tauri/src/launch` | Launch planning and captured commands |
| `src-tauri/src/discovery` | Start Menu and `.lnk` |
| `src-tauri/src/workflows` | Engine and the 12 templates |
| `src-tauri/src/platform` | Windows shell and the Linux dev fallback |
| `src-tauri/capabilities/default.json` | Webview permissions |

## Commands

Rust functions use snake_case. The TypeScript wrappers pass camelCase fields, which is Tauri's default rename. Errors cross the bridge as `{ code, message }` and `api.ts` rethrows `Error(message)` with the original value as `cause`.

Events: `workflow-progress`, `workflow-confirm`, `app-notice`, `scan-finished`.

## Run

```bash
npm run dev          # Vite only. Lists stay empty; the bridge is not there
npm run tauri dev    # The real process, database, and both windows
npm test
npm run rust:test
```

On Linux, `npm run tauri dev` needs WebKitGTK. Launches are the `linux-dev` fallback. Do not record those as Windows results.

## Tests

Frontend tests mock `src/lib/api.ts`. They check empty states, keyboard selection, the preview empty copy, unavailable workflows, and navigation.

Rust tests use temporary databases, fixture `.lnk` bytes, a fake workflow host, and, when present, `/bin/echo`, `/bin/sleep`, and a mingw-built PE. They do not call `ShellExecuteExW`.

## Adding a template

Add a `Spec` in `workflows/templates.rs` with a new `template_key`. State the prerequisite in words the UI can show. Cover readiness in `workflows::tests`. Do not point a template at a program that is not installed.

## Adding a command

Add the Rust function, register it in `shell/mod.rs`, and add a wrapper in `src/lib/api.ts`. If it touches the OS, validate in `security` first. Do not grant a new plugin permission when a Rust function can do the work.
