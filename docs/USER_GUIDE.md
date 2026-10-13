# User guide

## Open the dashboard

The dashboard lists apps, workflows, and settings. The first time, a short note explains that data stays on this computer. Continue dismisses it.

If a scan has not found anything, the apps page says so. Use Add app for a program, a `.lnk`, or an approved link such as `ms-settings:`. Browse uses the system file picker. Arguments are stored as a list. Quotes keep one argument together.

Rescan reads the Start Menu on Windows. It updates names and paths and keeps your favorites, tags, and custom names. Removing a discovered app hides it. Removing an app you added deletes that registration. Neither one uninstalls the program.

Right-click an app for Run, Details, Favorite, Copy path, Move up, and Move down. The grip on a card drags that app onto another card in the current list. Arrow keys on the grip do the same move. Apps hidden by the filter stay where they are.

## Details

Details shows the target, arguments, publisher and version when the file has them, collections, and launch history. "Not available" means the file did not contain that field.

The Preview tab starts at "No preview available". Import screenshot copies a png, jpg, gif, webp, or bmp you already made (up to 8 MB) into PulseLauncher's data. Remove preview deletes that copy.

## Launcher

The default shortcut is Alt+Space. If registration fails, the fallback (Ctrl+Alt+Space) is tried and Settings reports the result.

Type to search. Up and Down move the highlight. Enter launches or runs the highlighted row. Escape hides the overlay. Ctrl+K inserts the command prefix.

| Prefix | Limits the search to |
| --- | --- |
| `@` | Apps |
| `>` | Commands |
| `/` | Workflows |
| `#` | Indexed files, after you turn file search on |
| `=` | Calculator |
| `?` | Web search, after you turn it on |

An empty box shows favorites and recent apps. It does not invent any.

## Workflows

Each built-in command shows Ready, Needs setup, or Unavailable, plus the reason. Run is disabled until the state is Ready. Configure saves the folder, app, or URL that template asks for. Reset puts a built-in template back. Your own workflows can be deleted. Built-in ones cannot.

A step that asks you to confirm waits until you continue or decline. The question appears in the dashboard and in the search window, and answering it in either one closes the other. Decline stops the run. If a run is in progress, Stop requests cancellation.

Editing steps can add a notification, a wait, or an "open folder" step, and can reorder or remove steps. A workflow that contains a developer command is not trusted until you turn on developer commands in Settings and choose Trust command.

## Settings

Appearance, the shortcut, file index roots, history, and the developer-command switch are saved with Save settings. Rebuild file index walks only the folders you listed. Clear history removes launch events, workflow logs, and the file index.

Ollama and PulseFlow stay off until you fill them in. Ask shows the model reply and does not run it. Check PulseFlow address only contacts a localhost URL.

Copy export puts a backup on the clipboard. Preview import before Apply.
