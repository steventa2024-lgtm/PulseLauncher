# Visual regression captures

Host: Ubuntu, display `:1`. These are not Windows 11 captures.

| File | What it is |
| --- | --- |
| `board-main-launcher.png` | Reference crop beside the fixture launcher |
| `board-apps-tools.png` | Reference crop beside the fixture catalog |
| `board-commands.png` | Reference crop beside fixture command rows |
| `board-settings.png` | Reference crop beside the live WebKit settings page |
| `board-app-details.png` | Fixture details. The collage has no crop for this view |
| `capture-launcher-fixture-ubuntu-chrome.png` | Chrome headless, 1100×720, `?view=launcher&fixture=studio` |
| `capture-apps-fixture-ubuntu-chrome-1440.png` | Chrome, 1440×900, fixture catalog |
| `capture-apps-fixture-ubuntu-chrome-1366.png` | Chrome, 1366×768 |
| `capture-apps-fixture-ubuntu-chrome-1920.png` | Chrome, 1920×1080 |
| `capture-apps-fixture-ubuntu-chrome-125.png` | Chrome device scale 1.25, window 1920×1080 |
| `capture-apps-fixture-ubuntu-chrome-150.png` | Chrome device scale 1.5, window 1920×1080 |
| `capture-workflows-fixture-ubuntu-chrome.png` | Fixture command page |
| `capture-details-fixture-ubuntu-chrome.png` | Fixture details for `fx-cut` |
| `capture-settings-real-ubuntu-webkit.png` | Tauri WebKit, real preferences |
| `capture-apps-real-ubuntu-webkit-after-pointer-drag.png` | Real catalog after a pointer drag. Second is first |
| `capture-apps-bridge-unavailable-chrome.png` | Chrome without the Tauri bridge. The catalog stays empty |

Fixture rows are labeled in the app and are not written to SQLite. No SSIM score is stored. The reference and the captures differ in size and in the words they contain.
