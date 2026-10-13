import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, errorText } from "../../lib/api";
import { readinessLabel } from "../../lib/text";
import type { AppEntry, Facts, Step, WorkflowRun, WorkflowView } from "../../lib/types";
import { Field, Modal } from "../../components/ui";

export function WorkflowsView({ apps, onNotice }: { apps: AppEntry[]; onNotice: (t: string, b: string) => void }) {
  const [items, setItems] = useState<WorkflowView[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [facts, setFacts] = useState<Facts | null>(null);
  const [run, setRun] = useState<WorkflowRun | null>(null);
  const [confirm, setConfirm] = useState<{ workflowId: string; message: string } | null>(null);
  const [editing, setEditing] = useState(false);
  const [error, setError] = useState("");

  function load() {
    api.listWorkflows().then((list) => {
      setItems(list);
      setSelected((current) => current ?? list[0]?.id ?? null);
    }).catch((e) => setError(errorText(e)));
  }
  useEffect(load, []);
  useEffect(() => { api.environment().then(setFacts).catch(() => undefined); }, []);
  useEffect(() => {
    const unsubs = [
      listen<WorkflowRun>("workflow-progress", (e) => setRun(e.payload)).catch(() => () => undefined),
      listen<{ workflowId: string; message: string }>("workflow-confirm", (e) => setConfirm(e.payload)).catch(() => () => undefined),
    ];
    return () => { unsubs.forEach((p) => void p.then((u) => u())); };
  }, []);

  const current = items.find((item) => item.id === selected) ?? null;

  async function runCurrent() {
    if (!current) return;
    setRun(null);
    try {
      const result = await api.runWorkflow(current.id);
      setRun(result);
      onNotice(current.name, result.error ?? `Finished: ${result.status}`);
      load();
    } catch (e) {
      onNotice(current.name, errorText(e));
    }
  }

  return (
    <div className="grid h-full min-h-0 gap-4 lg:grid-cols-[280px_1fr]">
      <div className="scroll flex flex-col gap-2">
        <button className="btn" onClick={async () => {
          const name = window.prompt("Name for the new workflow");
          if (!name) return;
          const created = await api.createWorkflow(name);
          load();
          setSelected(created.id);
        }}>New workflow</button>
        {error ? <p className="text-[var(--color-danger)]">{error}</p> : null}
        {items.map((item) => (
          <button key={item.id} className={`card p-3 text-left ${item.id === selected ? "border-[var(--color-accent)]" : ""}`} onClick={() => { setSelected(item.id); setEditing(false); }}>
            <div className="font-medium">{item.name}</div>
            <span className={`pill mt-2 ${item.readiness.state === "ready" ? "pill-ok" : item.readiness.state === "unavailable" ? "pill-danger" : "pill-warn"}`}>
              {readinessLabel(item.readiness.state)}
            </span>
          </button>
        ))}
      </div>
      {current ? (
        <div className="scroll flex flex-col gap-4 pr-1">
          <header>
            <h2 className="text-2xl font-semibold">{current.name}</h2>
            <p className="mt-1 text-[var(--color-muted)]">{current.whatItDoes}</p>
          </header>
          <section className="card p-4">
            <h3 className="font-medium">Prerequisites</h3>
            <p className="text-sm text-[var(--color-muted)]">{current.prerequisites}</p>
            <ul className="mt-2 list-disc pl-5 text-sm">
              {current.readiness.details.map((line) => <li key={line}>{line}</li>)}
            </ul>
          </section>
          <Configure item={current} apps={apps} facts={facts} onSaved={(next) => { setItems((list) => list.map((w) => w.id === next.id ? next : w)); onNotice(next.name, "Configuration saved"); }} onNotice={onNotice} />
          <div className="flex flex-wrap gap-2">
            <button className="btn btn-primary" disabled={current.readiness.state !== "ready"} onClick={() => void runCurrent()}>Run</button>
            <button className="btn" onClick={() => setEditing((v) => !v)}>Edit</button>
            {current.templateKey ? <button className="btn" onClick={() => void api.resetTemplate(current.id).then((next) => { setItems((list) => list.map((w) => w.id === next.id ? next : w)); onNotice(next.name, "Reset to the built-in steps"); })}>Reset</button> : <button className="btn" onClick={() => void api.deleteWorkflow(current.id).then(load)}>Delete</button>}
            {current.steps.some((s) => s.type === "runCommand") ? <button className="btn" onClick={() => void api.trustWorkflow(current.id).then((next) => setItems((list) => list.map((w) => w.id === next.id ? next : w))).catch((e) => onNotice("Trust", errorText(e)))}>{current.trusted ? "Trusted" : "Trust command"}</button> : null}
          </div>
          {editing ? <StepEditor item={current} onSaved={(next) => { setItems((list) => list.map((w) => w.id === next.id ? next : w)); setEditing(false); }} onNotice={onNotice} /> : null}
          <section className="card p-4">
            <h3 className="font-medium">Last result</h3>
            <RunLog run={run ?? current.lastRun} />
            {run?.status === "running" ? <button className="btn mt-2" onClick={() => void api.cancelWorkflow(current.id)}>Stop</button> : null}
          </section>
        </div>
      ) : <p className="text-[var(--color-muted)]">No workflows stored.</p>}
      {confirm ? (
        <Modal title="Confirm this step" onClose={() => { void api.confirmWorkflow(confirm.workflowId, false); setConfirm(null); }}>
          <p>{confirm.message}</p>
          <div className="mt-4 flex justify-end gap-2">
            <button className="btn" onClick={() => { void api.confirmWorkflow(confirm.workflowId, false); setConfirm(null); }}>Decline</button>
            <button className="btn btn-primary" onClick={() => { void api.confirmWorkflow(confirm.workflowId, true); setConfirm(null); }}>Continue</button>
          </div>
        </Modal>
      ) : null}
    </div>
  );
}

function Configure({ item, apps, facts, onSaved, onNotice }: { item: WorkflowView; apps: AppEntry[]; facts: Facts | null; onSaved: (w: WorkflowView) => void; onNotice: (t: string, b: string) => void }) {
  const [values, setValues] = useState(item.values);
  useEffect(() => setValues(item.values), [item]);
  if (item.params.length === 0) return null;
  async function save() {
    try { onSaved(await api.configureWorkflow(item.id, values)); }
    catch (e) { onNotice(item.name, errorText(e)); }
  }
  return (
    <section className="card flex flex-col gap-3 p-4">
      <h3 className="font-medium">Configure</h3>
      {item.params.map((param) => (
        <Field key={param.key} label={param.label} hint={param.help}>
          {param.kind === "choice" || param.key === "terminal" ? (
            <select className="field" value={values[param.key] ?? ""} onChange={(e) => setValues({ ...values, [param.key]: e.target.value })}>
              <option value="">Choose</option>
              {(param.key === "terminal" ? (facts?.terminals ?? []).map((t) => ({ value: t.path, label: `${t.label} — ${t.path}` })) : param.choices).map((c) => <option key={c.value} value={c.value}>{c.label}</option>)}
            </select>
          ) : param.kind === "app" ? (
            <select className="field" value={values[param.key] ?? ""} onChange={(e) => setValues({ ...values, [param.key]: e.target.value })}>
              <option value="">None</option>
              {apps.map((app) => <option key={app.id} value={app.id}>{app.name}</option>)}
            </select>
          ) : param.kind === "apps" ? (
            <select multiple className="field min-h-24" value={parseIds(values[param.key])} onChange={(e) => {
              const ids = Array.from(e.target.selectedOptions).map((o) => o.value);
              setValues({ ...values, [param.key]: JSON.stringify(ids) });
            }}>
              {apps.map((app) => <option key={app.id} value={app.id}>{app.name}</option>)}
            </select>
          ) : (
            <input className="field" value={values[param.key] ?? ""} onChange={(e) => setValues({ ...values, [param.key]: e.target.value })} />
          )}
        </Field>
      ))}
      <button className="btn w-fit" onClick={() => void save()}>Save configuration</button>
    </section>
  );
}

function parseIds(raw: string | undefined): string[] {
  if (!raw) return [];
  try { const v = JSON.parse(raw) as unknown; return Array.isArray(v) ? v.filter((x) => typeof x === "string") : []; }
  catch { return []; }
}

function StepEditor({ item, onSaved, onNotice }: { item: WorkflowView; onSaved: (w: WorkflowView) => void; onNotice: (t: string, b: string) => void }) {
  const [steps, setSteps] = useState<Step[]>(item.steps);
  const [name, setName] = useState(item.name);
  function move(i: number, dir: -1 | 1) {
    const j = i + dir;
    if (j < 0 || j >= steps.length) return;
    const next = steps.slice();
    const a = next[i]!;
    next[i] = next[j]!;
    next[j] = a;
    setSteps(next);
  }
  async function save() {
    try {
      onSaved(await api.saveWorkflowSteps(item.id, { name, description: item.description, params: item.params, steps }));
    } catch (e) { onNotice("Steps", errorText(e)); }
  }
  return (
    <section className="card p-4">
      <h3 className="mb-2 font-medium">Steps</h3>
      <Field label="Name"><input className="field" value={name} onChange={(e) => setName(e.target.value)} /></Field>
      <ol className="mt-3 flex flex-col gap-2">
        {steps.map((step, i) => (
          <li key={`${step.type}-${i}`} className="flex items-center gap-2 rounded-lg border border-[var(--color-line)] px-3 py-2">
            <span className="flex-1">{i + 1}. {step.type} {step.path || step.appId || step.url || step.uri || step.text || ""}</span>
            <button className="btn" onClick={() => move(i, -1)} aria-label="Move step up">Up</button>
            <button className="btn" onClick={() => move(i, 1)} aria-label="Move step down">Down</button>
            <button className="btn" onClick={() => setSteps(steps.filter((_, n) => n !== i))}>Remove</button>
          </li>
        ))}
      </ol>
      <div className="mt-3 flex flex-wrap gap-2">
        <button className="btn" onClick={() => setSteps([...steps, { type: "notify", title: "Done", body: "Step finished", onError: "stop" }])}>Add notification</button>
        <button className="btn" onClick={() => setSteps([...steps, { type: "wait", millis: 500, onError: "stop" }])}>Add wait</button>
        <button className="btn" onClick={() => setSteps([...steps, { type: "openFolder", path: "", onError: "stop" }])}>Add open folder</button>
        <button className="btn btn-primary" onClick={() => void save()}>Save steps</button>
      </div>
      <p className="mt-2 text-xs text-[var(--color-muted)]">A developer command step only runs after Settings enables it and you trust this workflow. PulseLauncher never turns search text into a shell command.</p>
    </section>
  );
}

export function RunLog({ run }: { run: WorkflowRun | null }) {
  if (!run) return <p className="text-sm text-[var(--color-muted)]">This has not been run in this installation.</p>;
  return (
    <div className="mt-2">
      <p className="text-sm">Status: <strong>{run.status}</strong> {run.error ? `— ${run.error}` : ""}</p>
      <ol className="mt-2 flex flex-col gap-1 text-sm">
        {run.steps.map((step) => (
          <li key={step.index}>
            <span className={step.status === "succeeded" ? "text-[var(--color-ok)]" : step.status === "failed" ? "text-[var(--color-danger)]" : "text-[var(--color-warn)]"}>{step.status}</span>
            {" "}{step.index}. {step.action}: {step.message}
            <div className="text-xs text-[var(--color-faint)]">{step.detail}</div>
          </li>
        ))}
      </ol>
    </div>
  );
}
