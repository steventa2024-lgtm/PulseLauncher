import { useEffect, useState } from "react";
import * as api from "../../lib/api";
import { applyPrefs, useUi } from "../../lib/store";
import type { Diagnostics, IntegrationSlot, OllamaStatus, UserPreferences } from "../../lib/types";

const fallback: UserPreferences = {
  theme: "dark",
  accent: "#4388FF",
  density: "comfortable",
  reduceMotion: false,
  translucency: true,
  textScale: 1,
  highContrast: false,
  launcherShortcut: "Alt+Space",
  closeOnBlur: true,
  closeToTray: true,
  startWithWindows: false,
  showWebSearch: true,
  webSearchUrl: "https://duckduckgo.com/?q={query}",
  indexFiles: false,
  indexRoots: [],
  indexExclusions: ["node_modules", ".git", "target"],
  recordHistory: true,
  developerCommandExecution: false,
  onboardingDone: false,
  desktopShortcutsScan: false,
  ollamaEnabled: false,
  ollamaEndpoint: "http://127.0.0.1:11434",
  ollamaModel: "",
  pulseflowEndpoint: "",
  openpulseUrl: "",
};

export function SettingsView({ initial }: { initial?: UserPreferences }) {
  const setPrefs = useUi((s) => s.setPrefs);
  const setNotice = useUi((s) => s.setNotice);
  const [prefs, setLocal] = useState<UserPreferences>(initial ?? fallback);
  const [diag, setDiag] = useState<Diagnostics | null>(null);
  const [log, setLog] = useState("");
  const [ollama, setOllama] = useState<OllamaStatus | null>(null);
  const [slots, setSlots] = useState<IntegrationSlot[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [backupPreview, setBackupPreview] = useState<string>("");
  const [importJson, setImportJson] = useState("");

  useEffect(() => {
    if (initial) {
      applyPrefs(initial);
      return;
    }
    void api.getPreferences().then((p) => {
      setLocal(p);
      setPrefs(p);
      applyPrefs(p);
    }).catch((e) => setError(api.errorText(e)));
    void api.diagnostics().then(setDiag).catch(() => undefined);
    void api.integrationStatus(false).then(setSlots).catch(() => undefined);
  }, []);

  function edit(patch: Partial<UserPreferences>) {
    const next = { ...prefs, ...patch };
    setLocal(next);
    applyPrefs(next);
  }

  async function save() {
    setError(null);
    try {
      const saved = await api.setPreferences(prefs);
      setLocal(saved);
      setPrefs(saved);
      applyPrefs(saved);
      setNotice("Settings saved.");
      setDiag(await api.diagnostics());
    } catch (e) {
      setError(api.errorText(e));
    }
  }

  return (
    <div>
      <div className="topbar">
        <h1>Settings</h1>
        <button className="btn" onClick={() => void save()}>Save</button>
      </div>
      <div className="content settings">
        {error && <p role="alert">{error}</p>}
        <section className="card">
          <h2>Appearance</h2>
          <label className="setting">Theme
            <select aria-label="Theme" className="field" style={{ maxWidth: 180 }} value={prefs.theme} onChange={(e) => edit({ theme: e.target.value })}>
              <option value="dark">Dark blue</option>
              <option value="light">Light</option>
            </select>
          </label>
          <label className="setting">Accent
            <input aria-label="Accent" className="field" style={{ maxWidth: 140 }} value={prefs.accent} onChange={(e) => edit({ accent: e.target.value })} />
          </label>
          <label className="setting">Density
            <select aria-label="Density" className="field" style={{ maxWidth: 180 }} value={prefs.density} onChange={(e) => edit({ density: e.target.value })}>
              <option value="comfortable">Comfortable</option>
              <option value="compact">Compact</option>
            </select>
          </label>
          <label className="setting">Text scale
            <input aria-label="Text scale" type="number" min={0.85} max={1.6} step={0.05} className="field" style={{ maxWidth: 100 }} value={prefs.textScale} onChange={(e) => edit({ textScale: Number(e.target.value) })} />
          </label>
          <label className="setting"><span>Reduce motion</span><input aria-label="Reduce motion" type="checkbox" checked={prefs.reduceMotion} onChange={(e) => edit({ reduceMotion: e.target.checked })} /></label>
          <label className="setting"><span>High contrast</span><input aria-label="High contrast" type="checkbox" checked={prefs.highContrast} onChange={(e) => edit({ highContrast: e.target.checked })} /></label>
        </section>
        <section className="card">
          <h2>Launcher</h2>
          <label className="setting">Shortcut
            <input aria-label="Launcher shortcut" className="field" style={{ maxWidth: 200 }} value={prefs.launcherShortcut} onChange={(e) => edit({ launcherShortcut: e.target.value })} />
          </label>
          <p className="tiny">If the shortcut is taken, PulseLauncher tries Ctrl+Alt+Space and tells you. It does not replace the other program.</p>
          {diag?.shortcut.error && <p role="status">{diag.shortcut.error}</p>}
          {diag?.shortcut.active && <p className="tiny">Active shortcut: {diag.shortcut.active}</p>}
          <label className="setting"><span>Close launcher when it loses focus</span><input aria-label="Close on blur" type="checkbox" checked={prefs.closeOnBlur} onChange={(e) => edit({ closeOnBlur: e.target.checked })} /></label>
          <label className="setting"><span>Close window to tray</span><input aria-label="Close to tray" type="checkbox" checked={prefs.closeToTray} onChange={(e) => edit({ closeToTray: e.target.checked })} /></label>
          <label className="setting"><span>Start with Windows</span><input aria-label="Start with Windows" type="checkbox" checked={prefs.startWithWindows} onChange={(e) => edit({ startWithWindows: e.target.checked })} /></label>
          <label className="setting"><span>Show web search results</span><input aria-label="Web search" type="checkbox" checked={prefs.showWebSearch} onChange={(e) => edit({ showWebSearch: e.target.checked })} /></label>
          <label className="setting"><span>Record launch history</span><input aria-label="Record history" type="checkbox" checked={prefs.recordHistory} onChange={(e) => edit({ recordHistory: e.target.checked })} /></label>
          <button className="btn-danger" onClick={() => void api.clearHistory().then((n) => setNotice(`Removed ${n} history rows.`)).catch((e) => setError(api.errorText(e)))}>Clear history</button>
        </section>
        <section className="card">
          <h2>Indexing</h2>
          <p className="tiny">File search stays off until you add folders. PulseLauncher never walks your whole drive on its own.</p>
          <label className="setting"><span>Search chosen folders</span><input aria-label="Index files" type="checkbox" checked={prefs.indexFiles} onChange={(e) => edit({ indexFiles: e.target.checked })} /></label>
          <label className="tiny">Folders, one absolute path per line</label>
          <textarea className="field" aria-label="Index roots" rows={3} value={prefs.indexRoots.join("\n")} onChange={(e) => edit({ indexRoots: e.target.value.split(/\r?\n/).map((s) => s.trim()).filter(Boolean) })} />
          <label className="setting"><span>Also read desktop shortcuts</span><input aria-label="Desktop shortcuts" type="checkbox" checked={prefs.desktopShortcutsScan} onChange={(e) => edit({ desktopShortcutsScan: e.target.checked })} /></label>
        </section>
        <section className="card">
          <h2>Security</h2>
          <label className="setting">
            <span>Allow developer command steps (off by default)</span>
            <input aria-label="Developer command execution" type="checkbox" checked={prefs.developerCommandExecution} onChange={(e) => edit({ developerCommandExecution: e.target.checked })} />
          </label>
          <p className="tiny">Even when this is on, a command step shows the exact program and arguments and asks before it runs, unless you marked that saved step trusted yourself.</p>
        </section>
        <section className="card">
          <h2>ZeroPulse hooks</h2>
          <p className="tiny">These only launch programs you registered, or contact an address you typed. Empty means not set up.</p>
          {slots.map((slot) => (
            <p key={slot.label}>
              <strong>{slot.label}:</strong> {slot.registeredName ? `registered as ${slot.registeredName}` : "not registered"} — {slot.detail}
            </p>
          ))}
          <label className="tiny">PulseFlow localhost URL (optional)</label>
          <input className="field" aria-label="PulseFlow endpoint" value={prefs.pulseflowEndpoint} onChange={(e) => edit({ pulseflowEndpoint: e.target.value })} />
          <label className="tiny">OpenPulse URL (optional)</label>
          <input className="field" aria-label="OpenPulse URL" value={prefs.openpulseUrl} onChange={(e) => edit({ openpulseUrl: e.target.value })} />
          <button className="btn-ghost" onClick={() => void api.integrationStatus(true).then(setSlots).catch((e) => setError(api.errorText(e)))}>Test saved endpoints</button>
        </section>
        <section className="card">
          <h2>Local AI (optional)</h2>
          <label className="setting"><span>Use Ollama on this PC</span><input aria-label="Enable Ollama" type="checkbox" checked={prefs.ollamaEnabled} onChange={(e) => edit({ ollamaEnabled: e.target.checked })} /></label>
          <input className="field" aria-label="Ollama endpoint" value={prefs.ollamaEndpoint} onChange={(e) => edit({ ollamaEndpoint: e.target.value })} />
          <input className="field" aria-label="Ollama model" placeholder="Model name" value={prefs.ollamaModel} onChange={(e) => edit({ ollamaModel: e.target.value })} />
          <button className="btn-ghost" onClick={() => void api.ollamaStatus().then(setOllama).catch((e) => setError(api.errorText(e)))}>Test connection</button>
          {ollama && <p role="status">{ollama.message}</p>}
          <p className="tiny">Explanations never run a command. You still press Run yourself.</p>
        </section>
        <section className="card">
          <h2>Backup</h2>
          <button className="btn-ghost" onClick={() => void api.exportBackup().then(setBackupPreview).catch((e) => setError(api.errorText(e)))}>Export</button>
          {backupPreview && <textarea className="field" aria-label="Export JSON" readOnly rows={6} value={backupPreview} />}
          <textarea className="field" aria-label="Import JSON" rows={4} value={importJson} onChange={(e) => setImportJson(e.target.value)} placeholder="Paste a PulseLauncher backup" />
          <button className="btn-ghost" onClick={() => void api.previewBackup(importJson).then((p) => setNotice(`Preview: ${p.newApps} new apps, ${p.updatedApps} updated. Nothing written yet. Warnings: ${p.warnings.join(" ")}`)).catch((e) => setError(api.errorText(e)))}>Preview import</button>
          <button className="btn" onClick={() => void api.applyBackup(importJson).then(() => setNotice("Import applied.")).catch((e) => setError(api.errorText(e)))}>Apply import</button>
        </section>
        <section className="card">
          <h2>Diagnostics</h2>
          {diag ? (
            <ul className="tiny">
              <li>Version {diag.version}</li>
              <li>Platform {diag.platform}</li>
              <li>Data {diag.dataDir}</li>
              <li>Database {diag.dbPath} (schema {diag.schemaVersion})</li>
              <li>Apps {diag.appCount}; indexed files {diag.indexedFiles}</li>
              <li>Packaged apps: {diag.packagedAppsSupported ? "supported on this OS" : "not available on this OS"}</li>
            </ul>
          ) : (
            <p className="tiny">Diagnostics load from the desktop app.</p>
          )}
          <button className="btn-ghost" onClick={() => void api.readLog().then(setLog).catch((e) => setError(api.errorText(e)))}>Show log</button>
          {log && <pre className="log">{log || "(empty)"}</pre>}
          <p className="tiny">Updates are not downloaded automatically. Releases, when published, are the open-source repository artifacts. Unsigned Windows installers can show a SmartScreen warning; PulseLauncher does not try to hide that.</p>
          <p className="tiny">No account, telemetry, or subscription is required. The app works offline.</p>
        </section>
      </div>
    </div>
  );
}
