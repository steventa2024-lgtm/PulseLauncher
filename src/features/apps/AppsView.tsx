import { useEffect, useMemo, useState, type DragEvent } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import * as api from "../../lib/api";
import { sourceLabel, type AppEntry, type Collection, type ScanReport } from "../../lib/types";
import { useUi } from "../../lib/store";
import { Chip, Empty, Icon } from "../../components/Bits";

type Filter = "all" | "zeropulse" | "developer" | "productivity" | "favorites" | "recent" | "portable" | string;

const desktop = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export function AppsView({ initialApps, initialCollections }: { initialApps?: AppEntry[]; initialCollections?: Collection[] }) {
  const openApp = useUi((s) => s.openApp);
  const setNotice = useUi((s) => s.setNotice);
  const [apps, setApps] = useState<AppEntry[]>(initialApps ?? []);
  const [collections, setCollections] = useState<Collection[]>(initialCollections ?? []);
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<Filter>("all");
  const [layout, setLayout] = useState<"grid" | "list">("grid");
  const [error, setError] = useState<string | null>(null);
  const [scanReport, setScanReport] = useState<ScanReport | null>(null);
  const [busy, setBusy] = useState(false);
  const [menu, setMenu] = useState<{ x: number; y: number; app: AppEntry } | null>(null);
  const [wizard, setWizard] = useState(false);

  async function reload() {
    try {
      const [nextApps, nextCollections] = await Promise.all([api.listApps(false), api.listCollections()]);
      setApps(Array.isArray(nextApps) ? nextApps : []);
      setCollections(Array.isArray(nextCollections) ? nextCollections : []);
      setError(null);
    } catch (e) {
      setError(api.errorText(e));
    }
  }

  useEffect(() => {
    if (!initialApps) void reload();
  }, []);

  const counts = useMemo(() => {
    const zp = (collections ?? []).find((c) => c.systemKey === "zeropulse");
    return {
      installed: apps.filter((a) => a.source !== "manual").length,
      portable: apps.filter((a) => a.source === "manual" && a.target.kind === "executable").length,
      favorites: apps.filter((a) => a.favorite).length,
      recent: apps.filter((a) => a.lastLaunchedAt).length,
      zeropulse: zp ? apps.filter((a) => a.collectionIds.includes(zp.id)).length : 0,
    };
  }, [apps, collections]);

  const visible = apps
    .filter((app) => {
      const q = query.trim().toLowerCase();
      if (q && !`${app.name} ${app.tags.join(" ")} ${app.target.path}`.toLowerCase().includes(q)) return false;
      if (filter === "favorites") return app.favorite;
      if (filter === "recent") return Boolean(app.lastLaunchedAt);
      if (filter === "portable") return app.source === "manual";
      if (filter === "developer") return app.category === "developerTools";
      if (filter === "productivity") return app.category === "productivity";
      if (filter === "zeropulse") {
        const zp = (collections ?? []).find((c) => c.systemKey === "zeropulse");
        return zp ? app.collectionIds.includes(zp.id) : false;
      }
      if (filter.startsWith("col:")) return app.collectionIds.includes(filter.slice(4));
      return true;
    })
    .slice()
    .sort((a, b) => a.sortOrder - b.sortOrder || a.name.localeCompare(b.name));

  async function launch(app: AppEntry) {
    try {
      const result = await api.launchApp(app.id);
      setNotice(result.message);
      if (result.outcome === "accepted") void reload();
    } catch (e) {
      setNotice(api.errorText(e));
    }
  }

  async function onScan() {
    setBusy(true);
    try {
      const report = await api.scan();
      setScanReport(report);
      await reload();
    } catch (e) {
      setError(api.errorText(e));
    } finally {
      setBusy(false);
    }
  }

  function onDragStart(event: DragEvent, id: string) {
    event.dataTransfer.setData("text/plain", id);
  }
  async function onDrop(event: DragEvent, targetId: string) {
    event.preventDefault();
    const sourceId = event.dataTransfer.getData("text/plain");
    if (!sourceId || sourceId === targetId) return;
    const ids = visible.map((a) => a.id);
    const from = ids.indexOf(sourceId);
    const to = ids.indexOf(targetId);
    if (from < 0 || to < 0) return;
    ids.splice(to, 0, ids.splice(from, 1)[0]);
    try {
      await api.reorderApps(ids);
      await reload();
    } catch (e) {
      setNotice(api.errorText(e));
    }
  }

  return (
    <div>
      <div className="topbar">
        <h1>Apps & Tools</h1>
        <input className="search" aria-label="Filter apps" placeholder="Filter apps" value={query} onChange={(e) => setQuery(e.target.value)} />
        <button className="btn-ghost" onClick={() => setLayout(layout === "grid" ? "list" : "grid")}>
          {layout === "grid" ? "List" : "Grid"}
        </button>
        <button className="btn-ghost" onClick={() => void onScan()} disabled={busy}>
          {busy ? "Scanning…" : "Rescan"}
        </button>
        <button className="btn" onClick={() => setWizard(true)}>
          Add App
        </button>
      </div>
      <div className="content" onClick={() => setMenu(null)}>
        <div className="tabs" role="tablist" aria-label="App categories">
          {(
            [
              ["all", `All (${apps.length})`],
              ["zeropulse", `ZeroPulse (${counts.zeropulse})`],
              ["developer", "Developer Tools"],
              ["productivity", "Productivity"],
              ["favorites", `Favorites (${counts.favorites})`],
              ["recent", `Recent (${counts.recent})`],
              ["portable", `Portable (${counts.portable})`],
            ] as const
          ).map(([id, label]) => (
            <button key={id} className="btn-ghost" role="tab" aria-selected={filter === id} onClick={() => setFilter(id)}>
              {label}
            </button>
          ))}
          {collections
            .filter((c) => !c.systemKey)
            .map((c) => (
              <button key={c.id} className="btn-ghost" role="tab" aria-selected={filter === `col:${c.id}`} onClick={() => setFilter(`col:${c.id}`)}>
                {c.name} ({c.appCount})
              </button>
            ))}
        </div>
        <p className="tiny">
          Installed {counts.installed} · Portable {counts.portable} · Favorites {counts.favorites} · Recent {counts.recent}. Discovered apps are hidden from PulseLauncher, not uninstalled.
        </p>
        {error && (
          <p className="notice" role="alert">
            {error}
          </p>
        )}
        {scanReport && (
          <p className="tiny">
            Last scan found {scanReport.found}, added {scanReport.added}, updated {scanReport.updated}, stale {scanReport.markedStale}.
            {scanReport.issues.length > 0 && ` ${scanReport.issues.length} folder(s) could not be read.`}
          </p>
        )}
        {visible.length === 0 ? (
          <Empty
            title={apps.length === 0 ? "No applications yet" : "Nothing in this view"}
            body={
              apps.length === 0
                ? "PulseLauncher does not invent apps. Rescan the Start Menu, or register a portable .exe you actually have."
                : "Try another category or clear the filter."
            }
            action={
              <button className="btn" onClick={() => setWizard(true)}>
                Add App
              </button>
            }
          />
        ) : (
          <div className={layout === "grid" ? "grid" : "list"}>
            {visible.map((app) => (
              <article
                key={app.id}
                className="card"
                draggable
                onDragStart={(e) => onDragStart(e, app.id)}
                onDragOver={(e) => e.preventDefault()}
                onDrop={(e) => void onDrop(e, app.id)}
                onContextMenu={(e) => {
                  e.preventDefault();
                  setMenu({ x: e.clientX, y: e.clientY, app });
                }}
              >
                <header>
                  <Icon name={app.name} />
                  <div style={{ minWidth: 0 }}>
                    <h2>{app.name}</h2>
                    <div className="tiny">{sourceLabel(app.source)}</div>
                  </div>
                  {app.favorite && <Chip>Pinned</Chip>}
                  {app.stale && <Chip kind="warn">Not in last scan</Chip>}
                </header>
                <p className="tiny" title={app.target.path}>
                  {app.resolvedPath || app.target.path || app.target.aumid || "No path"}
                </p>
                <div className="row">
                  <button className="btn" onClick={() => void launch(app)}>
                    Run
                  </button>
                  <button className="btn-ghost" onClick={() => openApp(app.id)}>
                    Details
                  </button>
                </div>
              </article>
            ))}
          </div>
        )}
        {menu && (
          <div className="menu" style={{ left: menu.x, top: menu.y }} role="menu">
            <button onClick={() => void launch(menu.app)}>Run</button>
            <button onClick={() => openApp(menu.app.id)}>Details</button>
            <button
              onClick={() =>
                void api.updateApp(menu.app.id, { favorite: !menu.app.favorite }).then(() => reload())
              }
            >
              {menu.app.favorite ? "Unpin" : "Pin"}
            </button>
            <button
              onClick={() => {
                const zp = (collections ?? []).find((c) => c.systemKey === "zeropulse");
                if (!zp) return;
                const member = menu.app.collectionIds.includes(zp.id);
                void api.setMembership(menu.app.id, zp.id, !member).then(() => reload());
              }}
            >
              Toggle ZeroPulse collection
            </button>
            <button
              onClick={() =>
                void api.removeApp(menu.app.id).then((deleted) => {
                  setNotice(deleted ? "Registration removed." : "Hidden from PulseLauncher. The program itself was not uninstalled.");
                  return reload();
                })
              }
            >
              {menu.app.source === "manual" ? "Remove registration" : "Hide from PulseLauncher"}
            </button>
          </div>
        )}
      </div>
      {wizard && (
        <AddWizard
          onClose={() => setWizard(false)}
          onSaved={async (message) => {
            setWizard(false);
            setNotice(message);
            await reload();
          }}
        />
      )}
      {!desktop && initialApps === undefined && apps.length === 0 && !error && (
        <p className="tiny">Desktop bridge is used when this screen runs inside PulseLauncher.</p>
      )}
    </div>
  );
}

function AddWizard({ onClose, onSaved }: { onClose: () => void; onSaved: (message: string) => void }) {
  const [name, setName] = useState("");
  const [path, setPath] = useState("");
  const [args, setArgs] = useState("");
  const [workingDir, setWorkingDir] = useState("");
  const [category, setCategory] = useState("none");
  const [error, setError] = useState<string | null>(null);

  async function browse() {
    try {
      const picked = await open({ multiple: false, filters: [{ name: "Program", extensions: ["exe", "lnk"] }] });
      if (typeof picked === "string") {
        setPath(picked);
        if (!name) {
          const base = picked.split(/[/\\]/).pop() ?? "";
          setName(base.replace(/\.(exe|lnk)$/i, ""));
        }
      }
    } catch (e) {
      setError(api.errorText(e));
    }
  }

  async function save() {
    setError(null);
    try {
      const entry = await api.addApp({
        name,
        path,
        args: args.split(/\r?\n/).map((s) => s.trim()).filter(Boolean),
        workingDir: workingDir || undefined,
        category,
      });
      onSaved(`Registered ${entry.name}.`);
    } catch (e) {
      setError(api.errorText(e));
    }
  }

  return (
    <div className="overlay" role="presentation">
      <form
        className="dialog"
        aria-labelledby="add-title"
        onSubmit={(e) => {
          e.preventDefault();
          void save();
        }}
      >
        <h2 id="add-title">Add App</h2>
        <p className="tiny">Choose a real .exe or shortcut. PulseLauncher stores the path; it does not copy or modify the program.</p>
        <label className="tiny">Display name</label>
        <input className="field" aria-label="Display name" value={name} onChange={(e) => setName(e.target.value)} required />
        <label className="tiny">Program path</label>
        <div className="row">
          <input className="field" aria-label="Program path" value={path} onChange={(e) => setPath(e.target.value)} required />
          <button type="button" className="btn-ghost" onClick={() => void browse()}>
            Browse
          </button>
        </div>
        <label className="tiny">Arguments, one per line</label>
        <textarea className="field" aria-label="Arguments" rows={3} value={args} onChange={(e) => setArgs(e.target.value)} />
        <label className="tiny">Working folder (optional)</label>
        <input className="field" aria-label="Working folder" value={workingDir} onChange={(e) => setWorkingDir(e.target.value)} />
        <label className="tiny">Category</label>
        <select className="field" aria-label="Category" value={category} onChange={(e) => setCategory(e.target.value)}>
          <option value="none">None</option>
          <option value="developerTools">Developer Tools</option>
          <option value="productivity">Productivity</option>
        </select>
        {error && (
          <p role="alert" className="notice">
            {error}
          </p>
        )}
        <div className="row" style={{ marginTop: 12 }}>
          <button className="btn" type="submit">
            Save
          </button>
          <button className="btn-ghost" type="button" onClick={onClose}>
            Cancel
          </button>
        </div>
      </form>
    </div>
  );
}
