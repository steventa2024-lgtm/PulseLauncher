import { useEffect, useMemo, useState, type ReactNode } from "react";
import { z } from "zod";
import { Banner, Button, Dialog, EmptyState, Field, controlClass } from "../components/ui";
import { api, DesktopError, pickPath } from "../lib/api";
import type { AppEntry, Collection, ScanReport } from "../lib/types";
import { ZEROPULSE_COLLECTION_ID } from "../lib/types";
import { argumentLines, matchesZeropulse, sourceLabel } from "../lib/text";
import { useShell } from "../store";

const formSchema = z.object({
  displayName: z.string().trim().min(1, "Enter a display name.").max(120),
  target: z.string().trim().min(1, "Choose an executable, shortcut, or URL.").max(4096),
  argumentsText: z.string().max(20000),
  workingDirectory: z.string().max(4096),
  tags: z.string().max(500),
  notes: z.string().max(2000),
  collectionId: z.string(),
});

type Filter =
  | "all"
  | "zeropulse"
  | "developer"
  | "productivity"
  | "favorites"
  | "recent"
  | "portable"
  | `collection:${string}`;

export function AppsView() {
  const openApp = useShell((state) => state.openApp);
  const notify = useShell((state) => state.notify);
  const preferences = useShell((state) => state.preferences);
  const [apps, setApps] = useState<AppEntry[]>([]);
  const [collections, setCollections] = useState<Collection[]>([]);
  const [filter, setFilter] = useState<Filter>("all");
  const [query, setQuery] = useState("");
  const [sort, setSort] = useState<"name" | "recent" | "launches" | "manual">("name");
  const [error, setError] = useState<string | null>(null);
  const [scan, setScan] = useState<ScanReport | null>(null);
  const [wizard, setWizard] = useState<null | { collectionId?: string }>(null);
  const [menu, setMenu] = useState<{ x: number; y: number; id: string } | null>(null);
  const [loading, setLoading] = useState(true);

  async function reload() {
    setLoading(true);
    try {
      const [nextApps, nextCollections] = await Promise.all([api.appsList(false), api.collectionsList()]);
      setApps(nextApps);
      setCollections(nextCollections);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not load apps.");
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void reload();
  }, []);

  const zeroMembers = useMemo(() => {
    const collection = collections.find((item) => item.id === ZEROPULSE_COLLECTION_ID);
    return new Set(collection?.appIds ?? []);
  }, [collections]);

  const counts = {
    installed: apps.filter((app) => app.discovered && !app.missing).length,
    portable: apps.filter((app) => app.source === "manual").length,
    favorites: apps.filter((app) => app.favorite).length,
    recent: apps.filter((app) => app.lastLaunchedAt).length,
  };

  const visible = useMemo(() => {
    const collection = filter.startsWith("collection:")
      ? collections.find((item) => item.id === filter.slice("collection:".length))
      : undefined;
    let next = apps.filter((app) => {
      if (filter === "zeropulse") return matchesZeropulse(app, zeroMembers);
      if (filter === "developer") return app.category === "Developer Tools";
      if (filter === "productivity") return app.category === "Productivity";
      if (filter === "favorites") return app.favorite;
      if (filter === "recent") return Boolean(app.lastLaunchedAt);
      if (filter === "portable") return app.source === "manual";
      if (collection) return collection.appIds.includes(app.id);
      return true;
    });
    const needle = query.trim().toLowerCase();
    if (needle) {
      next = next.filter((app) =>
        [app.displayName, app.launchTarget, app.category, app.publisher ?? "", app.tags.join(" ")]
          .join(" ")
          .toLowerCase()
          .includes(needle),
      );
    }
    if (collection && sort === "manual") {
      const order = new Map(collection.appIds.map((id, index) => [id, index]));
      next = [...next].sort((a, b) => (order.get(a.id) ?? 0) - (order.get(b.id) ?? 0));
    } else {
      next = [...next].sort((a, b) => {
        if (sort === "launches") return b.launchCount - a.launchCount || a.displayName.localeCompare(b.displayName);
        if (sort === "recent") return (b.lastLaunchedAt ?? "").localeCompare(a.lastLaunchedAt ?? "");
        return a.displayName.localeCompare(b.displayName);
      });
    }
    return next;
  }, [apps, collections, filter, query, sort, zeroMembers]);

  async function launch(id: string) {
    try {
      const outcome = await api.appsLaunch(id);
      notify(outcome.message);
      await reload();
    } catch (err) {
      notify(err instanceof Error ? err.message : "Launch failed.");
    }
  }

  async function onScan() {
    try {
      const report = await api.appsScan();
      setScan(report);
      await reload();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Scan failed.");
    }
  }

  const viewMode = preferences?.viewMode ?? "grid";
  const activeCollection = filter.startsWith("collection:") ? filter.slice("collection:".length) : null;

  return (
    <section className="space-y-4">
      <header className="flex flex-wrap items-end justify-between gap-3">
        <div>
          <h1 className="text-2xl font-semibold">Apps & Tools</h1>
          <p className="text-sm text-muted">Installed entries and portable programs you register. Nothing here is invented.</p>
        </div>
        <div className="flex flex-wrap gap-2">
          <Button onClick={() => void onScan()}>Rescan</Button>
          <Button onClick={() => setWizard({})}>Add app</Button>
          <Button variant="primary" onClick={() => setWizard({ collectionId: ZEROPULSE_COLLECTION_ID })}>Add ZeroPulse app</Button>
        </div>
      </header>
      <div className="grid gap-2 sm:grid-cols-4">
        <Stat label="Installed" value={counts.installed} />
        <Stat label="Portable" value={counts.portable} />
        <Stat label="Favorites" value={counts.favorites} />
        <Stat label="Recent" value={counts.recent} />
      </div>
      {error ? <Banner tone="danger">{error}</Banner> : null}
      {scan ? (
        <Banner tone={scan.errors.length ? "warn" : "ok"}>
          Scan {scan.scannedAt}: added {scan.added}, updated {scan.updated}, marked missing {scan.markedMissing}.
          {scan.errors.length ? ` ${scan.errors.slice(0, 3).join(" ")}` : ""}
        </Banner>
      ) : null}
      <div className="flex flex-wrap gap-2">
        <FilterButton current={filter} id="all" onSelect={setFilter}>All apps</FilterButton>
        <FilterButton current={filter} id="zeropulse" onSelect={setFilter}>ZeroPulse</FilterButton>
        <FilterButton current={filter} id="developer" onSelect={setFilter}>Developer tools</FilterButton>
        <FilterButton current={filter} id="productivity" onSelect={setFilter}>Productivity</FilterButton>
        <FilterButton current={filter} id="favorites" onSelect={setFilter}>Favorites</FilterButton>
        <FilterButton current={filter} id="recent" onSelect={setFilter}>Recent</FilterButton>
        <FilterButton current={filter} id="portable" onSelect={setFilter}>Portable apps</FilterButton>
        {collections.map((collection) => (
          <FilterButton key={collection.id} current={filter} id={`collection:${collection.id}`} onSelect={setFilter}>
            {collection.name}
          </FilterButton>
        ))}
      </div>
      <div className="flex flex-wrap gap-2">
        <input aria-label="Filter apps" className={`${controlClass} max-w-sm`} placeholder="Filter by name, path, or tag" value={query} onChange={(event) => setQuery(event.target.value)} />
        <select aria-label="Sort apps" className={`${controlClass} max-w-[12rem]`} value={sort} onChange={(event) => setSort(event.target.value as typeof sort)}>
          <option value="name">Name</option>
          <option value="recent">Recent</option>
          <option value="launches">Launch count</option>
          {activeCollection ? <option value="manual">Collection order</option> : null}
        </select>
        <Button
          onClick={() => {
            if (!preferences) return;
            const viewMode = preferences.viewMode === "grid" ? "list" : "grid";
            void api.preferencesSet({ ...preferences, viewMode }).then((saved) => useShell.getState().setPreferences(saved));
          }}
        >
          {viewMode === "grid" ? "List" : "Grid"}
        </Button>
      </div>
      {loading ? <p className="text-sm text-muted">Loading applications…</p> : null}
      {!loading && visible.length === 0 ? (
        <EmptyState
          title={apps.length === 0 ? "No applications yet" : "Nothing matches this view"}
          body={
            apps.length === 0
              ? "Scan looks at Start Menu shortcuts on Windows and desktop entries on Linux. If discovery finds nothing, register an executable that exists on this machine."
              : "Try another category or clear the filter. Hidden entries stay out of this list."
          }
          action={<Button variant="primary" onClick={() => setWizard({})}>Add app</Button>}
        />
      ) : (
        <div className={viewMode === "grid" ? "grid gap-3 sm:grid-cols-2 xl:grid-cols-3" : "space-y-2"}>
          {visible.map((app) => (
            <article
              key={app.id}
              tabIndex={0}
              draggable={Boolean(activeCollection && sort === "manual")}
              onDragStart={(event) => event.dataTransfer.setData("text/plain", app.id)}
              onDragOver={(event) => {
                if (activeCollection && sort === "manual") event.preventDefault();
              }}
              onDrop={(event) => {
                if (!activeCollection) return;
                event.preventDefault();
                const source = event.dataTransfer.getData("text/plain");
                const collection = collections.find((item) => item.id === activeCollection);
                if (!collection || !source || source === app.id) return;
                const ids = collection.appIds.filter((id) => id !== source);
                const index = ids.indexOf(app.id);
                ids.splice(index < 0 ? ids.length : index, 0, source);
                void api.collectionsSetMembers(collection.id, ids).then(reload).catch((err: Error) => notify(err.message));
              }}
              onKeyDown={(event) => {
                if (event.key === "Enter" && !event.ctrlKey) {
                  event.preventDefault();
                  void launch(app.id);
                }
                if (event.key === "Enter" && event.ctrlKey) openApp(app.id);
              }}
              onContextMenu={(event) => {
                event.preventDefault();
                setMenu({ x: event.clientX, y: event.clientY, id: app.id });
              }}
              className="rounded-card border border-line bg-panel p-3 outline-none"
            >
              <button className="w-full text-left" onClick={() => openApp(app.id)}>
                <div className="flex items-start justify-between gap-2">
                  <div>
                    <h2 className="font-semibold">{app.displayName}</h2>
                    <p className="text-xs text-muted">{sourceLabel(app.source)} · {app.category}</p>
                  </div>
                  {app.missing ? <span className="text-xs font-semibold text-danger">Missing</span> : null}
                  {app.pinned ? <span className="text-xs text-bright">Pinned</span> : null}
                </div>
                <p className="mt-2 truncate text-xs text-muted" title={app.launchTarget}>{app.launchTarget}</p>
              </button>
              <div className="mt-3 flex gap-2">
                <Button variant="primary" onClick={() => void launch(app.id)} disabled={app.missing}>Run</Button>
                <Button onClick={() => openApp(app.id)}>Details</Button>
              </div>
            </article>
          ))}
        </div>
      )}
      {menu ? (
        <AppMenu
          x={menu.x}
          y={menu.y}
          app={apps.find((app) => app.id === menu.id)}
          collections={collections}
          onClose={() => setMenu(null)}
          onChanged={reload}
        />
      ) : null}
      {wizard ? (
        <RegisterDialog
          collectionId={wizard.collectionId ?? ""}
          collections={collections}
          onClose={() => setWizard(null)}
          onSaved={async (app) => {
            setWizard(null);
            await reload();
            openApp(app.id);
          }}
        />
      ) : null}
    </section>
  );
}

function Stat({ label, value }: { label: string; value: number }) {
  return (
    <div className="rounded-card border border-line bg-panel px-3 py-2">
      <div className="text-xs text-muted">{label}</div>
      <div className="text-xl font-semibold">{value}</div>
    </div>
  );
}

function FilterButton({ current, id, onSelect, children }: { current: string; id: Filter; onSelect: (id: Filter) => void; children: string }) {
  const active = current === id;
  return (
    <button
      className={`rounded-full border px-3 py-1 text-xs ${active ? "border-bright bg-panel text-foreground" : "border-line text-muted"}`}
      aria-pressed={active}
      onClick={() => onSelect(id)}
    >
      {children}
    </button>
  );
}

function AppMenu({
  x,
  y,
  app,
  collections,
  onClose,
  onChanged,
}: {
  x: number;
  y: number;
  app: AppEntry | undefined;
  collections: Collection[];
  onClose: () => void;
  onChanged: () => Promise<void>;
}) {
  const notify = useShell((state) => state.notify);
  const openApp = useShell((state) => state.openApp);
  if (!app) return null;
  async function run(action: () => Promise<unknown>) {
    try {
      await action();
      await onChanged();
    } catch (error) {
      notify(error instanceof Error ? error.message : "Action failed.");
    } finally {
      onClose();
    }
  }
  return (
    <div className="fixed z-30 min-w-52 rounded-card border border-line bg-elevated p-1 shadow-lift" style={{ left: x, top: y }} role="menu">
      <MenuItem onClick={() => void run(() => api.appsLaunch(app.id))}>Run</MenuItem>
      <MenuItem onClick={() => { openApp(app.id); onClose(); }}>Details</MenuItem>
      <MenuItem onClick={() => void run(() => api.appsFavorite(app.id, !app.favorite))}>{app.favorite ? "Unfavorite" : "Favorite"}</MenuItem>
      <MenuItem onClick={() => void run(() => api.appsPin(app.id, !app.pinned))}>{app.pinned ? "Unpin" : "Pin"}</MenuItem>
      <MenuItem onClick={() => void run(() => api.appsCopyPath(app.id))}>Copy path</MenuItem>
      <MenuItem onClick={() => void run(() => api.appsReveal(app.id))}>Open file location</MenuItem>
      {collections.map((collection) => (
        <MenuItem
          key={collection.id}
          onClick={() => void run(async () => {
            const ids = collection.appIds.includes(app.id)
              ? collection.appIds
              : [...collection.appIds, app.id];
            await api.collectionsSetMembers(collection.id, ids);
          })}
        >
          Add to {collection.name}
        </MenuItem>
      ))}
      {app.source === "manual" && !app.discovered ? (
        <MenuItem onClick={() => void run(() => api.appsDeleteManual(app.id))}>Remove custom entry</MenuItem>
      ) : (
        <MenuItem onClick={() => void run(() => api.appsHide(app.id, true))}>Hide from PulseLauncher</MenuItem>
      )}
    </div>
  );
}

function MenuItem({ children, onClick }: { children: ReactNode; onClick: () => void }) {
  return (
    <button role="menuitem" className="block w-full rounded px-3 py-1.5 text-left text-sm hover:bg-panel" onClick={onClick}>
      {children}
    </button>
  );
}

function RegisterDialog({
  collectionId,
  collections,
  onClose,
  onSaved,
}: {
  collectionId: string;
  collections: Collection[];
  onClose: () => void;
  onSaved: (app: AppEntry) => void;
}) {
  const [displayName, setDisplayName] = useState("");
  const [target, setTarget] = useState("");
  const [argumentsText, setArgumentsText] = useState("");
  const [workingDirectory, setWorkingDirectory] = useState("");
  const [tags, setTags] = useState("");
  const [notes, setNotes] = useState("");
  const [collection, setCollection] = useState(collectionId);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  async function submit() {
    const parsed = formSchema.safeParse({ displayName, target, argumentsText, workingDirectory, tags, notes, collectionId: collection });
    if (!parsed.success) {
      setError(parsed.error.issues[0]?.message ?? "Check the form.");
      return;
    }
    setSaving(true);
    try {
      const app = await api.appsRegister({
        displayName: parsed.data.displayName,
        target: parsed.data.target,
        arguments: argumentLines(parsed.data.argumentsText),
        workingDirectory: parsed.data.workingDirectory || null,
        tags: parsed.data.tags.split(",").map((tag) => tag.trim()).filter(Boolean),
        collectionId: parsed.data.collectionId || null,
        notes: parsed.data.notes || null,
      });
      onSaved(app);
    } catch (err) {
      setError(err instanceof DesktopError ? err.message : "Registration failed.");
    } finally {
      setSaving(false);
    }
  }

  return (
    <Dialog title="Add app" onClose={onClose}>
      <div className="space-y-3">
        <p className="text-sm text-muted">Registration checks that the target exists. It does not launch the program.</p>
        {error ? <Banner tone="danger">{error}</Banner> : null}
        <Field label="Executable, shortcut, or URL">
          <div className="flex gap-2">
            <input className={controlClass} value={target} onChange={(event) => setTarget(event.target.value)} />
            <Button onClick={() => void pickPath(false).then((path) => path && setTarget(path)).catch((err: Error) => setError(err.message))}>Browse</Button>
          </div>
        </Field>
        <Field label="Display name">
          <input className={controlClass} value={displayName} onChange={(event) => setDisplayName(event.target.value)} />
        </Field>
        <Field label="Arguments" hint="One argument per line. They are passed as an array, not a shell string.">
          <textarea className={`${controlClass} min-h-20`} value={argumentsText} onChange={(event) => setArgumentsText(event.target.value)} />
        </Field>
        <Field label="Working directory">
          <div className="flex gap-2">
            <input className={controlClass} value={workingDirectory} onChange={(event) => setWorkingDirectory(event.target.value)} />
            <Button onClick={() => void pickPath(true).then((path) => path && setWorkingDirectory(path)).catch((err: Error) => setError(err.message))}>Browse</Button>
          </div>
        </Field>
        <Field label="Tags" hint="Comma separated.">
          <input className={controlClass} value={tags} onChange={(event) => setTags(event.target.value)} />
        </Field>
        <Field label="Collection">
          <select className={controlClass} value={collection} onChange={(event) => setCollection(event.target.value)}>
            <option value="">None</option>
            {collections.map((item) => (
              <option key={item.id} value={item.id}>{item.name}</option>
            ))}
          </select>
        </Field>
        <Field label="Notes">
          <textarea className={`${controlClass} min-h-16`} value={notes} onChange={(event) => setNotes(event.target.value)} />
        </Field>
        <div className="flex justify-end gap-2">
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="primary" disabled={saving} onClick={() => void submit()}>{saving ? "Checking…" : "Save"}</Button>
        </div>
      </div>
    </Dialog>
  );
}
