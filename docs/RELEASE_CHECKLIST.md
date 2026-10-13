# Release checklist

Do not mark an item passed unless it was run on the stated OS.

## Automated, this session (Linux)

- [x] `cargo fmt --all`
- [x] `cargo clippy --workspace --all-targets -- -D warnings`
- [x] `cargo test -p pulslauncher-core` (34 passed)
- [x] `npm run typecheck`
- [x] `npm test` (10 passed)
- [x] `npm run lint`
- [x] `npm run build`

## Not run

- [ ] Windows `cargo test -p pulslauncher-core`
- [ ] Windows Start Menu discovery against a real profile
- [ ] Global hotkey conflict and summon timing on Windows hardware
- [ ] NSIS install, launch, uninstall on a clean Windows 11 machine
- [ ] Manual gate items 1–12 in the product prompt
- [ ] Dependency vulnerability scan beyond `cargo`/`npm` lockfiles being present

## Before a public Windows release

1. Run the Windows CI job and read its log.
2. Install the NSIS package on Windows 11 with no network after install.
3. Launch one real Start Menu app and one portable EXE whose path contains a space.
4. Confirm missing targets do not report success.
5. Run each of the 12 templates and record Needs setup where the prerequisite is absent.
6. Publish checksums for the installer. State that the binary is unsigned unless a certificate was used.
