# Design system

The UI is an original dark-blue desktop theme. Tokens live in `src/styles.css` and are applied as CSS variables so theme, contrast, density, and text scale can change without a second stylesheet.

| Token | Dark value | Use |
| --- | --- | --- |
| `--bg` | `#090D17` | Window background |
| `--bg-2` | `#111827` | Gradient end |
| `--panel` | `#151D2E` | Cards, palette field |
| `--border` | `#26344A` | Hairline borders |
| `--accent` | `#4388FF` | Primary button, default accent |
| `--accent-2` | `#5BA8FF` | Focus and match highlight |
| `--text` | `#F4F7FB` | Primary text |
| `--muted` | `#AABAD1` | Secondary text |
| `--radius` | `12px` | Cards and the palette |

Light theme, high contrast, compact density, reduced motion, and text scale are set on `<html>` from Settings (`data-theme`, `data-contrast`, `data-density`, `data-motion`, `--text-scale`). Focus uses a 2px accent ring. Buttons that cannot run stay disabled or say what is missing. Empty states say that no record exists; they do not show sample applications.

The in-app gallery (`?view=gallery`, or Gallery in the sidebar) renders the same button, chip, and empty-state classes the product uses.

Layout: dashboard sidebar about 220px, scrollable content, minimum window 800×600. The palette window is 720×540, placed near the top of the current monitor when the hotkey runs on a real desktop session.

Icons in the shell are the original mark under `src-tauri/icons`. App icons in the grid come from the OS extractor when it returns bytes; otherwise the card shows the first letter of the name.
