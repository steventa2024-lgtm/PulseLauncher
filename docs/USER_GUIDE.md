# User guide

PulseLauncher is a local Windows app. It stores apps, history, and settings in a SQLite file on this PC. It does not sign you in.

## Launcher

Press `Alt+Space` (or the shortcut shown in Settings). Type to filter installed apps.

| Prefix | Meaning |
| --- | --- |
| `>` | Commands and workflows |
| `@` | Apps |
| `/` | Workflows |
| `~` | Files, only after you turn indexing on |
| `=` | Calculator |
| `?` | Web search, using the URL template in Settings |

Arrow keys move the selection. Enter runs it. Alt+Enter opens details. Escape hides the palette. A failed launch stays visible and shows the message.

The web result is last. The template must be `http` or `https` and must contain `{query}`. The default is DuckDuckGo. Choosing it opens your browser. PulseLauncher does not fetch the page.

## Apps & Tools

The dashboard lists apps after a scan or after you add one. An empty list is empty. PulseLauncher does not ship a catalog of other products.

**Rescan** reads the Start Menu. When “Scan desktop shortcuts” is on, it also reads `.lnk` files on the desktop. Packaged apps are added only when Windows returns an app user model id. A later scan updates discovered rows and marks ones that disappeared as stale. Favorites, tags, names you edited, and collections stay. The scan does not delete manual apps and does not uninstall software.

**Add app** asks for an `.exe` or a `.lnk`. Arguments are one per line. They are passed as an argument array.

Filters cover all apps, ZeroPulse, developer, productivity, favorites, recent, portable, and collections you create. Pin and drag to reorder. Hiding a discovered app removes it from PulseLauncher only.

## App details

Open a card to see the path, the command that will run, file size and version when Windows returns them, and launch history. **Launch** uses the stored target. **Run with configured args** is the same target plus the arguments saved on the entry.

**Missing target** means the file is not there. PulseLauncher will not claim the launch worked.

The Preview tab says “No preview available” until you import a PNG or JPEG. Those files are copied into the app-data folder. Nothing is downloaded.

## Commands & Workflows

Twelve templates are seeded once. A template stays **Needs setup** until its prerequisite is true (for example, Task Manager exists, or you have picked a folder). **Ready** means the check passed. It does not mean the workflow is currently running.

Configure parameters, reorder step labels, then Run. Stop cancels between steps and kills a developer-command child process. Reset restores the shipped template. You cannot delete a template id.

The runner does one workflow at a time. A confirmation step waits until you accept or decline. Developer command steps do nothing until Settings enables them, and they still ask first unless you marked that saved step trusted.

## Settings

- Appearance: dark, light, high contrast, compact density, reduced motion, accent color, text scale.
- Launcher shortcut, close the palette when it loses focus, close the main window to the tray.
- Start with Windows is off until you enable it.
- Clear history deletes launch events.
- File indexing is off until you add folders. `node_modules`, `.git`, and `target` are skipped by default.
- Backup export writes manual apps, collections, workflows, and settings. It does not write history. Import shows a preview first. It cannot turn on developer commands or autostart.
- Diagnostics and the log are local. Secrets that match the redaction rules are masked.
- Test buttons for Ollama and a PulseFlow URL run only when you press them, and only against loopback addresses you typed. They are optional.

## Quit

Use Quit in the window or the tray. Closing the main window hides it when “close to tray” is on.
