# Release checklist

Version 0.1.0 is not a release. Do not tag, publish a GitHub release, or attach an installer until every box below is checked on Windows 11 x64 and the evidence is linked from the pull request.

## Automated, already expected on CI

- [ ] `cargo test --workspace` on Linux
- [ ] `npm test`, `npm run lint`, and `npm run build` on Linux
- [ ] `cargo test --workspace` and `npm test` on a Windows runner

## Windows manual gate

- [ ] `npm run tauri build` produces an NSIS installer
- [ ] Current-user install completes without a silent elevation
- [ ] Uninstall removes the program files and leaves app data
- [ ] Hotkey opens the palette on the monitor under the cursor
- [ ] Scan lists real Start Menu shortcuts, including a non-ASCII name
- [ ] Launching one of those apps returns an outcome and a history row
- [ ] A missing target shows an error and does not close the palette
- [ ] A packaged app either launches through its AUMID or is reported as unsupported
- [ ] Developer commands stay off after import
- [ ] Close to tray hides the window; Quit exits the process
- [ ] 800×600 and a scaled display do not clip the main window
- [ ] SmartScreen warning is visible and is not suppressed

## Honesty

- [ ] README status still matches what was actually run
- [ ] No screenshot in the repository shows an app that was not installed
- [ ] Third-party licenses include every new direct dependency

When the Windows gate is done, change the readiness sentence from **NOT READY** to **VERIFIED FOR DEFINED MVP** only for the behaviors that were exercised. Leave the rest listed.
