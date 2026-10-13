import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import * as api from "../../lib/api";
import { formatBytes, sourceLabel, statusLabel, type AppDetails, type Collection } from "../../lib/types";
import { useUi } from "../../lib/store";
import { Chip, Icon } from "../../components/Bits";

export function DetailsView({ appId, fixture }: { appId: string | null; fixture?: AppDetails }) {
  const setNotice = useUi((s) => s.setNotice);
  const setPage = useUi((s) => s.setPage);
  const [details, setDetails] = useState<AppDetails | null>(fixture ?? null);
  const [collections, setCollections] = useState<Collection[]>([]);
  const [tab, setTab] = useState<"preview" | "activity">("preview");
  const [images, setImages] = useState<Record<string, string>>({});
  const [error, setError] = useState<string | null>(null);
  const [editing, setEditing] = useState(false);

  async function load(id: string) {
    try {
      const next = await api.appDetails(id);
      setDetails(next);
      const cols = await api.listCollections();
      setCollections(cols);
      const pics: Record<string, string> = {};
      for (const asset of next.previews) {
        const data = await api.previewData(asset.id);
        if (data) pics[asset.id] = data;
      }
      setImages(pics);
      setError(null);
    } catch (e) {
      setError(api.errorText(e));
    }
  }

  useEffect(() => {
    if (fixture) return;
    if (appId) void load(appId);
  }, [appId, fixture]);

  if (!appId && !details) {
    return (
      <div className="content">
        <div className="empty card" role="status">
          <h2>No app selected</h2>
          <p>Open Details from an app card or press Alt+Enter on a launcher result.</p>
        </div>
      </div>
    );
  }
  if (!details) {
    return <div className="content">{error ? <p role="alert">{error}</p> : <p>Loading…</p>}</div>;
  }
  const app = details.app;
  const target = app.resolvedPath || app.target.path || app.target.uri || app.target.aumid || "";
  const statusKind = details.status === "ok" ? "ok" : details.status === "unverified" ? undefined : "bad";

  async function launch(extra: string[] = []) {
    try {
      const result = await api.launchApp(app.id, extra);
      setNotice(result.message);
      if (!fixture) await load(app.id);
    } catch (e) {
      setNotice(api.errorText(e));
    }
  }

  return (
    <div className="content">
      <button className="btn-ghost" onClick={() => setPage("apps")}>
        Back to apps
      </button>
      <section className="card hero" aria-label="App summary" style={{ marginTop: 12 }}>
        <Icon src={details.iconDataUrl} name={app.name} />
        <div>
          <h1 style={{ margin: 0 }}>{app.name}</h1>
          <p className="tiny">
            {details.metadata.publisher || app.publisher || "Publisher not reported"} · {details.metadata.version || app.version || "Version not reported"}
          </p>
          <div className="row">
            <Chip kind={statusKind}>{statusLabel(details.status)}</Chip>
            <Chip>{sourceLabel(app.source)}</Chip>
            <Chip>{app.source === "manual" ? "Portable / custom" : "Discovered"}</Chip>
          </div>
        </div>
      </section>
      <div className="row" style={{ margin: "12px 0" }}>
        <button className="btn" onClick={() => void launch()}>
          Launch
        </button>
        <button className="btn-ghost" onClick={() => void launch(app.target.args)} disabled={app.target.args.length === 0}>
          Run with configured arguments
        </button>
        <button className="btn-ghost" disabled={!target} onClick={() => void api.revealPath(app.target.kind === "shortcut" ? app.target.path : target).catch((e) => setNotice(api.errorText(e)))}>
          Open file location
        </button>
        <button className="btn-ghost" disabled={!target} onClick={() => void api.copyText(target).then(() => setNotice("Path copied."))}>
          Copy path
        </button>
        <button className="btn-ghost" onClick={() => void api.updateApp(app.id, { favorite: !app.favorite }).then(() => load(app.id))}>
          {app.favorite ? "Unpin" : "Favorite"}
        </button>
        {app.source === "manual" && (
          <button className="btn-ghost" onClick={() => setEditing(true)}>
            Edit registration
          </button>
        )}
        <button
          className="btn-danger"
          onClick={() =>
            void api.removeApp(app.id).then((deleted) => {
              setNotice(deleted ? "Registration removed." : "Hidden from PulseLauncher. Windows was not asked to uninstall it.");
              setPage("apps");
            })
          }
        >
          {app.source === "manual" ? "Remove custom entry" : "Hide from PulseLauncher"}
        </button>
      </div>
      {details.status === "missingTarget" && <p role="alert">The target is missing. Use Edit registration if this is a custom entry, or rescan if Windows moved the shortcut.</p>}
      <div className="tabs" role="tablist">
        <button className="btn-ghost" role="tab" aria-selected={tab === "preview"} onClick={() => setTab("preview")}>
          Preview
        </button>
        <button className="btn-ghost" role="tab" aria-selected={tab === "activity"} onClick={() => setTab("activity")}>
          Activity
        </button>
      </div>
      {tab === "preview" ? (
        <div className="split">
          <div className="card">
            {details.previews.length === 0 ? (
              <div className="empty" role="status">
                <strong>No preview available</strong>
                <p>PulseLauncher only shows a screenshot you add yourself. It does not download product images.</p>
              </div>
            ) : (
              details.previews.map((asset) => (
                <figure key={asset.id}>
                  {images[asset.id] ? <img src={images[asset.id]} alt={asset.caption || `${app.name} preview`} style={{ maxWidth: "100%", borderRadius: 8 }} /> : <p>Preview file missing.</p>}
                  <button className="btn-ghost" onClick={() => void api.deletePreview(asset.id).then(() => load(app.id))}>
                    Remove preview
                  </button>
                </figure>
              ))
            )}
            <button
              className="btn-ghost"
              onClick={() =>
                void open({ multiple: false, filters: [{ name: "Image", extensions: ["png", "jpg", "jpeg"] }] }).then(async (picked) => {
                  if (typeof picked !== "string") return;
                  await api.importPreview(app.id, picked);
                  await load(app.id);
                }).catch((e) => setNotice(api.errorText(e)))
              }
            >
              Add screenshot
            </button>
          </div>
          <dl className="card">
            <dt className="tiny">Executable / target</dt>
            <dd>{target || "Not available"}</dd>
            <dt className="tiny">Source</dt>
            <dd>{app.provenance || sourceLabel(app.source)}</dd>
            <dt className="tiny">Working directory</dt>
            <dd>{app.target.workingDir || "Default"}</dd>
            <dt className="tiny">Arguments</dt>
            <dd>{app.target.args.length ? details.commandPreview : "None"}</dd>
            <dt className="tiny">Size</dt>
            <dd>{formatBytes(details.metadata.fileSize)}</dd>
            <dt className="tiny">Launches recorded</dt>
            <dd>{app.launchCount}</dd>
            <dt className="tiny">Last launch</dt>
            <dd>{app.lastLaunchedAt || "Never"}</dd>
            <dt className="tiny">Tags</dt>
            <dd>{app.tags.join(", ") || "None"}</dd>
            <dt className="tiny">Collections</dt>
            <dd>
              {collections.length === 0 && "None"}
              {collections.map((c) => (
                <label key={c.id} className="tiny" style={{ display: "block" }}>
                  <input
                    type="checkbox"
                    checked={app.collectionIds.includes(c.id)}
                    onChange={(e) => void api.setMembership(app.id, c.id, e.target.checked).then(() => load(app.id))}
                  />{" "}
                  {c.name}
                </label>
              ))}
            </dd>
            {details.duplicates.length > 0 && (
              <>
                <dt className="tiny">Same target also registered as</dt>
                <dd>{details.duplicates.map((d) => d.name).join(", ")}</dd>
              </>
            )}
          </dl>
        </div>
      ) : (
        <div className="card">
          {details.events.length === 0 ? (
            <p className="empty">No launch attempts recorded for this app.</p>
          ) : (
            <ul>
              {details.events.map((event) => (
                <li key={event.id}>
                  <Chip kind={event.outcome === "accepted" ? "ok" : "bad"}>{event.outcome}</Chip> {event.at} — {event.detail}
                  {event.pid != null && ` (pid ${event.pid})`}
                </li>
              ))}
            </ul>
          )}
        </div>
      )}
      {error && <p role="alert">{error}</p>}
      {editing && (
        <EditDialog
          app={app}
          onClose={() => setEditing(false)}
          onSaved={async () => {
            setEditing(false);
            await load(app.id);
          }}
        />
      )}
    </div>
  );
}

function EditDialog({ app, onClose, onSaved }: { app: AppDetails["app"]; onClose: () => void; onSaved: () => void }) {
  const [name, setName] = useState(app.name);
  const [path, setPath] = useState(app.target.path);
  const [args, setArgs] = useState(app.target.args.join("\n"));
  const [workingDir, setWorkingDir] = useState(app.target.workingDir ?? "");
  const [tags, setTags] = useState(app.tags.join(", "));
  const [error, setError] = useState<string | null>(null);
  return (
    <div className="overlay">
      <form
        className="dialog"
        onSubmit={(e) => {
          e.preventDefault();
          void api
            .updateApp(app.id, {
              name,
              path,
              args: args.split(/\r?\n/).map((s) => s.trim()).filter(Boolean),
              workingDir,
              tags: tags.split(",").map((s) => s.trim()).filter(Boolean),
            })
            .then(() => onSaved())
            .catch((err) => setError(api.errorText(err)));
        }}
      >
        <h2>Edit registration</h2>
        <input className="field" aria-label="Display name" value={name} onChange={(e) => setName(e.target.value)} />
        <input className="field" aria-label="Program path" value={path} onChange={(e) => setPath(e.target.value)} />
        <textarea className="field" aria-label="Arguments" value={args} onChange={(e) => setArgs(e.target.value)} />
        <input className="field" aria-label="Working folder" value={workingDir} onChange={(e) => setWorkingDir(e.target.value)} />
        <input className="field" aria-label="Tags" value={tags} onChange={(e) => setTags(e.target.value)} />
        {error && <p role="alert">{error}</p>}
        <div className="row">
          <button className="btn" type="submit">Save</button>
          <button className="btn-ghost" type="button" onClick={onClose}>Cancel</button>
        </div>
      </form>
    </div>
  );
}
