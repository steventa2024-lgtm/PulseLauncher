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

The dashboard applies these from saved preferences, including `theme=system` via `prefers-color-scheme`. The launcher window clears the page background when translucency is on, so the real desktop shows through. A `?fixture=studio` page may draw an original CSS blue gradient behind the panel for screenshots. That gradient is labeled fixture-only and is not the overlay.

## Components

Shared pieces live in `src/components/chrome.tsx`: `PulseBrand`, `HotkeyPill`, `StudioHeader`, `SearchBox`, `ToggleSwitch`, and `SidebarItem`.

- `.card` is a panel with a 14px radius.
- `.search-ring` is the electric-blue search outline and soft glow. The input inside it has no second border.
- `.row-selected` is a navy accent tint with a thin accent stroke. It is not a white fill.
- `.tile` is an app card or theme card. A pinned star sits at the top right. The reorder grip is a 32px hit target at the top left.
- `.nav-item` is a sidebar row. The active row uses an accent tint and border.
- `.switch` is the compact toggle. On is the accent color.
- `.btn` and `.btn-primary` are the two actions. Disabled buttons stay visible and do not look successful.
- `.pill`, `.pill-ok`, `.pill-warn`, and `.pill-danger` are status chips. Workflow readiness uses those three states and no others.
- `.kbd` is a keyboard hint.
- `.field` is a text input, select, or textarea.
- `AppMark` shows an extracted icon when one exists, otherwise the first letter of the name. Borrowed product logos are not shipped.
- `Empty` is the state when a list has no real entries. It does not insert samples.

The Settings About section includes an interface gallery of these controls in the active theme. Appearance shows a live token preview, not a pasted screenshot.

## Type

The font stack is Segoe UI Variable, Segoe UI, then the system sans-serif. Category navigation is 228px wide and sits under a full-width header. The header keeps notices in a 4rem scroller so the page actions stay put.

## Motion and focus

Focus uses a 2px accent outline. The launcher moves the selection with Up/Down and does not wrap past either end. Enter activates the highlighted row. The highlight follows the result's identity when the list refreshes, so a reorder does not launch a different row.
