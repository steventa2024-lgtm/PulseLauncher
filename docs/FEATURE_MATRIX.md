# Feature matrix

Statuses match `docs/IMPLEMENTATION_STATUS.md`. “Shipped in tree” means the code is in this repository. “Verified” means a test or a Windows run actually exercised it.

| Feature | Shipped in tree | Verified |
| --- | --- | --- |
| Floating launcher palette | Yes | Keyboard behavior in Vitest only |
| Global hotkey (`Alt+Space`, fallback `Ctrl+Alt+Space`) | Yes | No |
| Apps dashboard, filters, grid/list | Yes | Empty-state and filter UI tests |
| Start Menu `.lnk` discovery | Yes | Fixture directories only |
| Desktop shortcut scan (off by default) | Yes | No |
| Packaged apps via `Get-StartApps` | Yes | No |
| Portable `.exe` and `.lnk` registration | Yes | Validation tests; dialog not opened |
| Hide discovered apps without uninstalling | Yes | Storage tests |
| Favorites, tags, rename, collections kept across scans | Yes | Reconcile tests |
| Drag reorder | Yes | No |
| App details, command preview, history | Yes | Details view test with a fixture |
| User-supplied preview image | Yes | No file was imported in a GUI |
| Commands and workflows, 12 templates | Yes | Engine tests |
| Needs-setup vs ready | Yes | Template status tests and a UI test |
| Confirm before destructive or developer steps | Yes | Engine tests |
| Developer `RunCommand` (off by default) | Yes | Engine tests; killed on timeout and cancel |
| Settings: theme, density, motion, contrast, accent | Yes | Theme class test |
| Close to tray, close launcher on blur | Yes | No |
| Start with Windows | Yes | No |
| Web search result (`http`/`https` template) | Yes | URL validation tests |
| File index (off, capped) | Yes | Index tests on a temp tree |
| Backup export / preview / apply | Yes | Round-trip test |
| Diagnostics and redacted log | Yes | Redaction unit test; log tail not opened in a GUI |
| Ollama explain (loopback, does not run tools) | Yes | No live server |
| PulseFlow / PulseCut / OpenPulse / PulseApply | Probe only | No. Names are matched against real registered apps. No endpoints are invented |
| Tray, single instance | Yes | No |
| Current-user NSIS bundle | Config only | No installer built |
| Account, telemetry, subscription | Absent on purpose | — |
