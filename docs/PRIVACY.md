# Privacy

PulseLauncher stores data on this PC only.

Stored locally, under the per-user app data directory:

- App registrations, tags, favorites, collections
- Launch history and workflow runs, if "Record launch history" is on
- Settings
- Icon cache and screenshots you import
- A local log (`pulse.log`), with secret-shaped strings redacted

Not collected: telemetry, crash uploads, account identifiers, or a silent index of your home drive.

File indexing starts only after you turn it on and list folders. Web search, Ollama, and endpoint tests run only when you invoke them. History can be deleted from Settings; that removes launch events and workflow runs.

Backups contain manual registrations, collections, workflows, and preferences. They do not contain launch history. Import refuses to turn on developer commands or start-with-Windows.
