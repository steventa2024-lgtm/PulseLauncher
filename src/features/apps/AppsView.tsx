import { useMemo, useState } from "react";
import { LayoutGrid, List, Plus, RefreshCw, Star } from "lucide-react";
import { api, errorText } from "../../lib/api";
import { sourceLabel, targetText } from "../../lib/text";
import type { AppEntry, Collection } from "../../lib/types";
import { AppMark, Empty, Field, Modal } from "../../components/ui";

const FILTERS = ["All", "ZeroPulse", "Developer Tools", "Productivity", "Favorites", "Recent", "Portable"] as const;

export function AppsView({
  apps,
  collections,
  icons,
  onOpen,
  onChanged,
  onNotice,
}: {
  apps: AppEntry[];
  collections: Collection[];
  icons: Record<string, string | null>;
  onOpen: (id: string) => void;
  onChanged: () => void;
  onNotice: (title: string, body: string) => void;
}) {
  const [filter, setFilter] = useState<string>("All");
  const [query, setQuery] = useState("");
  const [grid, setGrid] = useState(true);
  const [adding, setAdding] = useState(false);
  const [menu, setMenu] = useState<{ x: number; y: number; app: AppEntry } | null>(null);
  const [scanning, setScanning] = useState(false);

  const visible = useMemo(() => {
    const q = query.trim().toLowerCase();
    return apps.filter((app) => {
      if (q && !`${app.name} ${app.tags.join(" ")} ${targetText(app)}`.toLowerCase().includes(q)) return false;
      if (filter === "Favorites") return app.favorite;
      if (filter === "Recent") return !!app.lastLaunchedAt;
      if (filter === "Portable") return app.portable || app.source === "manual";
      if (filter === "All") return true;
      if (collections.some((c) => c.name === filter)) {
        const collection = collections.find((c) => c.name === filter);
        return !!collection && collection.appIds.includes(app.id);
      }
      return app.category === filter;
    });
  }, [apps, collections, filter, query]);

  async function launch(app: AppEntry) {
    try {
      const outcome = await api.launchApp(app.id);
      onNotice(app.name, outcome.message);
      onChanged();
    } catch (error) {
      onNotice(app.name, errorText(error));
    }
  }

  async function move(app: AppEntry, delta: number) {
    const ids = apps.map((item) => item.id);
    const index = ids.indexOf(app.id);
    const next = index + delta;
    if (index < 0 || next < 0 || next >= ids.length) return;
    const swapped = [...ids];
    const moved = swapped[index];
    const target = swapped[next];
    if (moved === undefined || target === undefined) return;
    swapped[index] = target;
    swapped[next] = moved;
    try {
      await api.reorderApps(swapped);
      onChanged();
    } catch (error) {
      onNotice("Reorder", errorText(error));
    }
  }

  async function scan() {
    setScanning(true);
    try {
      const report = await api.scanApps();
      const extra = report.errors.length ? ` ${report.errors[0]}` : "";
      onNotice("Scan finished", `Found ${report.found}. Added ${report.added}, updated ${report.updated}.${extra}`);
      onChanged();
    } catch (error) {
      onNotice("Scan failed", errorText(error));
    } finally {
      setScanning(false);
    }
  }

  const chips = [...FILTERS, ...collections.map((c) => c.name)];

  return (
    <div className="flex h-full min-h-0 flex-col gap-4">
      <div className="flex flex-wrap items-center gap-2">
        <input className="field max-w-sm" placeholder="Filter apps" aria-label="Filter apps" value={query} onChange={(e) => setQuery(e.target.value)} />
        <button className="btn" onClick={() => void scan()} disabled={scanning}><RefreshCw size={16} /> {scanning ? "Scanning" : "Rescan"}</button>
        <button className="btn btn-primary" onClick={() => setAdding(true)}><Plus size={16} /> Add app</button>
        <button className="btn btn-ghost" aria-label={grid ? "Show as list" : "Show as grid"} onClick={() => setGrid((v) => !v)}>
          {grid ? <List size={16} /> : <LayoutGrid size={16} />}
        </button>
        <span className="ml-auto text-sm text-[var(--color-muted)]">{visible.length} shown</span>
      </div>
      <div className="flex flex-wrap gap-2" role="tablist" aria-label="App categories">
        {chips.map((chip) => (
          <button key={chip} role="tab" aria-selected={filter === chip} className={`pill ${filter === chip ? "border-[var(--color-accent)] text-[var(--color-ink)]" : ""}`} onClick={() => setFilter(chip)}>
            {chip}
          </button>
        ))}
      </div>
      {apps.length === 0 ? (
        <Empty
          title="No apps yet"
          body="PulseLauncher looks in the Start Menu on Windows. If this computer has none, or you are not on Windows, add a program yourself. Nothing is invented to fill this list."
          action={<button className="btn btn-primary" onClick={() => setAdding(true)}>Add app</button>}
        />
      ) : visible.length === 0 ? (
        <Empty title="Nothing in this view" body="Try another category, or clear the filter." />
      ) : (
        <div className={grid ? "grid grid-cols-[repeat(auto-fill,minmax(240px,1fr))] gap-3" : "flex flex-col gap-2"}>
          {visible.map((app) => (
            <article
              key={app.id}
              className="card flex gap-3 p-3"
              onContextMenu={(e) => { e.preventDefault(); setMenu({ x: e.clientX, y: e.clientY, app }); }}
            >
              <AppMark name={app.name} icon={icons[app.id]} />
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2">
                  <h3 className="truncate font-medium">{app.name}</h3>
                  {app.favorite ? <Star size={14} className="text-[var(--color-warn)]" aria-label="Favorite" /> : null}
                  {app.status === "missing" ? <span className="pill pill-danger">Missing</span> : null}
                </div>
                <p className="truncate text-sm text-[var(--color-muted)]">{sourceLabel(app.source)} · {app.category ?? "Uncategorized"}</p>
                <p className="truncate text-xs text-[var(--color-faint)]" title={targetText(app)}>{targetText(app)}</p>
                <div className="mt-2 flex gap-2">
                  <button className="btn btn-primary" onClick={() => void launch(app)}>Run</button>
                  <button className="btn" onClick={() => onOpen(app.id)}>Details</button>
                </div>
              </div>
            </article>
          ))}
        </div>
      )}
      {menu ? (
        <div className="fixed z-40 min-w-44 rounded-xl border border-[var(--color-line)] bg-[var(--color-panel)] p-1 shadow-[var(--shadow-overlay)]" style={{ left: menu.x, top: menu.y }} role="menu">
          <button className="btn btn-ghost w-full justify-start" onClick={() => { void launch(menu.app); setMenu(null); }}>Run</button>
          <button className="btn btn-ghost w-full justify-start" onClick={() => { onOpen(menu.app.id); setMenu(null); }}>Details</button>
          <button className="btn btn-ghost w-full justify-start" onClick={() => { void api.updateApp(menu.app.id, { favorite: !menu.app.favorite }).then(onChanged); setMenu(null); }}>
            {menu.app.favorite ? "Unfavorite" : "Favorite"}
          </button>
          <button className="btn btn-ghost w-full justify-start" onClick={() => { void navigator.clipboard?.writeText(targetText(menu.app)); setMenu(null); }}>Copy path</button>
          <button className="btn btn-ghost w-full justify-start" onClick={() => { void move(menu.app, -1); setMenu(null); }}>Move up</button>
          <button className="btn btn-ghost w-full justify-start" onClick={() => { void move(menu.app, 1); setMenu(null); }}>Move down</button>
        </div>
      ) : null}
      {menu ? <button className="fixed inset-0 z-30 cursor-default" aria-label="Close menu" onClick={() => setMenu(null)} /> : null}
      {adding ? <AddApp collections={collections} onClose={() => setAdding(false)} onSaved={() => { setAdding(false); onChanged(); }} onNotice={onNotice} /> : null}
    </div>
  );
}

function AddApp({ collections, onClose, onSaved, onNotice }: { collections: Collection[]; onClose: () => void; onSaved: () => void; onNotice: (t: string, b: string) => void }) {
  const [name, setName] = useState("");
  const [path, setPath] = useState("");
  const [args, setArgs] = useState("");
  const [cwd, setCwd] = useState("");
  const [category, setCategory] = useState("");
  const [collectionId, setCollectionId] = useState("");
  const [busy, setBusy] = useState(false);

  async function browse() {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const picked = await open({ multiple: false, title: "Choose a program or shortcut" });
    if (typeof picked === "string") {
      setPath(picked);
      if (!name) setName(picked.split(/[/\\]/).pop()?.replace(/\.(exe|lnk)$/i, "") ?? "");
    }
  }

  async function save() {
    setBusy(true);
    try {
      const lower = path.toLowerCase();
      const kind = lower.endsWith(".lnk") ? "shortcut" : lower.includes(":") && !lower.includes("\\") && !lower.includes("/") ? "uri" : "executable";
      await api.addApp({
        name,
        kind,
        launchTarget: path,
        arguments: args.trim() ? splitArgs(args) : [],
        workingDir: cwd || null,
        category: category || null,
        portable: true,
        tags: [],
      }, collectionId || undefined);
      onNotice("App added", `${name} is registered. The original file was not changed.`);
      onSaved();
    } catch (error) {
      onNotice("Could not add app", errorText(error));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Modal title="Add app" onClose={onClose}>
      <div className="flex flex-col gap-3">
        <Field label="Display name"><input className="field" value={name} onChange={(e) => setName(e.target.value)} /></Field>
        <Field label="Program, shortcut, or link" hint="An absolute .exe or .lnk path, or an approved link such as ms-settings:">
          <div className="flex gap-2">
            <input className="field" value={path} onChange={(e) => setPath(e.target.value)} />
            <button className="btn" type="button" onClick={() => void browse()}>Browse</button>
          </div>
        </Field>
        <Field label="Arguments" hint="Separated by spaces. Quotes keep an argument together. They are stored as a list, not a shell command.">
          <input className="field" value={args} onChange={(e) => setArgs(e.target.value)} />
        </Field>
        <Field label="Working directory"><input className="field" value={cwd} onChange={(e) => setCwd(e.target.value)} /></Field>
        <Field label="Category"><input className="field" value={category} onChange={(e) => setCategory(e.target.value)} placeholder="ZeroPulse" /></Field>
        <Field label="Collection">
          <select className="field" value={collectionId} onChange={(e) => setCollectionId(e.target.value)}>
            <option value="">None</option>
            {collections.map((c) => <option key={c.id} value={c.id}>{c.name}</option>)}
          </select>
        </Field>
        <div className="flex justify-end gap-2">
          <button className="btn" onClick={onClose}>Cancel</button>
          <button className="btn btn-primary" disabled={busy || !name || !path} onClick={() => void save()}>{busy ? "Checking" : "Save"}</button>
        </div>
      </div>
    </Modal>
  );
}

function splitArgs(input: string): string[] {
  const out: string[] = [];
  let cur = "";
  let quote = false;
  for (const ch of input) {
    if (ch === '"') quote = !quote;
    else if (ch === " " && !quote) {
      if (cur) out.push(cur);
      cur = "";
    } else cur += ch;
  }
  if (cur) out.push(cur);
  return out;
}
