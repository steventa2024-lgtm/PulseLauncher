# Release checklist

A release does not exist until every box that says "required" is done on a Windows 11 x64 machine and the artifact is the file that was tested. This repository does not currently meet that bar.

Phase 3 added studio UI captures under `docs/design/visual-regression/`. Those captures are Linux Chrome or Linux WebKit. They do not check a Windows install. No NSIS file was produced in this phase. No hotkey latency was measured. SmartScreen was not touched.

## Required before a version is called ready

- [ ] `npm run check` passes on the release commit
- [ ] `npm run tauri build` produces an NSIS installer and the file is the one you install
- [ ] Install as a normal user. Confirm SmartScreen warns because the build is unsigned, and leave SmartScreen enabled
- [ ] WebView2 is present or the bootstrapper installs it. The app then launches offline
- [ ] Start Menu scan lists real shortcuts only. A machine with none shows the empty state
- [ ] Launch a real `.lnk`, a real `.exe`, and a packaged app. A missing target shows a failure
- [ ] `Alt+Space` toggles the overlay, or the saved fallback does, and Settings shows which one registered
- [ ] Close goes to the tray. Quit from the tray exits. A second start focuses the existing window
- [ ] Each of the 12 templates shows Ready, Needs setup, or Unavailable for a reason you can see. Run one that is Ready. Decline Check Then Launch and confirm nothing starts
- [ ] File search stays empty until roots are set. Web search stays off until enabled
- [ ] Ollama and PulseFlow do nothing while left at their defaults
- [ ] Export and import round-trip on a copy of the database. Import does not mark workflows trusted
- [ ] Diagnostics data path matches the files on disk

## Not required for the first MIT source drop

- [ ] Code signing
- [ ] A Microsoft Store listing
- [ ] Plugin marketplace
- [ ] Capturing screenshots of other programs
