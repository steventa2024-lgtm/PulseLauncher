import { useEffect, useState } from "react";
import { Banner, Button, EmptyState, Field, StatusBadge, controlClass } from "../components/ui";
import { api, pickPath } from "../lib/api";
import type { AppEntry, Collection, StepPreview, WorkflowView } from "../lib/types";
import { useShell } from "../store";
import { StepEditor } from "./StepEditor";

export function CommandsView() {
  const notify = useShell((state) => state.notify);
  const liveRuns = useShell((state) => state.liveRuns);
  const preferences = useShell((state) => state.preferences);
  const [workflows, setWorkflows] = useState<WorkflowView[]>([]);
  const [apps, setApps] = useState<AppEntry[]>([]);
  const [collections, setCollections] = useState<Collection[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [draft, setDraft] = useState<WorkflowView | null>(null);
  const [preview, setPreview] = useState<StepPreview[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [editing, setEditing] = useState(false);

  async function reload() {
    try {
      const [next, nextApps, nextCollections] = await Promise.all([
        api.workflowsList(),
        api.appsList(false),
        api.collectionsList(),
      ]);
      setWorkflows(next);
      setApps(nextApps);
      setCollections(nextCollections);
      setSelectedId((current) => current ?? next[0]?.id ?? null);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not load workflows.");
    }
  }

  useEffect(() => {
    void reload();
  }, []);

  useEffect(() => {
    const current = workflows.find((item) => item.id === selectedId) ?? null;
    setDraft(current);
    setPreview(null);
    setEditing(false);
  }, [selectedId, workflows]);

  const live = draft ? liveRuns[draft.id] : undefined;
  const last = live ?? draft?.lastRun ?? null;

  async function saveValues(next: WorkflowView) {
    try {
      const saved = await api.workflowsSave(next);
      setWorkflows((items) => items.map((item) => (item.id === saved.id ? saved : item)));
      notify("Workflow saved.");
    } catch (err) {
      notify(err instanceof Error ? err.message : "Save failed.");
    }
  }

  if (error && workflows.length === 0) return <Banner tone="danger">{error}</Banner>;

  return (
    <section className="grid gap-4 lg:grid-cols-[280px_minmax(0,1fr)]">
      <div className="space-y-2">
        <h1 className="text-2xl font-semibold">Commands & Workflows</h1>
        <p className="text-sm text-muted">Twelve starter templates. A template stays on Needs setup until its real prerequisites are present.</p>
        <ul className="space-y-1">
          {workflows.map((workflow) => (
            <li key={workflow.id}>
              <button
                className={`w-full rounded-lg border px-3 py-2 text-left ${workflow.id === selectedId ? "border-bright bg-panel" : "border-line"}`}
                onClick={() => setSelectedId(workflow.id)}
              >
                <span className="block text-sm font-medium">{workflow.name}</span>
                <StatusBadge status={workflow.prerequisite.status} />
              </button>
            </li>
          ))}
        </ul>
      </div>
      {draft ? (
        <div className="space-y-4">
          <header>
            <h2 className="text-xl font-semibold">{draft.name}</h2>
            <p className="text-sm text-muted">{draft.description}</p>
          </header>
          <div className="grid gap-3 md:grid-cols-2">
            <article className="rounded-card border border-line bg-panel p-3">
              <h3 className="text-sm font-semibold">What it does</h3>
              <p className="mt-1 text-sm text-muted">{draft.document.whatItDoes}</p>
            </article>
            <article className="rounded-card border border-line bg-panel p-3">
              <h3 className="text-sm font-semibold">Prerequisites</h3>
              <div className="mt-1"><StatusBadge status={draft.prerequisite.status} /></div>
              <p className="mt-1 text-sm text-muted">{draft.prerequisite.message || draft.document.prerequisitesHelp}</p>
            </article>
          </div>
          <section className="space-y-3 rounded-card border border-line bg-panel p-3">
            <h3 className="font-semibold">Configure</h3>
            {draft.document.parameters.length === 0 ? <p className="text-sm text-muted">This template has no parameters.</p> : null}
            {draft.document.parameters.map((parameter) => (
              <Field key={parameter.key} label={parameter.label} hint={parameter.help}>
                {parameter.kind === "appId" ? (
                  <select className={controlClass} value={draft.document.values[parameter.key] ?? ""} onChange={(event) => setDraft(updateValue(draft, parameter.key, event.target.value))}>
                    <option value="">Select a registered app</option>
                    {apps.map((app) => <option key={app.id} value={app.id}>{app.displayName}</option>)}
                  </select>
                ) : parameter.kind === "enum" ? (
                  <select className={controlClass} value={draft.document.values[parameter.key] ?? parameter.options[0] ?? ""} onChange={(event) => setDraft(updateValue(draft, parameter.key, event.target.value))}>
                    {parameter.options.map((option) => <option key={option} value={option}>{option}</option>)}
                  </select>
                ) : parameter.kind === "text" ? (
                  <textarea className={controlClass} value={draft.document.values[parameter.key] ?? ""} onChange={(event) => setDraft(updateValue(draft, parameter.key, event.target.value))} />
                ) : (
                  <div className="flex gap-2">
                    <input className={controlClass} value={draft.document.values[parameter.key] ?? ""} onChange={(event) => setDraft(updateValue(draft, parameter.key, event.target.value))} />
                    {parameter.kind === "path" ? (
                      <Button onClick={() => void pickPath(parameter.key.toLowerCase().includes("terminal") ? false : true).then((path) => {
                        if (path) setDraft(updateValue(draft, parameter.key, path));
                      }).catch((err: Error) => notify(err.message))}>Browse</Button>
                    ) : null}
                  </div>
                )}
              </Field>
            ))}
            <Button onClick={() => void saveValues(draft)}>Save configuration</Button>
          </section>
          <section className="space-y-2">
            <div className="flex flex-wrap gap-2">
              <Button variant="primary" onClick={() => void api.workflowsStart(draft.id).then((run) => {
                useShell.getState().setLiveRun(run);
                notify(run.summary);
              }).catch((err: Error) => notify(err.message))}>Run</Button>
              <Button disabled={last?.status !== "running"} onClick={() => last && void api.workflowsCancel(last.id).catch((err: Error) => notify(err.message))}>Stop</Button>
              <Button onClick={() => void api.workflowsPreview(draft.id).then(setPreview).catch((err: Error) => notify(err.message))}>Preview</Button>
              <Button onClick={() => setEditing((value) => !value)}>{editing ? "Close editor" : "Edit steps"}</Button>
              <Button onClick={() => void api.workflowsReset(draft.id).then((saved) => {
                setWorkflows((items) => items.map((item) => (item.id === saved.id ? saved : item)));
                notify(saved.templateKey ? "Template reset." : "Only built-in templates can be reset.");
              }).catch((err: Error) => notify(err.message))}>Reset</Button>
            </div>
            <p className="text-xs text-muted">Run records the real engine result. Needs setup means the workflow did not launch anything.</p>
          </section>
          {preview ? (
            <section>
              <h3 className="mb-2 font-semibold">Preview</h3>
              <ul className="space-y-1 text-sm">
                {preview.map((step) => (
                  <li key={step.stepId} className="rounded border border-line px-3 py-2">
                    <span className="font-medium">{step.action}</span>
                    <span className="mt-1 block text-muted">{step.detail}</span>
                  </li>
                ))}
              </ul>
            </section>
          ) : null}
          <section className="rounded-card border border-line bg-panel p-3">
            <h3 className="font-semibold">Last result</h3>
            {last ? (
              <div className="mt-2 space-y-2 text-sm">
                <StatusBadge status={last.status} />
                <p>{last.summary}</p>
                <ul className="space-y-1 text-muted">
                  {last.steps.map((step) => (
                    <li key={`${step.stepId}-${step.startedAt}`}>{step.stepId}: {step.status} — {step.message}</li>
                  ))}
                </ul>
              </div>
            ) : <p className="mt-1 text-sm text-muted">This workflow has not been run in this database.</p>}
          </section>
          {editing ? (
            <section className="space-y-3">
              <h3 className="font-semibold">Step editor</h3>
              <p className="text-sm text-muted">Ordered steps only. PulseLauncher is not a node canvas; PulseFlow remains a separate product and is not called unless you configure a real executable or endpoint in Settings.</p>
              <StepEditor
                document={draft.document}
                apps={apps}
                collections={collections}
                developerCommands={preferences?.developerCommandsEnabled ?? false}
                onChange={(document) => setDraft({ ...draft, document })}
              />
              <Button variant="primary" onClick={() => void saveValues(draft)}>Save steps</Button>
            </section>
          ) : (
            <ol className="space-y-1 text-sm text-muted">
              {draft.document.steps.map((step, index) => (
                <li key={step.id}>{index + 1}. {step.type} · {step.id}</li>
              ))}
            </ol>
          )}
        </div>
      ) : <EmptyState title="No workflow selected" body="Templates are created in the local database on first launch." />}
    </section>
  );
}

function updateValue(workflow: WorkflowView, key: string, value: string): WorkflowView {
  return {
    ...workflow,
    document: {
      ...workflow.document,
      values: { ...workflow.document.values, [key]: value },
    },
  };
}
