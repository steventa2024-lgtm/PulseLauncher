# Security

## Reporting

Report vulnerabilities privately to the maintainers of the ZeroPulse repository. Do not open a public issue with exploit detail.

## Boundaries

- The webview receives named commands such as `command_apps_launch` and `command_workflows_start`. There is no `execute(string)` bridge and the Tauri shell plugin is not enabled.
- Launch uses a stored app id, a validated URL, a known folder, or a previously registered program. Search text is never interpolated into a shell command.
- Developer command steps are stored as a program plus an argument array and run only when the user enables them. They default to off.
- URL schemes are limited to `http`, `https`, and `ms-settings`. Credentials in URLs are rejected.
- Index roots cannot be a filesystem root or the user's home directory. Depth and file count are capped.
- Preview import accepts PNG, JPG, WEBP, and GIF under 8 MB and stores the copy under app data. Live screen capture is not implemented.
- Workflow import rejects documents that fail validation, including bare `javascript:` URLs. Import size is capped at 2 MB.
- Logs pass through a redaction pass for common token and password patterns. That is not a guarantee against every secret shape.
- Startup with the OS is opt-in. It writes the current-user Run key on Windows or a user autostart desktop file on Linux. It does not elevate.
- No telemetry is sent. Ollama and PulseFlow are contacted only after the user enables or configures them.

## Threats considered

Malicious `.lnk` files are parsed as data. They are not launched during discovery. A shortcut whose target is missing is marked missing. Untrusted workflow JSON cannot add a free-form shell string unless the user has enabled developer commands and the step names a program. Path traversal in backup paths is rejected. Hiding a discovered app does not uninstall it. Deleting a manual entry removes only the PulseLauncher row.

Windows Credential Manager is not used because the core app stores no credentials. If a future plugin needs a secret, it should use DPAPI rather than the preferences JSON.
