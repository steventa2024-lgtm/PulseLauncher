# Reference implementation

The selected concept is Option 2, Dark Blue Professional Studio. The full collage is `pulselauncher_selected_full_reference.png`. It is a presentation board. The numbered titles outside the windows are not part of the application.

Crops:

| File | Screen | React surface |
| --- | --- | --- |
| `pulselauncher_main_launcher_reference.png` | Floating command window | `src/features/launcher/Launcher.tsx` in the `launcher` Tauri window (`?view=launcher`) |
| `pulselauncher_apps_tools_reference.png` | Apps & Tools | `src/features/apps/AppsView.tsx` |
| `pulselauncher_commands_reference.png` | Commands & Workflows | `src/features/workflows/WorkflowsView.tsx` |
| `pulselauncher_settings_reference.png` | Settings & Customization | `src/features/settings/SettingsView.tsx`, Appearance section |
| (not in the collage) | App Details & Preview | `src/features/details/DetailsView.tsx` |

Shared chrome is `src/components/chrome.tsx`: `PulseBrand`, `HotkeyPill`, `StudioHeader`, `SearchBox`, `ToggleSwitch`, `SidebarItem`. Tokens are in `src/styles.css`.

## Region map

**Header.** Waveform mark is `public/pulselauncher.svg`. The wordmark is white `Pulse` plus accent `Launcher`. The shortcut pill uses `.kbd`. The gear opens Settings. Notices sit in the header, newest first, and scroll inside a 4rem cap.

**Search.** `.search-ring` is a 1px accent border plus a soft accent glow. The launcher placeholder is “Type to search apps, files, commands, or ask AI…”. Ask-AI does nothing unless the user turns on local Ollama in Settings. There is no paid API.

**Selected row.** `.row-selected` is a navy-to-accent tint with an accent stroke. It is not a white fill.

**App tile.** `.tile` is a rounded panel. The pin is the top-right star. The grip is the top-left `::` and uses pointer capture, not an HTML5 drag. Arrow keys and Move up / Move down remain.

**Settings.** Appearance has Dark, Light, and System cards, seven accent swatches plus a hex field, Window Style (`comfortable` = Modern (Rounded), `compact` = Compact), and switches for icons, shortcuts, transparency, reduced motion, and higher contrast. The aside is a live token preview, not a pasted screenshot. Other sections keep the existing preferences.

## Fixture mode

`?fixture=studio` is a frontend screenshot harness. It does not call the database and it does not register apps. The banner says the names are not installed. `npm run dev` without that query stays empty when the desktop bridge is down.

The launcher fixture background is a CSS gradient drawn in the page. It is labeled “Fixture wallpaper” and is not the Tauri overlay. The real overlay stays transparent when translucency is on, so the user’s desktop shows through. No Windows wallpaper is bundled.

Empty fixture search shows five labeled rows so the floating panel has the reference’s row count. Typing searches the rest of the fixture catalog. Those rows cannot launch.

## Known composition differences

- Catalog subtitles are the stored category, not the collage lines “Video Editor” or “ZeroPulse App”.
- Command names are the twelve stored templates. The collage’s `dev:server` row is not a built-in workflow. The dashboard shows the detail pane beside the list so configure and run stay available.
- Brand icons in the collage are not shipped. Tiles use an extracted icon or a letter.
- App Details is a fifth view in the same navy language. The collage does not picture it.
- Settings keeps text scale, reduced motion, and higher contrast under Appearance. The collage shows three toggles.
- The live theme preview is a small token mock, not the floating waveform card drawn in the collage.
- Comparison boards are in `docs/design/visual-regression/`. They are side-by-side views. No pixel-match percentage is claimed. The reference is a compressed collage and the captures contain different text.

## Pointer reorder

On Ubuntu, in the Tauri WebKit window, holding the pointer on Second’s grip and releasing it on True placed Second first. The before/after capture is `capture-apps-real-ubuntu-webkit-after-pointer-drag.png`. Release is handled on `window`, because a release on the destination card does not hit the grip. This was not repeated on Windows WebView2.
