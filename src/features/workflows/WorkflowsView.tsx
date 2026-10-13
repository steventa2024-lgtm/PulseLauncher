import { useEffect, useState } from "react";
import * as api from "../../lib/api";
import type { WorkflowCard, WorkflowDefinition, WorkflowRun, WorkflowStep } from "../../lib/types";
import { useUi } from "../../lib/store";
import { Chip, Empty } from "../../components/Bits";

export function WorkflowsView({ initial }: { initial?: WorkflowCard[] }) {
  const setNotice = useUi((s) => s.setNotice);
  const [cards, setCards] = useState<WorkflowCard[]>(initial ?? []);
  const [selected, setSelected] = useState<string | null>(initial?.[0]?.definition.id ?? null);
  const [error, setError] = useState<string | null>(null);
  const [running, setRunning] = useState(false);
  const [last, setLast] = useState<WorkflowRun | null>(null);

  async function reload() {
    try {
      const rows = await api.listWorkflows();
      setCards(rows);
      setError(null);
      if (!selected && rows[0]) setSelected(rows[0].definition.id);
    } catch (e) {
      setError(api.errorText(e));
    }
  }
  useEffect(() => {
    if (!initial) void reload();
  }, []);

  const card = cards.find((c) => c.definition.id === selected) ?? null;

  async function run(def: WorkflowDefinition) {
    setRunning(true);
    setLast(null);
    try {
      const result = await api.runWorkflow(def.id, def.values ?? {});
      setLast(result);
      setNotice(result.status === "succeeded" ? `${def.name} finished.` : `${def.name} ${result.status}.`);
      await reload();
    } catch (e) {
      setError(api.errorText(e));
    } finally {
      setRunning(false);
    }
  }

  return (
    <div>
      <div className="topbar">
        <h1>Commands & Workflows</h1>
      </div>
      <div className="content split">
        <div>
          {error && <p role="alert">{error}</p>}
          {cards.length === 0 && !error && (
            <Empty title="No commands yet" body="Built-in templates appear after the desktop app creates its database. Nothing here is simulated." />
          )}
          <div className="list">
            {cards.map((item) => (
              <button key={item.definition.id} className="card" onClick={() => { setSelected(item.definition.id); setLast(item.status.lastRun ?? null); }}>
                <div className="row">
                  <strong>{item.definition.name}</strong>
                  <Chip kind={item.status.state === "ready" ? "ok" : "warn"}>{item.status.state === "ready" ? "Ready" : "Needs setup"}</Chip>
                </div>
                <div className="tiny">{item.definition.kind === "command" ? "Command" : "Workflow"} · {item.definition.description}</div>
              </button>
            ))}
          </div>
        </div>
        {card && (
          <Editor
            card={card}
            running={running}
            last={last ?? card.status.lastRun ?? null}
            onChange={(next) => setCards((rows) => rows.map((row) => (row.definition.id === next.id ? { ...row, definition: next } : row)))}
            onRun={() => void run(card.definition)}
            onStop={() => void api.cancelWorkflow().catch((e) => setError(api.errorText(e)))}
            onSave={async () => {
              try {
                await api.saveWorkflow(card.definition);
                setNotice("Saved.");
                await reload();
              } catch (e) {
                setError(api.errorText(e));
              }
            }}
            onReset={async () => {
              if (!card.definition.templateId) return;
              const fresh = await api.resetTemplate(card.definition.templateId);
              setNotice(`Reset ${fresh.name}.`);
              await reload();
            }}
          />
        )}
      </div>
    </div>
  );
}

function Editor({
  card,
  running,
  last,
  onChange,
  onRun,
  onStop,
  onSave,
  onReset,
}: {
  card: WorkflowCard;
  running: boolean;
  last: WorkflowRun | null;
  onChange: (def: WorkflowDefinition) => void;
  onRun: () => void;
  onStop: () => void;
  onSave: () => void;
  onReset: () => void;
}) {
  const def = card.definition;
  function setValue(name: string, value: string) {
    onChange({ ...def, values: { ...def.values, [name]: value } });
  }
  function updateStep(index: number, step: WorkflowStep) {
    const steps = def.steps.slice();
    steps[index] = step;
    onChange({ ...def, steps });
  }
  function move(index: number, dir: -1 | 1) {
    const next = index + dir;
    if (next < 0 || next >= def.steps.length) return;
    const steps = def.steps.slice();
    const [row] = steps.splice(index, 1);
    steps.splice(next, 0, row);
    onChange({ ...def, steps });
  }
  return (
    <section className="card" aria-label="Workflow editor">
      <h2>{def.name}</h2>
      <p>{def.description}</p>
      <Chip kind={card.status.state === "ready" ? "ok" : "warn"}>{card.status.state === "ready" ? "Ready" : "Needs setup"}</Chip>
      <h3>What it needs</h3>
      <ul>
        {card.status.prerequisites.length === 0 && <li>No extra prerequisite.</li>}
        {card.status.prerequisites.map((p) => (
          <li key={p.label}>
            <Chip kind={p.ok ? "ok" : "warn"}>{p.ok ? "Met" : "Needs setup"}</Chip> {p.label}: {p.detail}
          </li>
        ))}
      </ul>
      <h3>Configure</h3>
      {def.params.length === 0 && <p className="tiny">This template has no settings.</p>}
      {def.params.map((param) => (
        <label key={param.name} style={{ display: "block", marginBottom: 8 }}>
          <span className="tiny">{param.label}{param.required ? " (required)" : ""}</span>
          {param.kind === "choice" ? (
            <select className="field" aria-label={param.label} value={def.values?.[param.name] ?? param.default ?? ""} onChange={(e) => setValue(param.name, e.target.value)}>
              {param.choices.map((c) => (
                <option key={c.value} value={c.value}>{c.label}</option>
              ))}
            </select>
          ) : param.kind === "bool" ? (
            <select className="field" aria-label={param.label} value={def.values?.[param.name] ?? "false"} onChange={(e) => setValue(param.name, e.target.value)}>
              <option value="true">Yes</option>
              <option value="false">No</option>
            </select>
          ) : (
            <input className="field" aria-label={param.label} value={def.values?.[param.name] ?? ""} onChange={(e) => setValue(param.name, e.target.value)} />
          )}
          {param.help && <div className="tiny">{param.help}</div>}
        </label>
      ))}
      <h3>Steps</h3>
      <div className="steps">
        {def.steps.map((step, index) => (
          <div className="step" key={step.id}>
            <span className="tiny">{index + 1}</span>
            <div>
              <input className="field" aria-label={`Step ${index + 1} label`} value={step.label} onChange={(e) => updateStep(index, { ...step, label: e.target.value })} />
              <div className="tiny">{step.action.type}</div>
            </div>
            <div>
              <button className="btn-ghost" type="button" onClick={() => move(index, -1)} aria-label={`Move step ${index + 1} up`}>Up</button>
              <button className="btn-ghost" type="button" onClick={() => move(index, 1)} aria-label={`Move step ${index + 1} down`}>Down</button>
            </div>
          </div>
        ))}
      </div>
      <div className="row" style={{ marginTop: 12 }}>
        <button className="btn" onClick={onRun} disabled={running}>{running ? "Running…" : "Run"}</button>
        <button className="btn-ghost" onClick={onStop} disabled={!running}>Stop</button>
        <button className="btn-ghost" onClick={onSave}>Save</button>
        {def.templateId && <button className="btn-ghost" onClick={onReset}>Reset</button>}
      </div>
      <h3>Last result</h3>
      {!last && <p className="tiny">Not run in this session, and no saved run.</p>}
      {last && (
        <div>
          <Chip kind={last.status === "succeeded" ? "ok" : last.status === "cancelled" ? "warn" : "bad"}>{last.status}</Chip>
          <ul>
            {last.steps.map((step) => (
              <li key={step.stepId}>
                <strong>{step.label}</strong> ({step.status}) {step.message}
                {step.preview && <div className="tiny">Will run: {step.preview}</div>}
                {step.output && <pre className="log">{step.output}</pre>}
              </li>
            ))}
          </ul>
        </div>
      )}
    </section>
  );
}
