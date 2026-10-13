# Feature matrix

Compared as product behavior, not as a claim that any other launcher's code was used.

| Capability | PulseLauncher |
| --- | --- |
| Local catalog and launch | Yes. Windows shell launch is implemented and not yet run on Windows |
| Start Menu scan | Windows only, known folders, `.lnk` parsed without executing |
| Packaged apps | Optional, `Get-StartApps`, AUMID with `!` only |
| Global shortcut | `Alt+Space`, fallback `Ctrl+Alt+Space`, result shown on save |
| Calculator | Yes, arithmetic and functions, no `eval` |
| File search | Opt-in, chosen roots only |
| Web search | Off by default. Opens the browser |
| Workflows | 12 templates plus user workflows. Readiness is Ready, Needs setup, or Unavailable |
| Plugins from the internet | No |
| Accounts, sync, telemetry | No |
| AI | Optional local Ollama. Suggestions are not run |
| Uninstall programs | No. Remove hides a discovered entry or deletes a manual registration |
| Themes | Dark navy, light, and system. Accent swatches, density, text scale, contrast, reduced motion, icon and shortcut visibility, launcher translucency |
| Portable / custom apps | Manual add of an absolute path, shortcut, or approved URI |

## Views

1. Launcher overlay — brand header, glowing search, keyboard, prefixes, Enter and Alt+1–5
2. Apps & Tools — category nav with real counts, tile grid, rescan, add, run, details, pin, move up/down, pointer reorder
3. Commands & Workflows — the stored templates, configure, run, confirm, step list. Fixture rows cannot run
4. Settings — appearance cards, accent, window style, toggles, live preview, shortcut, privacy, paths, developer commands, local Ollama, PulseFlow, local backup, diagnostics, component gallery
5. App details — target, metadata, Open, pin, collections, activity, imported preview or "No preview available"
