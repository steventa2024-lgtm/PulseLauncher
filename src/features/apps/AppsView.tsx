import { useEffect, useMemo, useRef, useState } from "react";
import { Folder, Grid2x2, Pin, Plus, RefreshCw, Settings, Star, Terminal, Workflow, Wrench } from "lucide-react";
import { api, errorText } from "../../lib/api";
import { targetText } from "../../lib/text";
import type { AppEntry, Collection } from "../../lib/types";
import { AppMark, Empty, Field, Modal } from "../../components/ui";
import { SearchBox, SidebarItem } from "../../components/chrome";
import { reorderVisible } from "./reorder";

const CATS = [
  { id: "All", label: "All Apps", icon: Grid2x2 },
  { id: "ZeroPulse", label: "ZeroPulse", icon: Star },
  { id: "Favorites", label: "Pinned", icon: Pin },
  { id: "Developer Tools", label: "Development", icon: Terminal },
  { id: "Productivity", label: "Productivity", icon: Wrench },
  { id: "System", label: "System", icon: Folder },
  { id: "Media", label: "Media", icon: Folder },
] as const;

export function AppsView({
  apps,
  collections,
  icons,
  onOpen,
  onChanged,
  onNotice,
  onOpenWorkflows,
  onOpenSettings,
}: {
  apps: AppEntry[];
  collections: Collection[];
  icons: Record<string, string | null>;
  onOpen: (id: string) => void;
  onChanged: () => void;
  onNotice: (title: string, body: string) => void;
  onOpenWorkflows?: () => void;
  onOpenSettings?: () => void;
}) {
  const [filter, setFilter] = useState<string>("All");
  const [query, setQuery] = useState("");
  const [grid, setGrid] = useState(true);
  const [adding, setAdding] = useState(false);
  const [menu, setMenu] = useState<{ x: number; y: number; app: AppEntry } | null>(null);
  const [scanning, setScanning] = useState(false);
  const [draggingId, setDraggingId] = useState<string | null>(null);
  const [overId, setOverId] = useState<string | null>(null);
  const [reorderStatus, setReorderStatus] = useState("");

  const visible = useMemo(() => {
    const q = query.trim().toLowerCase();
    return apps.filter((app) => {
      if (q && !`${app.name} ${app.tags.join(" ")} ${targetText(app)}`.toLowerCase().includes(q)) return false;
      if (filter === "Favorites") return app.favorite;
      if (filter === "Recent") return !!app.lastLaunchedAt;
      if (filter === "Portable") return app.portable || app.source === "manual";
      if (filter === "All") return true;
      if (filter === "Developer Tools") return app.category === "Developer Tools" || app.category === "Development";
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

  async function persistOrder(next: string[]) {
    const current = apps.map((item) => item.id).join("\0");
    if (next.join("\0") === current) return;
    try {
      await api.reorderApps(next);
      onChanged();
    } catch (error) {
      onNotice("Reorder", errorText(error));
    }
  }

  async function move(app: AppEntry, delta: number) {
    const visibleIds = visible.map((item) => item.id);
    const index = visibleIds.indexOf(app.id);
    const neighbor = visibleIds[index + delta];
    if (index < 0 || neighbor === undefined) return;
    await persistOrder(reorderVisible(apps.map((item) => item.id), visibleIds, app.id, neighbor));
    setReorderStatus(`${app.name} moved ${delta < 0 ? "up" : "down"} in the visible list.`);
  }

  const dragState = useRef({ draggingId, apps, visible });
  dragState.current = { draggingId, apps, visible };
  const persistRef = useRef(persistOrder);
  persistRef.current = persistOrder;

  useEffect(() => {
    function onUp(event: PointerEvent) {
      const from = dragState.current.draggingId;
      if (!from) return;
      dragState.current.draggingId = null;
      const under = document.elementFromPoint(event.clientX, event.clientY);
      const target = under?.closest("[data-app-id]")?.getAttribute("data-app-id") ?? "";
      const name = dragState.current.apps.find((item) => item.id === from)?.name ?? "App";
      setDraggingId(null);
      setOverId(null);
      if (!target || target === from) return;
      const next = reorderVisible(
        dragState.current.apps.map((item) => item.id),
        dragState.current.visible.map((item) => item.id),
        from,
        target,
      );
      setReorderStatus(`${name} moved in the visible list.`);
      void persistRef.current(next);
    }
    window.addEventListener("pointerup", onUp);
    return () => window.removeEventListener("pointerup", onUp);
  }, []);

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

  const extraCollections = collections.filter((c) => !CATS.some((cat) => cat.label === c.name || cat.id === c.name));

  function countFor(id: string) {
    if (id === "All") return apps.length;
    if (id === "Favorites") return apps.filter((app) => app.favorite).length;
    if (id === "Developer Tools") return apps.filter((app) => app.category === "Developer Tools" || app.category === "Development").length;
    if (id === "ZeroPulse") {
      const collection = collections.find((c) => c.name === "ZeroPulse");
      if (collection) return collection.appIds.length;
    }
    return apps.filter((app) => app.category === id).length;
  }

  return (
    <>
    <div className="flex min-h-full gap-4">
      <nav className="flex w-[228px] shrink-0 flex-col gap-1" aria-label="App categories">
        {CATS.map((cat) => {
          const Icon = cat.icon;
          return (
            <SidebarItem key={cat.id} active={filter === cat.id} icon={<Icon size={15} />} label={cat.label} count={countFor(cat.id)} onClick={() => setFilter(cat.id)} />
          );
        })}
        {extraCollections.map((c) => (
          <SidebarItem key={c.id} active={filter === c.name} label={c.name} count={c.appIds.length} onClick={() => setFilter(c.name)} />
        ))}
        <div className="mt-3 border-t border-[var(--color-line)] pt-2">
          <SidebarItem active={false} icon={<Workflow size={15} />} label="Commands & Workflows" onClick={() => onOpenWorkflows?.()} />
          <SidebarItem active={false} icon={<Settings size={15} />} label="Settings" onClick={() => onOpenSettings?.()} />
        </div>
      </nav>
      <p className="sr-only" aria-live="polite">{reorderStatus}</p>
      <div className="flex min-w-0 flex-1 flex-col gap-3">
      <div className="flex flex-wrap items-center gap-2">
        <h2 className="mr-2 text-lg font-semibold">{CATS.find((c) => c.id === filter)?.label ?? filter}</h2>
        <div className="ml-auto w-full max-w-xs">
          <SearchBox value={query} onChange={setQuery} placeholder="Search applications..." label="Filter apps" />
        </div>
        <button className="btn" onClick={() => void scan()} disabled={scanning}><RefreshCw size={16} /> {scanning ? "Scanning" : "Rescan"}</button>
        <button className="btn btn-primary" onClick={() => setAdding(true)}><Plus size={16} /> Add app</button>
        <button className="btn btn-ghost" aria-label={grid ? "Show as list" : "Show as grid"} onClick={() => setGrid((v) => !v)}>
          {grid ? "List" : "Grid"}
        </button>
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
        <div className={grid ? "grid grid-cols-2 gap-3 md:grid-cols-3 xl:grid-cols-4" : "flex flex-col gap-2"}>
          {visible.map((app) => (
            <article
              key={app.id}
              data-app-id={app.id}
              className={`tile relative flex flex-col items-center gap-2 p-3 text-center ${overId === app.id ? "border-[var(--color-accent)]" : ""} ${draggingId === app.id ? "opacity-60" : ""}`}
              onContextMenu={(e) => { e.preventDefault(); setMenu({ x: e.clientX, y: e.clientY, app }); }}
              onPointerEnter={() => { if (draggingId && draggingId !== app.id) setOverId(app.id); }}
            >
              <button
                className="absolute top-2 right-2 text-[var(--color-muted)]"
                aria-label={app.favorite ? `Unpin ${app.name}` : `Pin ${app.name}`}
                onClick={() => void api.updateApp(app.id, { favorite: !app.favorite }).then(onChanged)}
              >
                <Star size={14} className={app.favorite ? "fill-[var(--color-accent)] text-[var(--color-accent)]" : ""} />
              </button>
              <span
                role="button"
                tabIndex={0}
                aria-label={`Drag ${app.name}`}
                aria-keyshortcuts="ArrowUp ArrowDown"
                title="Drag to reorder. Arrow keys move this app in the current list."
                className="absolute top-1 left-1 grid h-8 w-8 cursor-grab touch-none place-items-center text-xs text-[var(--color-faint)]"
                onPointerCancel={() => { setDraggingId(null); setOverId(null); }}
                onPointerDown={(event) => {
                  event.currentTarget.setPointerCapture?.(event.pointerId);
                  dragState.current.draggingId = app.id;
                  setDraggingId(app.id);
                  setMenu(null);
                }}
                onKeyDown={(event) => {
                  if (event.key === "ArrowUp") { event.preventDefault(); void move(app, -1); }
                  if (event.key === "ArrowDown") { event.preventDefault(); void move(app, 1); }
                }}
              >
                ::
              </span>
              <AppMark name={app.name} icon={icons[app.id]} size={42} />
              <h3 className="w-full truncate text-sm font-medium">{app.name}</h3>
              <p className="w-full truncate text-xs text-[var(--color-muted)]">{app.category ?? "Uncategorized"}{app.status === "missing" ? " · Missing" : ""}</p>
              <div className="flex gap-2">
                <button className="btn btn-primary" onClick={() => void launch(app)}>Run</button>
                <button className="btn" onClick={() => onOpen(app.id)}>Details</button>
              </div>
            </article>
          ))}
        </div>
      )}
      </div>
    </div>
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
    </>
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
