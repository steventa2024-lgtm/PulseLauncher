import { useEffect, useState } from "react";
import { api, errorText } from "../../lib/api";
import { formatBytes, formatWhen, kindLabel, sourceLabel, targetText } from "../../lib/text";
import type { AppDetails, Collection } from "../../lib/types";
import { AppMark, Field } from "../../components/ui";

export function DetailsView({
  id,
  collections,
  icon,
  onBack,
  onChanged,
  onNotice,
}: {
  id: string;
  collections: Collection[];
  icon?: string | null;
  onBack: () => void;
  onChanged: () => void;
  onNotice: (title: string, body: string) => void;
}) {
  const [details, setDetails] = useState<AppDetails | null>(null);
  const [tab, setTab] = useState<"overview" | "preview" | "activity">("overview");
  const [editing, setEditing] = useState(false);
  const [error, setError] = useState("");

  function load() {
    api.getApp(id).then(setDetails).catch((e) => setError(errorText(e)));
  }
  useEffect(load, [id]);

  if (error) return <p className="text-[var(--color-danger)]">{error}</p>;
  if (!details) return <p className="text-[var(--color-muted)]">Loading details…</p>;
  const app = details.app;
  const manual = app.source === "manual";

  async function act(work: () => Promise<{ message: string } | void>, title: string) {
    try {
      const result = await work();
      onNotice(title, result && "message" in result ? result.message : "Done");
      onChanged();
      load();
    } catch (e) {
      onNotice(title, errorText(e));
    }
  }

  return (
    <div className="flex flex-col gap-4 pb-8">
      <button className="btn w-fit" onClick={onBack}>Back to apps</button>
      <section className="card flex flex-wrap items-start gap-4 p-5">
        <AppMark name={app.name} icon={icon} size={72} />
        <div className="min-w-0 flex-1">
          <h2 className="text-2xl font-semibold">{app.name}</h2>
          <p className="text-[var(--color-muted)]">
            {[app.metadata.productName, app.metadata.publisher, app.metadata.version].filter(Boolean).join(" · ") || "Version and publisher were not available in the file."}
          </p>
          <div className="mt-2 flex flex-wrap gap-2">
            <span className="pill">{kindLabel(app.target.kind)}</span>
            <span className="pill">{sourceLabel(app.source)}</span>
            <span className={`pill ${app.status === "missing" ? "pill-danger" : app.status === "ok" ? "pill-ok" : ""}`}>
              {app.status === "ok" ? "Target present" : app.status === "missing" ? "Target missing" : "Not verified"}
            </span>
            {app.portable || manual ? <span className="pill">Portable / custom</span> : null}
          </div>
        </div>
        <div className="flex flex-wrap gap-2">
          <button className="btn btn-primary" onClick={() => void act(() => api.launchApp(app.id), "Launch")}>Launch</button>
          <button className="btn" onClick={() => void act(() => api.launchApp(app.id, app.target.arguments), "Launch")}>Run with arguments</button>
          <button className="btn" onClick={() => void act(() => api.revealApp(app.id), "Location")}>Open file location</button>
          <button className="btn" onClick={() => void act(async () => { await api.copyText(targetText(app)); return { message: "Path copied" }; }, "Copy")}>Copy path</button>
        </div>
      </section>
      <div className="flex gap-2" role="tablist">
        {(["overview", "preview", "activity"] as const).map((name) => (
          <button key={name} role="tab" aria-selected={tab === name} className={`btn ${tab === name ? "btn-primary" : ""}`} onClick={() => setTab(name)}>
            {name[0]!.toUpperCase() + name.slice(1)}
          </button>
        ))}
      </div>
      {tab === "overview" ? (
        <section className="card grid gap-3 p-5 md:grid-cols-2">
          <Info label="Target" value={targetText(app)} />
          <Info label="Launch entry" value={app.target.launchTarget} />
          <Info label="Arguments" value={app.target.arguments.length ? app.target.arguments.join(" ") : app.target.linkArguments || "None"} />
          <Info label="Working directory" value={app.target.workingDir || "Not set"} />
          <Info label="Size" value={formatBytes(app.metadata.fileSize)} />
          <Info label="Launches" value={`${app.launchCount} · last ${formatWhen(app.lastLaunchedAt)}`} />
          <Info label="Description" value={app.metadata.description || "Not available"} />
          <Info label="Tags" value={app.tags.length ? app.tags.join(", ") : "None"} />
          <div className="flex flex-wrap gap-2 md:col-span-2">
            <button className="btn" onClick={() => void act(async () => { await api.updateApp(app.id, { favorite: !app.favorite }); }, "Favorite")}>
              {app.favorite ? "Remove favorite" : "Favorite"}
            </button>
            <button className="btn" onClick={() => setEditing(true)}>{manual ? "Edit registration" : "Rename and tag"}</button>
            <button className="btn" onClick={() => void act(async () => api.removeApp(app.id), "Remove")}>
              {manual ? "Remove custom entry" : "Hide from PulseLauncher"}
            </button>
          </div>
          <div className="md:col-span-2">
            <p className="mb-2 text-sm text-[var(--color-muted)]">Collections</p>
            <div className="flex flex-wrap gap-2">
              {collections.map((c) => {
                const on = c.appIds.includes(app.id);
                return (
                  <button key={c.id} className={`pill ${on ? "border-[var(--color-accent)]" : ""}`} onClick={() => void act(async () => { await api.setMembership(c.id, app.id, !on); }, c.name)}>
                    {on ? "In" : "Add to"} {c.name}
                  </button>
                );
              })}
            </div>
          </div>
        </section>
      ) : null}
      {tab === "preview" ? (
        <section className="card p-5">
          {details.previews[0]?.dataUrl ? (
            <img src={details.previews[0].dataUrl} alt={`Preview of ${app.name}`} className="max-h-80 rounded-xl border border-[var(--color-line)]" />
          ) : (
            <div className="flex flex-col items-start gap-3">
              <AppMark name={app.name} icon={icon} size={96} />
              <h3 className="text-lg">No preview available</h3>
              <p className="max-w-xl text-[var(--color-muted)]">PulseLauncher does not download product images and does not capture other windows. You can import a screenshot you made yourself. It is stored with PulseLauncher's data and you can delete it.</p>
            </div>
          )}
          <div className="mt-4 flex gap-2">
            <button className="btn" onClick={() => void importImage(app.id, onNotice, load)}>Import screenshot</button>
            {details.previews.length ? <button className="btn" onClick={() => void act(async () => { await api.clearPreviews(app.id); }, "Preview")}>Remove preview</button> : null}
          </div>
        </section>
      ) : null}
      {tab === "activity" ? (
        <section className="card p-5">
          {details.events.length === 0 ? <p className="text-[var(--color-muted)]">No launches recorded yet.</p> : (
            <ul className="flex flex-col gap-2">
              {details.events.map((event) => (
                <li key={event.id} className="border-b border-[var(--color-line)] pb-2">
                  <span className={event.success ? "text-[var(--color-ok)]" : "text-[var(--color-danger)]"}>{event.success ? "Accepted" : "Failed"}</span>
                  <span className="ml-2 text-sm text-[var(--color-muted)]">{formatWhen(event.at)}{event.processId ? ` · process ${event.processId}` : ""}</span>
                  <p>{event.message}</p>
                </li>
              ))}
            </ul>
          )}
        </section>
      ) : null}
      {editing ? (
        <EditForm app={app} onClose={() => setEditing(false)} onSaved={() => { setEditing(false); load(); onChanged(); }} onNotice={onNotice} />
      ) : null}
    </div>
  );
}

function Info({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <div className="text-xs uppercase tracking-wide text-[var(--color-faint)]">{label}</div>
      <div className="break-all">{value}</div>
    </div>
  );
}

async function importImage(appId: string, onNotice: (t: string, b: string) => void, reload: () => void) {
  try {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const picked = await open({ multiple: false, title: "Choose a screenshot you created", filters: [{ name: "Image", extensions: ["png", "jpg", "jpeg", "gif", "webp", "bmp"] }] });
    if (typeof picked !== "string") return;
    await api.importPreview(appId, picked);
    onNotice("Preview saved", "The image was copied into PulseLauncher's data.");
    reload();
  } catch (e) {
    onNotice("Preview", errorText(e));
  }
}

function EditForm({ app, onClose, onSaved, onNotice }: { app: AppDetails["app"]; onClose: () => void; onSaved: () => void; onNotice: (t: string, b: string) => void }) {
  const manual = app.source === "manual";
  const [name, setName] = useState(app.name);
  const [path, setPath] = useState(app.target.launchTarget);
  const [args, setArgs] = useState(app.target.arguments.join(" "));
  const [cwd, setCwd] = useState(app.target.workingDir ?? "");
  const [tags, setTags] = useState(app.tags.join(", "));
  const [category, setCategory] = useState(app.category ?? "");

  async function save() {
    try {
      await api.updateApp(app.id, {
        name,
        launchTarget: manual ? path : undefined,
        arguments: manual ? args.split(" ").filter(Boolean) : undefined,
        workingDir: manual ? cwd || null : undefined,
        category: category || null,
        tags: tags.split(",").map((t) => t.trim()).filter(Boolean),
      });
      onNotice("Saved", "The registration was updated.");
      onSaved();
    } catch (e) {
      onNotice("Could not save", errorText(e));
    }
  }

  return (
    <div className="card p-5">
      <h3 className="mb-3 font-semibold">{manual ? "Edit registration" : "Rename, category and tags"}</h3>
      {!manual ? <p className="mb-3 text-sm text-[var(--color-muted)]">This entry came from Windows. Its target stays as Windows recorded it. Hiding it does not uninstall the program.</p> : null}
      <div className="grid gap-3 md:grid-cols-2">
        <Field label="Name"><input className="field" value={name} onChange={(e) => setName(e.target.value)} /></Field>
        <Field label="Category"><input className="field" value={category} onChange={(e) => setCategory(e.target.value)} /></Field>
        {manual ? <Field label="Target"><input className="field" value={path} onChange={(e) => setPath(e.target.value)} /></Field> : null}
        {manual ? <Field label="Arguments"><input className="field" value={args} onChange={(e) => setArgs(e.target.value)} /></Field> : null}
        {manual ? <Field label="Working directory"><input className="field" value={cwd} onChange={(e) => setCwd(e.target.value)} /></Field> : null}
        <Field label="Tags" hint="Comma separated"><input className="field" value={tags} onChange={(e) => setTags(e.target.value)} /></Field>
      </div>
      <div className="mt-3 flex gap-2">
        <button className="btn btn-primary" onClick={() => void save()}>Save</button>
        <button className="btn" onClick={onClose}>Cancel</button>
      </div>
    </div>
  );
}
