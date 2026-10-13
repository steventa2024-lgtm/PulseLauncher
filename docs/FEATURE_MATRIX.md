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
| Themes | Dark navy and light, accent, density, text scale, contrast, reduced motion |
| Portable / custom apps | Manual add of an absolute path, shortcut, or approved URI |

## Views

1. Launcher overlay — search, keyboard, prefixes
2. Apps & Tools — filters, rescan, add, run, details, favorite, move up/down
3. Commands & Workflows — templates, configure, run, confirm, step list
4. Settings — appearance, shortcut, privacy, developer commands, Ollama, PulseFlow, backup, diagnostics, component gallery
5. App details — target, metadata, collections, activity, imported preview or "No preview available"
