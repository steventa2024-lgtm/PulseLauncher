# Design system

PulseLauncher uses a dark navy dashboard. Tokens live in `src/styles.css` as Tailwind v4 `@theme` variables.

| Token | Dark value | Use |
| --- | --- | --- |
| `--color-bg` | `#090D17` | Window background |
| `--color-raised` | `#111827` | Sidebar |
| `--color-panel` | `#151D2E` | Cards |
| `--color-panel-2` | `#1B2638` | Buttons, hover, icon tiles |
| `--color-line` | `#26344A` | Borders |
| `--color-accent` | `#4388FF` | Primary actions. Settings can replace this with another `#RRGGBB` |
| `--color-accent-2` | `#5BA8FF` | Focus and icon letters |
| `--color-ink` | `#F4F7FB` | Primary text |
| `--color-muted` | `#AABAD1` | Secondary text |
| `--color-faint` | `#7E90A8` | Paths and metadata |
| `--color-ok` | `#3DDC97` | Ready |
| `--color-warn` | `#E6B450` | Needs setup |
| `--color-danger` | `#F07178` | Missing or failed |

## Themes

`data-theme="light"` switches the same tokens to a light slate. `data-contrast="high"` makes muted text the same as primary text and draws borders in the accent color. `data-motion="reduce"` disables animation and transition. `data-density="compact"` shortens controls. `--text-scale` multiplies the root font size (0.85–1.6).

The dashboard applies these from saved preferences. The launcher window clears the page background so the transparent Tauri window can show through. In a plain browser that transparency is just the page color.

## Components

- `.card` is a panel with a 14px radius.
- `.btn` and `.btn-primary` are the two actions. Disabled buttons stay visible and do not look successful.
- `.pill`, `.pill-ok`, `.pill-warn`, and `.pill-danger` are status chips. Workflow readiness uses those three states and no others.
- `.kbd` is a keyboard hint.
- `.field` is a text input, select, or textarea.
- `AppMark` shows an extracted `.ico` when one exists, otherwise the first letter of the name.
- `Empty` is the state when a list has no real entries. It does not insert samples.

The Settings page includes an interface gallery of these controls in the active theme.

## Type

The font stack is Segoe UI Variable, Segoe UI, then the system sans-serif. The window is at least 800×600. The sidebar is 220px.

## Motion and focus

Focus uses a 2px accent outline. The launcher moves the selection with Up/Down and does not wrap past either end. Enter activates the highlighted row. The highlight follows the result's identity when the list refreshes, so a reorder does not launch a different row.
