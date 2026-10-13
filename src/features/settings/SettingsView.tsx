import { useEffect, useState } from "react";
import { api, errorText } from "../../lib/api";
import { formatWhen } from "../../lib/text";
import type { Diagnostics, UserPreferences } from "../../lib/types";
import { Field } from "../../components/ui";
import { SidebarItem, ToggleSwitch } from "../../components/chrome";
import { PulseBrand } from "../../components/chrome";

export function SettingsView({ prefs, onPrefs, onNotice }: { prefs: UserPreferences; onPrefs: (p: UserPreferences) => void; onNotice: (t: string, b: string) => void }) {
  const [draft, setDraft] = useState(prefs);
  const [diag, setDiag] = useState<Diagnostics | null>(null);
  const [diagError, setDiagError] = useState("");
  const [ollama, setOllama] = useState("");
  const [pulse, setPulse] = useState("");
  const [suggest, setSuggest] = useState("");
  const [question, setQuestion] = useState("");
  const [importText, setImportText] = useState("");
  const [importNotes, setImportNotes] = useState<string[]>([]);
  const [section, setSection] = useState("appearance");
  const sections = ["General", "Appearance", "Search", "Apps & Paths", "Commands", "AI Integration", "Extensions", "Hotkeys", "Backup & Sync", "About"];
  useEffect(() => setDraft(prefs), [prefs]);
  useEffect(() => { api.diagnostics().then(setDiag).catch((e) => setDiagError(errorText(e))); }, []);

  function set<K extends keyof UserPreferences>(key: K, value: UserPreferences[K]) {
    setDraft({ ...draft, [key]: value });
  }

  async function live(partial: Partial<UserPreferences>) {
    const next = { ...draft, ...partial };
    setDraft(next);
    onPrefs(next);
    try { await api.savePreferences(next); }
    catch (e) { onNotice("Settings", errorText(e)); }
  }

  async function save() {
    try {
      const status = await api.savePreferences(draft);
      onPrefs(draft);
      onNotice("Settings saved", status.warning ?? (status.active ? `Shortcut ${status.active} is active.` : "Shortcut is not registered."));
      setDiag(await api.diagnostics());
    } catch (e) { onNotice("Settings", errorText(e)); }
  }

  const swatches = ["#4388FF", "#8B5CF6", "#EC4899", "#EF4444", "#F59E0B", "#22C55E", "#06B6D4"];

  return (
    <div className="flex min-h-full gap-4">
      <nav className="flex w-[228px] shrink-0 flex-col gap-1" aria-label="Settings">
        {sections.map((name) => (
          <SidebarItem key={name} active={section === name.toLowerCase()} label={name} onClick={() => setSection(name.toLowerCase())} />
        ))}
      </nav>
      <div className="scroll flex min-w-0 flex-1 flex-col gap-4 pr-1">
      {section === "appearance" ? (
      <section className="card grid gap-4 p-4 lg:grid-cols-[1fr_220px]">
        <div>
        <h2 className="text-lg font-semibold">Appearance</h2>
        <p className="mt-3 text-sm text-[var(--color-muted)]">Theme</p>
        <div className="mt-2 grid grid-cols-3 gap-2">
          {(["dark", "light", "system"] as const).map((theme) => (
            <button key={theme} className={`tile p-3 ${draft.theme === theme ? "row-selected" : ""}`} aria-pressed={draft.theme === theme} onClick={() => void live({ theme })}>
              <span className="mb-2 block h-8 rounded-md border border-[var(--color-line)]" style={{ background: theme === "light" ? "#f3f6fb" : theme === "system" ? "linear-gradient(90deg,#090d17 50%,#f3f6fb 50%)" : "#151d2e" }} />
              <span className="text-sm">{theme === "dark" ? "Dark" : theme === "light" ? "Light" : "System"}</span>
            </button>
          ))}
        </div>
        <p className="mt-4 text-sm text-[var(--color-muted)]">Accent Color</p>
        <div className="mt-2 flex flex-wrap items-center gap-2">
          {swatches.map((color) => (
            <button key={color} aria-label={`Accent ${color}`} className="h-6 w-6 rounded-full border-2" style={{ background: color, borderColor: draft.accent.toLowerCase() === color.toLowerCase() ? "white" : "transparent" }} onClick={() => void live({ accent: color })} />
          ))}
          <input className="field hex h-8" aria-label="Custom accent" value={draft.accent} onChange={(e) => void live({ accent: e.target.value })} />
        </div>
        <Field label="Window Style">
          <select className="field" value={draft.density} onChange={(e) => void live({ density: e.target.value })}>
            <option value="comfortable">Modern (Rounded)</option>
            <option value="compact">Compact</option>
          </select>
        </Field>
        <Field label="Text scale"><input className="field" type="number" min={0.85} max={1.6} step={0.05} value={draft.textScale} onChange={(e) => void live({ textScale: Number(e.target.value) })} /></Field>
        <ToggleSwitch label="Show app icons" checked={draft.showAppIcons} onChange={(v) => void live({ showAppIcons: v })} />
        <ToggleSwitch label="Show shortcuts" checked={draft.showShortcuts} onChange={(v) => void live({ showShortcuts: v })} hint="Enter and Alt+1 through Alt+5 in the launcher." />
        <ToggleSwitch label="Enable transparency" checked={draft.translucency} onChange={(v) => void live({ translucency: v })} />
        <ToggleSwitch label="Reduce motion" checked={draft.reduceMotion} onChange={(v) => void live({ reduceMotion: v })} />
        <ToggleSwitch label="Higher contrast" checked={draft.highContrast} onChange={(v) => void live({ highContrast: v })} />
        </div>
        <aside className="tile self-start p-3" aria-label="Launcher preview">
          <PulseBrand />
          <div className="search-ring mt-3 h-8 rounded-lg" />
          <div className="row-selected mt-2 h-8 rounded-lg" />
        </aside>
      </section>
      ) : null}
      {section === "general" ? (
      <section className="card grid gap-3 p-4 md:grid-cols-2">
        <h2 className="md:col-span-2 text-lg font-semibold">General</h2>
        <Toggle label="Hide launcher when it loses focus" checked={draft.hideLauncherOnBlur} onChange={(v) => set("hideLauncherOnBlur", v)} />
        <Toggle label="Close window to tray" checked={draft.closeToTray} onChange={(v) => set("closeToTray", v)} />
        <Toggle label="Start minimized" checked={draft.startMinimized} onChange={(v) => set("startMinimized", v)} />
        <Toggle label="Start with Windows" checked={draft.launchAtStartup} onChange={(v) => set("launchAtStartup", v)} />
      </section>
      ) : null}
      {section === "hotkeys" ? (
      <section className="card grid gap-3 p-4 md:grid-cols-2">
        <h2 className="md:col-span-2 text-lg font-semibold">Hotkeys</h2>
        <Field label="Shortcut" hint="If this is taken, the fallback is tried and the result is shown when you save.">
          <input className="field" value={draft.launcherHotkey} onChange={(e) => set("launcherHotkey", e.target.value)} />
        </Field>
        <Field label="Fallback"><input className="field" value={draft.launcherHotkeyFallbacks[0] ?? ""} onChange={(e) => set("launcherHotkeyFallbacks", [e.target.value])} /></Field>
      </section>
      ) : null}
      {section === "search" ? (
      <section className="card grid gap-3 p-4">
        <h2 className="text-lg font-semibold">Search</h2>
        <p className="text-sm text-[var(--color-muted)]">File search stays off until you choose folders. Web search stays off until you turn it on. Neither scans the whole disk.</p>
        <Toggle label="Search files in folders I choose" checked={draft.searchFiles} onChange={(v) => set("searchFiles", v)} />
        <Toggle label="Offer web search (opens your browser)" checked={draft.webSearchEnabled} onChange={(v) => set("webSearchEnabled", v)} />
        <Toggle label="Remember launch history" checked={draft.recordHistory} onChange={(v) => set("recordHistory", v)} />
        <div className="flex gap-2">
          <button className="btn" onClick={() => void api.reindexFiles().then((r) => onNotice("Index", `${r.indexed} entries.${r.truncated ? " Stopped at the limit." : ""}`)).catch((e) => onNotice("Index", errorText(e)))}>Rebuild file index</button>
          <button className="btn" onClick={() => void api.clearHistory().then(() => onNotice("History", "Launch history, workflow logs and the file index were cleared.")).catch((e) => onNotice("History", errorText(e)))}>Clear history</button>
        </div>
      </section>
      ) : null}
      {section === "apps & paths" ? (
      <section className="card grid gap-3 p-4">
        <h2 className="text-lg font-semibold">Apps & Paths</h2>
        <p className="text-sm text-[var(--color-muted)]">Start Menu discovery runs on Windows. These switches choose extra sources. They do not invent programs.</p>
        <Field label="Indexed folders" hint="One absolute path per line. Used only when file search is on.">
          <textarea className="field min-h-20" value={draft.indexRoots.join("\n")} onChange={(e) => set("indexRoots", e.target.value.split("\n").map((s) => s.trim()).filter(Boolean))} />
        </Field>
        <Toggle label="Include desktop shortcuts" checked={draft.includeDesktopShortcuts} onChange={(v) => set("includeDesktopShortcuts", v)} />
        <Toggle label="Include packaged Windows apps" checked={draft.includePackagedApps} onChange={(v) => set("includePackagedApps", v)} />
      </section>
      ) : null}
      {section === "commands" ? (
      <section className="card grid gap-3 p-4">
        <h2 className="text-lg font-semibold">Commands</h2>
        <p className="text-sm text-[var(--color-muted)]">Off by default. A saved workflow can run an absolute program and capture its output only after this is on and you trust that workflow. There is no command that executes arbitrary text from search.</p>
        <Toggle label="Allow reviewed developer commands" checked={draft.developerCommandsEnabled} onChange={(v) => set("developerCommandsEnabled", v)} />
      </section>
      ) : null}
      {section === "ai integration" ? (
      <section className="card grid gap-3 p-4">
        <h2 className="text-lg font-semibold">AI Integration</h2>
        <p className="text-sm text-[var(--color-muted)]">Optional local Ollama only. Nothing is sent to a paid API, and a suggestion is never run.</p>
        <Toggle label="Use a local Ollama model" checked={draft.ollamaEnabled} onChange={(v) => set("ollamaEnabled", v)} />
        <Field label="Ollama address"><input className="field" value={draft.ollamaEndpoint} onChange={(e) => set("ollamaEndpoint", e.target.value)} /></Field>
        <Field label="Model"><input className="field" value={draft.ollamaModel} onChange={(e) => set("ollamaModel", e.target.value)} /></Field>
        <button className="btn w-fit" onClick={() => void api.ollamaStatus().then((s) => setOllama(s.message + (s.models.length ? ` ${s.models.join(", ")}` : ""))).catch((e) => setOllama(errorText(e)))}>Test connection</button>
        {ollama ? <p className="text-sm">{ollama}</p> : null}
        <Field label="Ask the model" hint="The answer is only shown. It is not run.">
          <input className="field" value={question} onChange={(e) => setQuestion(e.target.value)} />
        </Field>
        <button className="btn w-fit" disabled={!draft.ollamaEnabled} onClick={() => void api.ollamaSuggest(question).then((s) => setSuggest(`${s.notice}\n\n${s.text}`)).catch((e) => setSuggest(errorText(e)))}>Ask</button>
        {suggest ? <pre className="whitespace-pre-wrap text-sm">{suggest}</pre> : null}
        <Field label="PulseFlow address" hint="Leave empty unless your PulseFlow build documents a localhost endpoint. PulseLauncher will not invent one.">
          <input className="field" value={draft.pulseflowUrl} onChange={(e) => set("pulseflowUrl", e.target.value)} />
        </Field>
        <button className="btn w-fit" onClick={() => void api.pulseflowStatus().then((s) => setPulse(s.message)).catch((e) => setPulse(errorText(e)))}>Check PulseFlow address</button>
        {pulse ? <p className="text-sm">{pulse}</p> : null}
      </section>
      ) : null}
      {section === "extensions" ? (
      <section className="card p-4">
        <h2 className="text-lg font-semibold">Extensions</h2>
        <p className="mt-2 text-sm text-[var(--color-muted)]">PulseLauncher does not load plugins from the internet. Workflows and manual apps are the extension points.</p>
      </section>
      ) : null}
      {section === "backup & sync" ? (
      <section className="card p-4">
        <h2 className="text-lg font-semibold">Backup & Sync</h2>
        <p className="mt-1 text-sm text-[var(--color-muted)]">Backup stays on this computer. There is no account sync.</p>
        <div className="mt-2 flex gap-2">
          <button className="btn" onClick={() => void api.exportBundle().then((json) => { void navigator.clipboard?.writeText(json); onNotice("Export", "The backup JSON was copied. It contains preferences, custom apps and workflows."); })}>Copy export</button>
        </div>
        <Field label="Import JSON" hint="Preview first. Applying replaces preferences and adds what validates.">
          <textarea className="field min-h-24" value={importText} onChange={(e) => setImportText(e.target.value)} />
        </Field>
        <div className="mt-2 flex gap-2">
          <button className="btn" onClick={() => void previewImport(false)}>Preview</button>
          <button className="btn btn-primary" onClick={() => void previewImport(true)}>Apply</button>
        </div>
        {importNotes.map((n) => <p key={n} className="mt-1 text-sm">{n}</p>)}
      </section>
      ) : null}
      {section === "about" ? (
      <>
      {diagError ? <p className="text-sm text-[var(--color-warn)]">{diagError}</p> : null}
      <DiagnosticsPanel diag={diag} />
      <Gallery />
      </>
      ) : null}
      <div><button className="btn btn-primary" onClick={() => void save()}>Save settings</button></div>
      </div>
    </div>
  );

  async function previewImport(apply: boolean) {
    try {
      const report = await api.importBundle(importText, apply);
      setImportNotes([
        apply ? `Added ${report.appsAdded} apps, updated ${report.workflows} workflows and ${report.collections} collections.` : report.notes[0] ?? "Preview ready.",
        ...report.appsSkipped,
        ...report.notes,
      ]);
      if (apply) {
        onPrefs(await api.getPreferences());
        onNotice("Import", "The backup was applied.");
      }
    } catch (e) { setImportNotes([errorText(e)]); }
  }
}

function Toggle({ label, checked, onChange }: { label: string; checked: boolean; onChange: (v: boolean) => void }) {
  return (
    <label className="flex items-center justify-between gap-3 rounded-xl border border-[var(--color-line)] px-3 py-2 text-sm">
      <span>{label}</span>
      <input type="checkbox" checked={checked} onChange={(e) => onChange(e.target.checked)} />
    </label>
  );
}

function DiagnosticsPanel({ diag }: { diag: Diagnostics | null }) {
  if (!diag) return null;
  return (
    <section className="card p-4 text-sm">
      <h2 className="text-lg font-semibold">Diagnostics</h2>
      <p className="mt-2">PulseLauncher {diag.info.version} · {diag.info.platform}{diag.info.windowsNative ? "" : " · Windows launch APIs are not active on this host"}</p>
      <p>Data: {diag.info.dataDir}</p>
      <p>Database: {diag.info.dbPath} · schema {diag.schemaVersion}</p>
      <p>{diag.appCount} apps · {diag.portableCount} portable or custom · {diag.favoriteCount} favorites · {diag.fileIndexCount} indexed files</p>
      <p>Shortcut requested {diag.hotkey.requested || "none"}, active {diag.hotkey.active ?? "none"}. {diag.hotkey.warning ?? ""}</p>
      <p>Last scan: {diag.lastScan ? `${formatWhen(diag.lastScan.finishedAt)} · found ${diag.lastScan.found}` : "not run yet"}</p>
      {diag.lastScan?.errors.map((e) => <p key={e} className="text-[var(--color-warn)]">{e}</p>)}
      <h3 className="mt-3 font-medium">Recent activity</h3>
      {diag.recentEvents.length === 0 ? <p className="text-[var(--color-muted)]">No activity recorded.</p> : diag.recentEvents.slice(0, 8).map((e) => (
        <p key={e.id}>{formatWhen(e.at)} · {e.success ? "ok" : "failed"} · {e.label}: {e.message}</p>
      ))}
    </section>
  );
}

export function Gallery() {
  return (
    <section className="card p-4" aria-label="Component gallery">
      <h2 className="text-lg font-semibold">Interface gallery</h2>
      <p className="mb-3 text-sm text-[var(--color-muted)]">The controls PulseLauncher uses, in the active theme.</p>
      <div className="flex flex-wrap items-center gap-2">
        <button className="btn btn-primary">Primary</button>
        <button className="btn">Secondary</button>
        <button className="btn" disabled>Disabled</button>
        <span className="pill pill-ok">Ready</span>
        <span className="pill pill-warn">Needs setup</span>
        <span className="pill pill-danger">Missing</span>
        <kbd className="kbd">Alt</kbd><kbd className="kbd">Space</kbd>
        <input className="field max-w-40" placeholder="Field" aria-label="Sample field" />
      </div>
    </section>
  );
}
