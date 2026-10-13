import { useEffect, useState } from "react";
import { Banner, Button, Dialog, EmptyState, Field, StatusBadge, controlClass } from "../components/ui";
import { api, pickPath } from "../lib/api";
import type { AppEntry, Collection, LaunchEvent, LaunchPreview, PreviewAsset } from "../lib/types";
import { argumentLines, formatBytes, sourceLabel } from "../lib/text";
import { useShell } from "../store";

type Tab = "overview" | "preview" | "activity";

export function DetailsView() {
  const selectedAppId = useShell((state) => state.selectedAppId);
  const notify = useShell((state) => state.notify);
  const setView = useShell((state) => state.setView);
  const [app, setApp] = useState<AppEntry | null>(null);
  const [events, setEvents] = useState<LaunchEvent[]>([]);
  const [previews, setPreviews] = useState<PreviewAsset[]>([]);
  const [image, setImage] = useState<string | null>(null);
  const [icon, setIcon] = useState<string | null>(null);
  const [preview, setPreview] = useState<LaunchPreview | null>(null);
  const [collections, setCollections] = useState<Collection[]>([]);
  const [tab, setTab] = useState<Tab>("overview");
  const [error, setError] = useState<string | null>(null);
  const [editing, setEditing] = useState(false);

  async function load(id: string) {
    try {
      const [entry, history, shots, groups] = await Promise.all([
        api.appsGet(id),
        api.historyList(id),
        api.previewsList(id),
        api.collectionsList(),
      ]);
      setApp(entry);
      setEvents(history);
      setPreviews(shots);
      setCollections(groups);
      setError(null);
      const [iconUrl, command] = await Promise.all([
        entry.iconPath ? api.iconsRead(entry.iconPath).catch(() => null) : Promise.resolve(null),
        api.appsLaunchPreview(id).catch(() => null),
      ]);
      setIcon(iconUrl);
      setPreview(command);
      if (shots[0]) {
        setImage(await api.previewData(shots[0].id).catch(() => null));
      } else {
        setImage(null);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not load this app.");
    }
  }

  useEffect(() => {
    if (selectedAppId) void load(selectedAppId);
  }, [selectedAppId]);

  if (!selectedAppId) {
    return (
      <EmptyState
        title="No app selected"
        body="Open an app from the grid or from a launcher result to see its real metadata."
        action={<Button onClick={() => setView("apps")}>Back to apps</Button>}
      />
    );
  }
  if (!app) {
    return error ? <Banner tone="danger">{error}</Banner> : <p className="text-sm text-muted">Loading details…</p>;
  }

  const current = app;
  async function launch() {
    try {
      const outcome = await api.appsLaunch(current.id);
      notify(outcome.message);
      await load(current.id);
    } catch (err) {
      notify(err instanceof Error ? err.message : "Launch failed.");
    }
  }

  return (
    <section className="space-y-4">
      <button className="text-sm text-muted" onClick={() => setView("apps")}>Apps & Tools</button>
      <div className="rounded-card border border-line bg-panel p-5">
        <div className="flex flex-wrap items-start gap-4">
          <div className="flex h-16 w-16 items-center justify-center overflow-hidden rounded-xl border border-line bg-ink text-xl font-semibold">
            {icon ? <img src={icon} alt="" className="h-full w-full object-contain" /> : app.displayName.slice(0, 1)}
          </div>
          <div className="min-w-0 flex-1">
            <h1 className="text-2xl font-semibold">{app.displayName}</h1>
            <p className="text-sm text-muted">
              {app.version ? `Version ${app.version}` : "Version not provided by the system"} · {app.publisher ?? "Publisher not provided"}
            </p>
            <div className="mt-2 flex flex-wrap gap-2 text-xs">
              <StatusBadge status={app.missing ? "missing" : "ready"} />
              <span className="text-muted">{app.discovered ? "Discovered" : "Manually registered"}</span>
              <span className="text-muted">{app.source === "manual" ? "Portable / custom" : "Installed entry"}</span>
            </div>
          </div>
        </div>
        {app.missing ? (
          <div className="mt-4">
            <Banner tone="danger">The target is missing. Edit the path to a file that exists. PulseLauncher will not report a launch as successful.</Banner>
          </div>
        ) : null}
        <div className="mt-4 flex flex-wrap gap-2">
          <Button variant="primary" disabled={app.missing} onClick={() => void launch()}>Launch</Button>
          <Button disabled={app.missing || app.arguments.length === 0} onClick={() => void launch()}>Run with configured arguments</Button>
          <Button onClick={() => void api.appsReveal(app.id).then((outcome) => notify(outcome.message)).catch((err: Error) => notify(err.message))}>Open file location</Button>
          <Button onClick={() => void api.appsCopyPath(app.id).then(() => notify("Path copied.")).catch((err: Error) => notify(err.message))}>Copy path</Button>
          <Button onClick={() => setEditing(true)}>Edit registration</Button>
          <Button onClick={() => void api.appsFavorite(app.id, !app.favorite).then(() => load(app.id))}>{app.favorite ? "Unfavorite" : "Favorite"}</Button>
        </div>
        {app.arguments.length === 0 ? (
          <p className="mt-2 text-xs text-muted">No extra arguments are stored, so “Run with configured arguments” stays disabled. Launch uses the registered target only.</p>
        ) : null}
      </div>
      <div className="flex gap-2" role="tablist" aria-label="App detail sections">
        {(["overview", "preview", "activity"] as Tab[]).map((item) => (
          <button key={item} role="tab" aria-selected={tab === item} className={`rounded-lg px-3 py-1.5 text-sm ${tab === item ? "bg-panel" : "text-muted"}`} onClick={() => setTab(item)}>
            {item === "overview" ? "Overview" : item === "preview" ? "Preview" : "Activity"}
          </button>
        ))}
      </div>
      {error ? <Banner tone="danger">{error}</Banner> : null}
      {tab === "overview" ? (
        <div className="grid gap-3 md:grid-cols-2">
          <Info label="Target" value={app.launchTarget} />
          <Info label="Resolved target" value={app.resolvedTarget ?? "Not resolved"} />
          <Info label="Source" value={sourceLabel(app.source)} />
          <Info label="Working directory" value={app.workingDirectory ?? "Not set"} />
          <Info label="Category" value={app.category} />
          <Info label="Tags" value={app.tags.length ? app.tags.join(", ") : "None"} />
          <Info label="Size" value={formatBytes(app.sizeBytes)} />
          <Info label="Launches" value={`${app.launchCount}${app.lastLaunchedAt ? ` · last ${app.lastLaunchedAt}` : ""}`} />
          <Info label="Command preview" value={preview?.display ?? "Preview unavailable until the target can be checked."} />
          <Info label="Notes" value={app.notes ?? "None"} />
          <div className="md:col-span-2">
            <Field label="Add to collection">
              <select
                className={controlClass}
                defaultValue=""
                onChange={(event) => {
                  const id = event.target.value;
                  const collection = collections.find((item) => item.id === id);
                  if (!collection) return;
                  const appIds = collection.appIds.includes(app.id) ? collection.appIds : [...collection.appIds, app.id];
                  void api.collectionsSetMembers(id, appIds).then(() => notify(`Added to ${collection.name}.`)).catch((err: Error) => notify(err.message));
                }}
              >
                <option value="">Choose a collection</option>
                {collections.map((collection) => (
                  <option key={collection.id} value={collection.id}>{collection.name}</option>
                ))}
              </select>
            </Field>
          </div>
          <div>
            {app.source === "manual" && !app.discovered ? (
              <Button variant="danger" onClick={() => void api.appsDeleteManual(app.id).then(() => setView("apps")).catch((err: Error) => notify(err.message))}>Remove custom entry</Button>
            ) : (
              <Button onClick={() => void api.appsHide(app.id, true).then(() => setView("apps")).catch((err: Error) => notify(err.message))}>Hide from PulseLauncher</Button>
            )}
            <p className="mt-2 text-xs text-muted">Hiding an operating-system app only removes it from PulseLauncher. It does not uninstall software.</p>
          </div>
        </div>
      ) : null}
      {tab === "preview" ? (
        <div className="space-y-3">
          {image ? (
            <img src={image} alt="User imported preview" className="max-h-80 rounded-card border border-line object-contain" />
          ) : (
            <div className="rounded-card border border-dashed border-line bg-panel px-6 py-12 text-center">
              <div className="mx-auto mb-3 flex h-16 w-16 items-center justify-center rounded-xl border border-line text-xl">{app.displayName.slice(0, 1)}</div>
              <p className="font-semibold">No preview available</p>
              <p className="mt-1 text-sm text-muted">PulseLauncher does not download product screenshots. Import an image you already have, or leave this empty.</p>
            </div>
          )}
          <div className="flex flex-wrap gap-2">
            <Button onClick={() => void pickPath(false, [{ name: "Images", extensions: ["png", "jpg", "jpeg", "webp", "gif"] }]).then(async (path) => {
              if (!path) return;
              await api.previewImport(app.id, path);
              await load(app.id);
            }).catch((err: Error) => notify(err.message))}>Import screenshot</Button>
            <Button disabled title="Live capture is not available. Windows does not provide a safe thumbnail for arbitrary windows, and this build will not grab the screen automatically.">Capture window</Button>
            {previews[0] ? (
              <Button variant="danger" onClick={() => void api.previewDelete(previews[0].id).then(() => load(app.id)).catch((err: Error) => notify(err.message))}>Remove preview</Button>
            ) : null}
          </div>
          <p className="text-xs text-muted">Capture stays disabled. Import copies a PNG, JPG, WEBP, or GIF into app data. You can delete it later.</p>
        </div>
      ) : null}
      {tab === "activity" ? (
        events.length === 0 ? (
          <EmptyState title="No launches recorded" body="Accepted and rejected launch attempts for this app appear here after you run it." />
        ) : (
          <ul className="space-y-2">
            {events.map((event) => (
              <li key={event.id} className="rounded-card border border-line bg-panel px-3 py-2 text-sm">
                <div className="flex items-center justify-between gap-2">
                  <StatusBadge status={event.outcome} />
                  <span className="text-xs text-muted">{event.createdAt}</span>
                </div>
                <p className="mt-1 text-muted">{event.message}</p>
              </li>
            ))}
          </ul>
        )
      ) : null}
      {editing ? (
        <EditDialog
          app={app}
          onClose={() => setEditing(false)}
          onSaved={async () => {
            setEditing(false);
            await load(app.id);
          }}
        />
      ) : null}
    </section>
  );
}

function Info({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-card border border-line bg-panel px-3 py-2">
      <div className="text-xs text-muted">{label}</div>
      <div className="break-all text-sm">{value}</div>
    </div>
  );
}

function EditDialog({ app, onClose, onSaved }: { app: AppEntry; onClose: () => void; onSaved: () => Promise<void> }) {
  const [displayName, setDisplayName] = useState(app.displayName);
  const [target, setTarget] = useState(app.launchTarget);
  const [argumentsText, setArgumentsText] = useState(app.arguments.join("\n"));
  const [workingDirectory, setWorkingDirectory] = useState(app.workingDirectory ?? "");
  const [tags, setTags] = useState(app.tags.join(", "));
  const [category, setCategory] = useState(app.category);
  const [notes, setNotes] = useState(app.notes ?? "");
  const [error, setError] = useState<string | null>(null);

  async function save() {
    try {
      await api.appsUpdate({
        id: app.id,
        displayName,
        target,
        arguments: argumentLines(argumentsText),
        workingDirectory: workingDirectory || null,
        tags: tags.split(",").map((tag) => tag.trim()).filter(Boolean),
        category,
        notes: notes || null,
      });
      await onSaved();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Update failed.");
    }
  }

  return (
    <Dialog title="Edit registration" onClose={onClose}>
      <div className="space-y-3">
        {error ? <Banner tone="danger">{error}</Banner> : null}
        <Field label="Display name"><input className={controlClass} value={displayName} onChange={(event) => setDisplayName(event.target.value)} /></Field>
        <Field label="Target" hint="The file must exist. A missing path cannot be saved.">
          <div className="flex gap-2">
            <input className={controlClass} value={target} onChange={(event) => setTarget(event.target.value)} />
            <Button onClick={() => void pickPath(false).then((path) => path && setTarget(path))}>Browse</Button>
          </div>
        </Field>
        <Field label="Arguments"><textarea className={`${controlClass} min-h-20`} value={argumentsText} onChange={(event) => setArgumentsText(event.target.value)} /></Field>
        <Field label="Working directory"><input className={controlClass} value={workingDirectory} onChange={(event) => setWorkingDirectory(event.target.value)} /></Field>
        <Field label="Category"><input className={controlClass} value={category} onChange={(event) => setCategory(event.target.value)} /></Field>
        <Field label="Tags"><input className={controlClass} value={tags} onChange={(event) => setTags(event.target.value)} /></Field>
        <Field label="Notes"><textarea className={controlClass} value={notes} onChange={(event) => setNotes(event.target.value)} /></Field>
        <div className="flex justify-end gap-2">
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="primary" onClick={() => void save()}>Save</Button>
        </div>
      </div>
    </Dialog>
  );
}
