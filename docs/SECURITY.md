# Security

## Trust boundary

The webview is untrusted. It may only call the commands registered in `src-tauri/src/lib.rs`. There is no `execute(string)` command. Capabilities in `src-tauri/capabilities/default.json` grant window events, the file dialog, and notifications. They do not grant a shell plugin.

## Launch

- Registered apps launch by id. The backend loads the stored target and calls `ShellExecuteExW` (or `IApplicationActivationManager` for a packaged app). Shortcut files are opened as shortcuts so their own arguments stay intact.
- Extra arguments are appended as an array for executables. They are not concatenated into a `cmd` string.
- A target must be an absolute existing `.exe` (or `.lnk` for a shortcut) before a launch is attempted. A missing file returns `missingTarget` and is recorded that way.
- Discovery and icon extraction only read bytes. `ExtractIconExW` / the `.lnk` parser do not run the file.
- Developer command steps are off in settings. When enabled, the step still shows the program and argument list and asks for confirmation unless that saved step was already marked trusted by the user. Imports clear the trusted flag.

## Workflows

Templates and imports are validated: step count, nesting, argument length, URL scheme, and path shape. Variable substitution does not rescan inserted values, so a parameter cannot inject `{{env:...}}`. Environment variables exposed to templates are an allow-list (`SystemRoot`, `LOCALAPPDATA`, and the other names in `workflow/engine.rs`). Logs pass through `security::redact` before they are stored.

Destructive file operations are not part of the action set. Creating a note uses `create_new` and a unique name, so an existing file is not overwritten. Path traversal in the file name is rejected.

## Network

No request is made at startup. Web search opens the validated URL in the default browser only after the user chooses that result. Ollama and PulseFlow probes require a loopback `http`/`https` URL. An update checker is not implemented.

## Threats considered

| Input | Handling |
| --- | --- |
| Malicious `.lnk` | Parser is size-capped, rejects a bad header, and never resolves via the shell during scan |
| Impersonated executable | User sees the exact path and command preview before and after launch |
| Workflow JSON | Schema version, size cap, and `validate_definition` before save |
| Shell metacharacters in names | Argument arrays and Windows quoting; no shell |
| Path traversal in note names | `safe_join` rejects `..`, separators, and device names |
| Remote URLs | `http`/`https` only; credentials in the URL are rejected |
| Secret-looking log lines | Redaction of common tokens, `key=value` secrets, and URL userinfo |

## Autostart

Start-with-Windows is off until the user saves that setting. The autostart plugin is then enabled or disabled to match the stored preference. Closing the window hides to the tray when that preference is on. Quit exits the process.
