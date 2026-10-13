import { useCallback, useEffect, useState, type ReactNode } from "react";
import { Banner, Button, EmptyState, Field, controlClass } from "../components/ui";
import { api, pickPath, pickSaveJson } from "../lib/api";
import type { Collection, Diagnostics, ImportPreview, LaunchEvent, OllamaReport, PulseFlowReport, UserPreferences } from "../lib/types";
import { applyPreferences, useShell } from "../store";

export function SettingsView() {
  const preferences = useShell((state) => state.preferences);
  const setPreferences = useShell((state) => state.setPreferences);
  const notify = useShell((state) => state.notify);
  const [diagnostics, setDiagnostics] = useState<Diagnostics | null>(null);
  const [events, setEvents] = useState<LaunchEvent[]>([]);
  const [collections, setCollections] = useState<Collection[]>([]);
  const [ollama, setOllama] = useState<OllamaReport | null>(null);
  const [pulseflow, setPulseflow] = useState<PulseFlowReport | null>(null);
  const [suggestion, setSuggestion] = useState<string | null>(null);
  const [prompt, setPrompt] = useState("");
  const [importJson, setImportJson] = useState<string | null>(null);
  const [importPreview, setImportPreview] = useState<ImportPreview | null>(null);
  const [developerAck, setDeveloperAck] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [collectionName, setCollectionName] = useState("");

  const refresh = useCallback(async () => {
    try {
      const [prefs, diag, history, groups] = await Promise.all([
        api.preferencesGet(),
        api.diagnostics(),
        api.historyList(),
        api.collectionsList(),
      ]);
      setPreferences(prefs);
      applyPreferences(prefs);
      setDiagnostics(diag);
      setEvents(history.slice(0, 40));
      setCollections(groups);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Settings could not be loaded.");
    }
  }, [setPreferences]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  async function save(next: UserPreferences) {
    try {
      const saved = await api.preferencesSet(next);
      setPreferences(saved);
      applyPreferences(saved);
      if (saved.shortcutWarning) notify(saved.shortcutWarning);
      else notify("Settings saved.");
    } catch (err) {
      notify(err instanceof Error ? err.message : "Could not save settings.");
      await refresh();
    }
  }

  if (!preferences && error) {
    return <EmptyState title="Desktop settings unavailable" body={error} />;
  }
  if (!preferences) return <p className="text-sm text-muted">Loading settings…</p>;

  return (
    <section className="mx-auto max-w-3xl space-y-6">
      <header>
        <h1 className="text-2xl font-semibold">Settings & Customization</h1>
        <p className="text-sm text-muted">Preferences stay on this computer. Nothing is uploaded.</p>
      </header>
      {error ? <Banner tone="danger">{error}</Banner> : null}
      {preferences.shortcutWarning ? <Banner tone="warn">{preferences.shortcutWarning}</Banner> : null}
      {!preferences.onboardingComplete ? <Onboarding preferences={preferences} diagnostics={diagnostics} onDone={() => void save({ ...preferences, onboardingComplete: true })} /> : null}

      <Card title="Appearance">
        <div className="grid gap-3 md:grid-cols-2">
          <Field label="Theme">
            <select className={controlClass} value={preferences.theme} onChange={(event) => void save({ ...preferences, theme: event.target.value as UserPreferences["theme"] })}>
              <option value="dark">Dark blue</option>
              <option value="light">Light</option>
            </select>
          </Field>
          <Field label="Density">
            <select className={controlClass} value={preferences.density} onChange={(event) => void save({ ...preferences, density: event.target.value as UserPreferences["density"] })}>
              <option value="comfortable">Comfortable</option>
              <option value="compact">Compact</option>
            </select>
          </Field>
          <Field label="Text scale">
            <input className={controlClass} type="number" min={0.85} max={1.5} step={0.05} value={preferences.textScale} onChange={(event) => void save({ ...preferences, textScale: Number(event.target.value) })} />
          </Field>
          <Field label="App layout">
            <select className={controlClass} value={preferences.viewMode} onChange={(event) => void save({ ...preferences, viewMode: event.target.value as UserPreferences["viewMode"] })}>
              <option value="grid">Grid</option>
              <option value="list">List</option>
            </select>
          </Field>
        </div>
        <Check label="High contrast" checked={preferences.highContrast} onChange={(highContrast) => void save({ ...preferences, highContrast })} />
        <Check label="Reduce motion" checked={preferences.reduceMotion} onChange={(reduceMotion) => void save({ ...preferences, reduceMotion })} />
        <Check label="Translucent panel styling" checked={preferences.blur} onChange={(blur) => void save({ ...preferences, blur })} />
        <p className="text-xs text-muted">The blur toggle changes panel styling. This build does not claim native Windows acrylic.</p>
      </Card>

      <Card title="Launcher">
        <Field label="Global shortcut" hint={preferences.activeShortcut ? `Active: ${preferences.activeShortcut}` : "Saved after the desktop app registers it."}>
          <input className={controlClass} defaultValue={preferences.shortcut} key={preferences.shortcut} onBlur={(event) => {
            if (event.target.value !== preferences.shortcut) void save({ ...preferences, shortcut: event.target.value });
          }} />
        </Field>
        <Check label="Close launcher when it loses focus" checked={preferences.closeOnBlur} onChange={(closeOnBlur) => void save({ ...preferences, closeOnBlur })} />
        <Check label="Close dashboard to tray" checked={preferences.closeToTray} onChange={(closeToTray) => void save({ ...preferences, closeToTray })} />
        <Check label="Start with the operating system" checked={preferences.startWithOs} onChange={(startWithOs) => void save({ ...preferences, startWithOs })} />
        <p className="text-xs text-muted">Startup is off until you enable it. On Linux this writes a user autostart desktop file. On Windows it uses the current-user Run key. It does not bypass UAC.</p>
      </Card>

      <Card title="Search and indexing">
        <Check label="Search opted-in folders" checked={preferences.searchFiles} onChange={(searchFiles) => void save({ ...preferences, searchFiles })} />
        <Check label="Include desktop shortcuts in scans" checked={preferences.indexDesktopShortcuts} onChange={(indexDesktopShortcuts) => void save({ ...preferences, indexDesktopShortcuts })} />
        <Check label="Offer web search in the launcher" checked={preferences.webSearchEnabled} onChange={(webSearchEnabled) => void save({ ...preferences, webSearchEnabled })} />
        <p className="text-xs text-muted">Web search opens DuckDuckGo in your browser only after you activate that result. The home folder and drive roots cannot be indexed.</p>
        <div className="space-y-1 text-sm">
          {preferences.indexRoots.length === 0 ? <p className="text-muted">No folders are indexed.</p> : preferences.indexRoots.map((root) => (
            <div key={root} className="flex items-center justify-between gap-2">
              <span className="truncate">{root}</span>
              <Button onClick={() => void save({ ...preferences, indexRoots: preferences.indexRoots.filter((item) => item !== root) })}>Remove</Button>
            </div>
          ))}
        </div>
        <Button onClick={() => void pickPath(true).then((path) => {
          if (!path || preferences.indexRoots.includes(path)) return;
          return save({ ...preferences, indexRoots: [...preferences.indexRoots, path] });
        }).catch((err: Error) => notify(err.message))}>Add folder</Button>
        <Button onClick={() => void api.indexRun().then((count) => notify(`Indexed ${count} files.`)).catch((err: Error) => notify(err.message))}>Build index</Button>
      </Card>

      <Card title="Collections">
        <div className="flex gap-2">
          <input aria-label="New collection name" className={controlClass} value={collectionName} onChange={(event) => setCollectionName(event.target.value)} />
          <Button onClick={() => void api.collectionsCreate(collectionName).then(() => { setCollectionName(""); return refresh(); }).catch((err: Error) => notify(err.message))}>Create</Button>
        </div>
        <ul className="space-y-2 text-sm">
          {collections.map((collection) => (
            <li key={collection.id} className="flex items-center justify-between gap-2">
              <span>{collection.name} · {collection.appIds.length}</span>
              <Button disabled={collection.builtIn} onClick={() => void api.collectionsDelete(collection.id).then(refresh).catch((err: Error) => notify(err.message))}>Delete</Button>
            </li>
          ))}
        </ul>
      </Card>

      <Card title="Developer commands">
        <p className="text-sm text-muted">Off by default. Enabling allows saved workflows to run a program and argument array you configured. Search text is never executed.</p>
        <Check label="I understand saved developer commands can start programs I configure" checked={developerAck || preferences.developerCommandsEnabled} onChange={setDeveloperAck} />
        <Check label="Enable developer command steps" checked={preferences.developerCommandsEnabled} onChange={(developerCommandsEnabled) => {
          if (developerCommandsEnabled && !developerAck && !preferences.developerCommandsEnabled) {
            notify("Confirm the acknowledgement before enabling developer commands.");
            return;
          }
          void save({ ...preferences, developerCommandsEnabled });
        }} />
      </Card>

      <Card title="ZeroPulse and local AI">
        <Field label="OpenPulse executable or https URL" hint="Leave empty if you do not use OpenPulse. No port or token is assumed.">
          <input className={controlClass} value={preferences.openpulseTarget} onChange={(event) => setPreferences({ ...preferences, openpulseTarget: event.target.value })} onBlur={() => { const current = useShell.getState().preferences; if (current) void save(current); }} />
        </Field>
        <Button onClick={() => void api.openpulseLaunch().then((outcome) => notify(outcome.message)).catch((err: Error) => notify(err.message))}>Launch configured OpenPulse target</Button>
        <Field label="PulseFlow executable" hint="Optional. PulseLauncher launches this file only. It does not invent an API.">
          <input className={controlClass} value={preferences.pulseflowExecutable} onChange={(event) => setPreferences({ ...preferences, pulseflowExecutable: event.target.value })} onBlur={() => { const current = useShell.getState().preferences; if (current) void save(current); }} />
        </Field>
        <Field label="PulseFlow endpoint" hint="Optional localhost URL. A test reports the HTTP status it actually receives.">
          <input className={controlClass} value={preferences.pulseflowEndpoint} onChange={(event) => setPreferences({ ...preferences, pulseflowEndpoint: event.target.value })} onBlur={() => { const current = useShell.getState().preferences; if (current) void save(current); }} />
        </Field>
        <Button onClick={() => void api.pulseflowTest().then(setPulseflow).catch((err: Error) => notify(err.message))}>Test PulseFlow</Button>
        {pulseflow ? <Banner tone={pulseflow.reachable ? "ok" : "info"}>{pulseflow.message}</Banner> : null}
        <Check label="Enable local Ollama suggestions" checked={preferences.ollamaEnabled} onChange={(ollamaEnabled) => void save({ ...preferences, ollamaEnabled })} />
        <Field label="Ollama endpoint">
          <input className={controlClass} value={preferences.ollamaEndpoint} onChange={(event) => setPreferences({ ...preferences, ollamaEndpoint: event.target.value })} onBlur={() => { const current = useShell.getState().preferences; if (current) void save(current); }} />
        </Field>
        <Field label="Model name">
          <input className={controlClass} value={preferences.ollamaModel} onChange={(event) => setPreferences({ ...preferences, ollamaModel: event.target.value })} onBlur={() => { const current = useShell.getState().preferences; if (current) void save(current); }} />
        </Field>
        <Button onClick={() => void api.ollamaTest().then(setOllama).catch((err: Error) => notify(err.message))}>Test Ollama</Button>
        {ollama ? <Banner tone={ollama.reachable ? "ok" : "warn"}>{ollama.message}{ollama.models.length ? ` Models: ${ollama.models.join(", ")}` : ""}</Banner> : null}
        <Field label="Ask for an explanation" hint="The reply is text only. PulseLauncher will not run it.">
          <textarea className={controlClass} value={prompt} onChange={(event) => setPrompt(event.target.value)} />
        </Field>
        <Button disabled={!preferences.ollamaEnabled} onClick={() => void api.ollamaSuggest(prompt).then((result) => {
          setSuggestion(result.executed ? "Unexpected executed flag." : result.text || result.message);
        }).catch((err: Error) => notify(err.message))}>Suggest</Button>
        {suggestion ? <Banner>Not executed. {suggestion}</Banner> : null}
      </Card>

      <Card title="Backup and history">
        <div className="flex flex-wrap gap-2">
          <Button onClick={() => void pickSaveJson("pulslauncher-backup.json").then(async (path) => {
            if (!path) return;
            await api.backupWrite(path, false);
            notify("Backup written without history.");
          }).catch((err: Error) => notify(err.message))}>Export settings</Button>
          <Button onClick={() => void pickPath(false, [{ name: "JSON", extensions: ["json"] }]).then(async (path) => {
            if (!path) return;
            const json = await api.backupRead(path);
            setImportJson(json);
            setImportPreview(await api.backupPreview(json));
          }).catch((err: Error) => notify(err.message))}>Preview import</Button>
          <Button variant="danger" onClick={() => void api.historyClear().then(refresh).then(() => notify("Launch history cleared.")).catch((err: Error) => notify(err.message))}>Clear history</Button>
        </div>
        {importPreview && importJson ? (
          <div className="space-y-2">
            <Banner>
              Import would apply {importPreview.apps} apps, {importPreview.collections} collections, {importPreview.workflows} workflows. History in the file is ignored.
              {importPreview.warnings.length ? ` ${importPreview.warnings.join(" ")}` : ""}
            </Banner>
            <Button variant="primary" onClick={() => void api.backupApply(importJson).then(() => { setImportJson(null); setImportPreview(null); return refresh(); }).catch((err: Error) => notify(err.message))}>Apply import</Button>
          </div>
        ) : null}
      </Card>

      <Card title="Diagnostics">
        {diagnostics ? (
          <div className="space-y-1 text-sm">
            <p>Version {diagnostics.version} · {diagnostics.os}</p>
            <p>Data {diagnostics.dataDir}</p>
            <p>Database {diagnostics.dbPath}</p>
            <p>Apps {diagnostics.visibleAppCount} visible / {diagnostics.appCount} stored</p>
            <p>Last scan {diagnostics.lastScanAt ?? "not recorded"}{diagnostics.lastScanError ? ` · ${diagnostics.lastScanError}` : ""}</p>
            <p>Index {diagnostics.indexCount} files{diagnostics.lastIndexAt ? ` · ${diagnostics.lastIndexAt}` : ""}</p>
            <ul className="list-disc pl-5 text-muted">{diagnostics.notes.map((note) => <li key={note}>{note}</li>)}</ul>
          </div>
        ) : <p className="text-sm text-muted">Diagnostics appear when the desktop bridge is connected.</p>}
        <h3 className="pt-2 text-sm font-semibold">Recent log</h3>
        {events.length === 0 ? <p className="text-sm text-muted">No launch events stored.</p> : (
          <ul className="max-h-64 space-y-1 overflow-auto text-xs text-muted">
            {events.map((event) => <li key={event.id}>{event.createdAt} · {event.outcome} · {event.message}</li>)}
          </ul>
        )}
        <p className="text-xs text-muted">Updates are not downloaded automatically. Release files, when published by the project, are optional HTTPS downloads. This build is unsigned unless a maintainer adds signing later. SmartScreen warnings cannot be bypassed from the app.</p>
      </Card>
    </section>
  );
}

function Card({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="space-y-3 rounded-card border border-line bg-panel p-4">
      <h2 className="text-lg font-semibold">{title}</h2>
      {children}
    </section>
  );
}

function Check({ label, checked, onChange }: { label: string; checked: boolean; onChange: (value: boolean) => void }) {
  return (
    <label className="flex items-center gap-2 text-sm">
      <input type="checkbox" checked={checked} onChange={(event) => onChange(event.target.checked)} />
      {label}
    </label>
  );
}

function Onboarding({
  preferences,
  diagnostics,
  onDone,
}: {
  preferences: UserPreferences;
  diagnostics: Diagnostics | null;
  onDone: () => void;
}) {
  const setView = useShell((state) => state.setView);
  return (
    <Card title="First run">
      <p className="text-sm text-muted">PulseLauncher works offline. There is no account. The shortcut is {preferences.activeShortcut || preferences.shortcut}. If that key is taken, Settings shows the fallback.</p>
      <p className="text-sm text-muted">{diagnostics ? `${diagnostics.visibleAppCount} visible apps in the local database.` : "App counts appear after the shell finishes its first scan."}</p>
      {diagnostics?.lastScanError ? <Banner tone="warn">{diagnostics.lastScanError}</Banner> : null}
      <div className="flex flex-wrap gap-2">
        <Button variant="primary" onClick={onDone}>Continue</Button>
        <Button onClick={() => { onDone(); setView("apps"); }}>Add an app</Button>
      </div>
    </Card>
  );
}
