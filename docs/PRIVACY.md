# Privacy

PulseLauncher stores data on the local machine:

- the SQLite database of apps, preferences, workflow runs, and launch events
- imported preview images under the app-data `previews` directory
- an optional file index of folders the user selected

The app does not create an account, send analytics, or upload the index. Web search, when enabled, opens a DuckDuckGo URL in the default browser only after the user activates that result. Ollama and PulseFlow network calls happen only for the endpoint the user configured.

Clear history deletes launch events, workflow runs, and launch counts. It does not delete app registrations. Uninstall behavior for a future Windows installer should leave app data unless the user removes that folder; this repository has not yet verified an installed/uninstalled Windows package.
