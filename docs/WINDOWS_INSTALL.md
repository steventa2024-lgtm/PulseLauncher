# Windows install

This is the intended install path. An installer binary is not in this repository, because it was not built here.

## Requirements

- Windows 11 x64, or Windows 10 x64 if the Tauri and WebView2 dependencies still support it
- The Microsoft Edge WebView2 runtime. The NSIS setting `webviewInstallMode` is `downloadBootstrapper`, so the installer can download it. That download needs network once. After that, search, launch, and workflows do not
- A normal user account. The installer mode is `currentUser`. It does not request elevation by itself

## What you should see

The build is unsigned. Windows SmartScreen can warn. That warning is correct for an unsigned binary. Do not disable SmartScreen to get past it. If you build it yourself, you can review the source and the lockfiles first.

PulseLauncher does not add a Defender exclusion, does not change UAC, and does not run as administrator unless you launch a program that itself requires elevation. Windows will show its own prompt in that case.

## Data location

The database and imported previews are under the per-user app data directory for `com.zeropulse.pulselauncher`. Settings → Diagnostics prints the exact path and the database file after the app has started. Uninstalling the program does not delete that folder unless you remove it yourself.

## Build (on Windows)

```bash
npm ci
npm run tauri build
```

The expected bundle target is NSIS. Do not treat a failed build as a successful installer. `docs/RELEASE_CHECKLIST.md` lists what has to be true before a release exists.

## What a Linux checkout can prove

`cargo check --target x86_64-pc-windows-gnu` compiled the Windows modules on the development host. It did not install the app, register a hotkey with the Windows shell, scan a Start Menu, or launch a Windows program.
