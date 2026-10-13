# Privacy

PulseLauncher stores its data on the computer where it is installed. There is no account, no telemetry, and no hosted API.

## What is stored

- Apps you add or that a scan finds: names, paths, arguments, tags, favorites, and launch counts
- Workflows, their configuration, and the last runs (the run log keeps 30)
- Preferences, including the hotkey and any Ollama or PulseFlow address you type
- An optional file index of folders you list
- Preview images you import, copied into the app data folder
- Launch history, if "Remember launch history" is on

Diagnostics in Settings shows the data directory and the database path. Clearing history deletes launch events, workflow runs, and the file index. It does not delete apps or workflows.

## What leaves the machine

Nothing leaves by default.

- Web search is off. If you enable it, choosing a web result opens your browser to the search template with the query filled in.
- Ollama is off. If you enable it, requests go to the endpoint you set (default `http://127.0.0.1:11434`).
- PulseFlow is blank. A check is sent only to a loopback URL you enter.
- Startup registration uses the operating system's own autostart mechanism when you turn that setting on.

File indexing does not run until you enable it and provide roots. Default exclusions include `node_modules`, `.git`, `target`, and `AppData`.

## Export

Export copies a JSON backup to the clipboard. It contains preferences, custom apps, collections, and workflows. Treat that JSON as private. Import previews the change before it applies, revalidates preferences, and marks imported workflows untrusted.
