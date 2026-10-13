# Security

PulseLauncher launches programs the user already has. That is a powerful action, so the dangerous paths are explicit and the defaults are closed.

## Threat model

| Threat | What the code does |
| --- | --- |
| Malicious `.lnk` | Parsed as MS-SHLLINK. The parser does not execute the target. Truncation and oversized files (over 1 MB) fail the parse. A shortcut is launched as the `.lnk` itself so Windows, not PulseLauncher, resolves its target. |
| Impersonated executable | The catalog stores the path it found or the path the user typed. Icons and version strings are read from the PE resources only. They are display data, not a trust decision. |
| Workflow JSON | Import is size-capped (256 KB parse, 2 MB backup). Values are revalidated. `javascript:` and other non-http(s) web URLs are rejected. Imported workflows are untrusted. Template rows update values only when those values validate. |
| Shell metacharacters | Launch arguments are a list. `run_captured` uses `Command::new` with that list, never a shell. Output redacts password, token, and common secret prefixes. |
| Path traversal | Absolute paths reject `..` segments on both `/` and `\`. Preview deletion canonicalizes the file and deletes it only when it is inside `data_dir/previews`. Notes must stay inside the chosen folder and never overwrite an existing file. |
| Fake metadata | Missing version or publisher is shown as unavailable. |
| URL and URI tricks | Web search and Open URL accept http(s) only, with no embedded credentials. `javascript`, `file`, `ms-msdt`, `search-ms`, `ms-appinstaller`, and `shell` are blocked. `ms-settings:` is checked separately. Opening a file refuses executables, scripts, and shortcuts. |
| Argument injection from search | There is no command that evaluates search text. Calculator results are copied. Workflow steps do not re-expand substituted values. |
| Developer commands | Off by default. `runCommand` requires the setting and a separate trust action. Saving a definition that contains `runCommand` clears trust. |
| Prompt or suggestion injection | Ollama text is shown and marked not executed. PulseFlow URLs that are not loopback are rejected. An empty PulseFlow URL is "not configured". |
| Frontend compromise | The webview capability list does not include a shell, fs, or process plugin. CSP `connect-src` is limited to Tauri IPC. |

## Reporting

If you find a way to launch something the user did not choose, escape the preview or note directory, or execute workflow text as a shell command, report it privately to the maintainers with the command name and a minimal workflow JSON. Please avoid including live secrets in the report; the redaction helper is not a promise that every secret shape is caught.

## Limits

A process id from `ShellExecuteExW` means Windows accepted the launch. It does not mean the program stayed running. Global hotkey registration can succeed while another program still consumes the key. PulseLauncher reports the registration result and does not silently rewrite the saved shortcut.

This document describes the source. The Windows shell path has been type-checked for `x86_64-pc-windows-gnu` and has not been executed on Windows in this environment.
