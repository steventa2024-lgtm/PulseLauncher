# Design system

The interface uses the selected dark-blue studio direction. Tokens live in `src/index.css` and are mapped into Tailwind in `tailwind.config.js`.

| Token | Dark value | Use |
| --- | --- | --- |
| `--bg` | `#090D17` | Window background |
| `--bg-elevated` | `#111827` | Sidebar and dialogs |
| `--panel` | `#151D2E` | Cards |
| `--border` | `#26344A` | Hairline borders |
| `--accent` | `#4388FF` | Focus and icons |
| `--accent-bright` | `#5BA8FF` | Highlights |
| `--accent-button` | `#8EBEFF` | Primary button fill |
| `--accent-ink` | `#07111F` | Primary button text |
| `--text` | `#F4F7FB` | Primary text |
| `--muted` | `#AABAD1` | Secondary text |

White text on `#4388FF` is not used for small labels. Primary buttons use dark ink on `#8EBEFF`. In the light theme the primary button is `#1D4ED8` with light text.

Radius is 12px. The dashboard sidebar is 220px and collapses into a horizontal bar under 860px so an 800×600 window still scrolls instead of clipping. The launcher is placed near the top of the current monitor, about 720px wide, and shrinks when the monitor is narrower.

Motion is limited to color and opacity. `reduceMotion` and `prefers-reduced-motion` remove transitions. High contrast thickens the border token and raises secondary text contrast.

The in-app Design gallery shows buttons, status badges, empty states, and a sample card. That card is labeled as a specimen and does not represent an installed program.

Icons in the interface are Lucide (ISC). The application icon is original.
