import { Banner, Button, Field, controlClass } from "../components/ui";
import type { AppEntry, Collection, ErrorPolicy, Step, WorkflowDocument } from "../lib/types";
import { newId } from "../lib/text";

const POLICIES: ErrorPolicy[] = ["stop", "skip"];

export function StepEditor({
  document,
  apps,
  collections,
  developerCommands,
  onChange,
}: {
  document: WorkflowDocument;
  apps: AppEntry[];
  collections: Collection[];
  developerCommands: boolean;
  onChange: (document: WorkflowDocument) => void;
}) {
  function updateStep(index: number, step: Step) {
    const steps = document.steps.map((item, itemIndex) => (itemIndex === index ? step : item));
    onChange({ ...document, steps });
  }

  function move(index: number, delta: number) {
    const next = index + delta;
    if (next < 0 || next >= document.steps.length) return;
    const steps = [...document.steps];
    const [step] = steps.splice(index, 1);
    steps.splice(next, 0, step);
    onChange({ ...document, steps });
  }

  function add(type: Step["type"]) {
    if (document.steps.length >= 40) return;
    onChange({ ...document, steps: [...document.steps, blankStep(type)] });
  }

  return (
    <div className="space-y-3">
      <ol className="space-y-3">
        {document.steps.map((step, index) => (
          <li key={step.id} className="rounded-card border border-line bg-ink p-3">
            <div className="mb-2 flex flex-wrap items-center justify-between gap-2">
              <span className="text-sm font-semibold">{index + 1}. {stepLabel(step.type)}</span>
              <div className="flex gap-1">
                <Button onClick={() => move(index, -1)} aria-label={`Move step ${index + 1} up`}>Up</Button>
                <Button onClick={() => move(index, 1)} aria-label={`Move step ${index + 1} down`}>Down</Button>
                <Button variant="danger" onClick={() => onChange({ ...document, steps: document.steps.filter((_, item) => item !== index) })}>Remove</Button>
              </div>
            </div>
            <StepFields step={step} apps={apps} collections={collections} onChange={(next) => updateStep(index, next)} />
          </li>
        ))}
      </ol>
      <div className="flex flex-wrap gap-2">
        <Add type="openFolder" onAdd={add} />
        <Add type="openKnownFolder" onAdd={add} />
        <Add type="openUrl" onAdd={add} />
        <Add type="copyText" onAdd={add} />
        <Add type="delay" onAdd={add} />
        <Add type="notify" onAdd={add} />
        <Add type="createNote" onAdd={add} />
        <Add type="launchApp" onAdd={add} />
        <Add type="checkExists" onAdd={add} />
        <Add type="confirm" onAdd={add} />
        <Add type="launchTrustedTool" onAdd={add} />
        <Add type="launchCollection" onAdd={add} />
        <Button disabled={!developerCommands} onClick={() => add("runCommand")}>Add developer command</Button>
      </div>
      {!developerCommands ? (
        <Banner>Developer command steps stay unavailable until you enable them in Settings. They are off by default.</Banner>
      ) : (
        <Banner tone="warn">A developer command runs only the program and argument array you save. Review it before you run the workflow.</Banner>
      )}
    </div>
  );
}

function Add({ type, onAdd }: { type: Step["type"]; onAdd: (type: Step["type"]) => void }) {
  return <Button onClick={() => onAdd(type)}>Add {stepLabel(type)}</Button>;
}

function StepFields({
  step,
  apps,
  collections,
  onChange,
}: {
  step: Step;
  apps: AppEntry[];
  collections: Collection[];
  onChange: (step: Step) => void;
}) {
  if (step.type === "launchApp") {
    return (
      <div className="grid gap-2 md:grid-cols-2">
        <Field label="App">
          <select className={controlClass} value={step.appId} onChange={(event) => onChange({ ...step, appId: event.target.value })}>
            <option value="">Select an app</option>
            {apps.map((app) => <option key={app.id} value={app.id}>{app.displayName}</option>)}
          </select>
        </Field>
        <Policy value={step.onError} onChange={(onError) => onChange({ ...step, onError })} />
      </div>
    );
  }
  if (step.type === "launchTrustedTool") {
    return (
      <div className="grid gap-2 md:grid-cols-2">
        <Field label="Tool" hint="Only Task Manager or a detected terminal. This is not an arbitrary path.">
          <select className={controlClass} value={step.tool} onChange={(event) => onChange({ ...step, tool: event.target.value as "taskManager" | "terminal" })}>
            <option value="taskManager">Task Manager</option>
            <option value="terminal">Terminal</option>
          </select>
        </Field>
        <Policy value={step.onError} onChange={(onError) => onChange({ ...step, onError })} />
      </div>
    );
  }
  if (step.type === "openFolder") {
    return (
      <div className="grid gap-2">
        <Field label="Folder"><input className={controlClass} value={step.path} onChange={(event) => onChange({ ...step, path: event.target.value })} /></Field>
        <label className="flex items-center gap-2 text-sm"><input type="checkbox" checked={step.skipIfBlank} onChange={(event) => onChange({ ...step, skipIfBlank: event.target.checked })} /> Skip when the path is blank</label>
        <Policy value={step.onError} onChange={(onError) => onChange({ ...step, onError })} />
      </div>
    );
  }
  if (step.type === "openKnownFolder") {
    return (
      <Field label="Known folder">
        <select className={controlClass} value={step.folder} onChange={(event) => onChange({ ...step, folder: event.target.value as "downloads" | "documents" })}>
          <option value="downloads">Downloads</option>
          <option value="documents">Documents</option>
        </select>
      </Field>
    );
  }
  if (step.type === "openUrl") {
    return <Field label="URL" hint="http, https, or ms-settings. Other schemes are rejected."><input className={controlClass} value={step.url} onChange={(event) => onChange({ ...step, url: event.target.value })} /></Field>;
  }
  if (step.type === "copyText") {
    return <Field label="Text"><input className={controlClass} value={step.text} onChange={(event) => onChange({ ...step, text: event.target.value })} /></Field>;
  }
  if (step.type === "delay") {
    return <Field label="Delay milliseconds" hint="Maximum 60 seconds."><input className={controlClass} type="number" min={0} max={60000} value={step.ms} onChange={(event) => onChange({ ...step, ms: Number(event.target.value) })} /></Field>;
  }
  if (step.type === "notify") {
    return (
      <div className="grid gap-2">
        <Field label="Title"><input className={controlClass} value={step.title} onChange={(event) => onChange({ ...step, title: event.target.value })} /></Field>
        <Field label="Body"><input className={controlClass} value={step.body} onChange={(event) => onChange({ ...step, body: event.target.value })} /></Field>
      </div>
    );
  }
  if (step.type === "createNote") {
    return (
      <div className="grid gap-2">
        <Field label="Directory"><input className={controlClass} value={step.directory} onChange={(event) => onChange({ ...step, directory: event.target.value })} /></Field>
        <Field label="Body"><textarea className={controlClass} value={step.body} onChange={(event) => onChange({ ...step, body: event.target.value })} /></Field>
      </div>
    );
  }
  if (step.type === "checkExists" || step.type === "confirm") {
    return (
      <div className="grid gap-2 md:grid-cols-2">
        {step.type === "checkExists" ? (
          <Field label="Path"><input className={controlClass} value={step.path} onChange={(event) => onChange({ ...step, path: event.target.value })} /></Field>
        ) : (
          <Field label="Message"><input className={controlClass} value={step.message} onChange={(event) => onChange({ ...step, message: event.target.value })} /></Field>
        )}
        <Field label="Then step id"><input className={controlClass} value={step.thenId} onChange={(event) => onChange({ ...step, thenId: event.target.value })} /></Field>
        <Field label="Else step id"><input className={controlClass} value={step.elseId} onChange={(event) => onChange({ ...step, elseId: event.target.value })} /></Field>
      </div>
    );
  }
  if (step.type === "runCommand") {
    return (
      <div className="grid gap-2">
        <Field label="Program"><input className={controlClass} value={step.program} onChange={(event) => onChange({ ...step, program: event.target.value })} /></Field>
        <Field label="Arguments" hint="One per line.">
          <textarea className={controlClass} value={step.args.join("\n")} onChange={(event) => onChange({ ...step, args: event.target.value.split(/\r?\n/).filter(Boolean) })} />
        </Field>
      </div>
    );
  }
  return (
    <Field label="Collection">
      <select className={controlClass} value={step.collectionId} onChange={(event) => onChange({ ...step, collectionId: event.target.value })}>
        {collections.map((collection) => <option key={collection.id} value={collection.id}>{collection.name}</option>)}
      </select>
    </Field>
  );
}

function Policy({ value, onChange }: { value: ErrorPolicy; onChange: (value: ErrorPolicy) => void }) {
  return (
    <Field label="If this step fails">
      <select className={controlClass} value={value} onChange={(event) => onChange(event.target.value as ErrorPolicy)}>
        {POLICIES.map((policy) => <option key={policy} value={policy}>{policy === "stop" ? "Stop the workflow" : "Skip and continue"}</option>)}
      </select>
    </Field>
  );
}

function blankStep(type: Step["type"]): Step {
  const id = newId("step");
  switch (type) {
    case "launchApp":
      return { type, id, appId: "", args: [], onError: "stop" };
    case "launchTrustedTool":
      return { type, id, tool: "terminal", onError: "stop" };
    case "openFolder":
      return { type, id, path: "", skipIfBlank: false, onError: "stop" };
    case "openKnownFolder":
      return { type, id, folder: "downloads", onError: "stop" };
    case "openUrl":
      return { type, id, url: "https://", onError: "stop" };
    case "copyText":
      return { type, id, text: "", onError: "stop" };
    case "delay":
      return { type, id, ms: 500, onError: "stop" };
    case "notify":
      return { type, id, title: "PulseLauncher", body: "", onError: "stop" };
    case "createNote":
      return { type, id, directory: "", body: "", onError: "stop" };
    case "checkExists":
      return { type, id, path: "", thenId: "", elseId: "" };
    case "confirm":
      return { type, id, message: "Continue?", thenId: "", elseId: "" };
    case "runCommand":
      return { type, id, program: "", args: [], workingDirectory: null, timeoutMs: null, onError: "stop" };
    case "launchCollection":
      return { type, id, collectionId: "col-zeropulse", skipMissing: true, onError: "stop" };
  }
}

function stepLabel(type: Step["type"]): string {
  switch (type) {
    case "launchApp":
      return "Launch app";
    case "launchTrustedTool":
      return "Trusted tool";
    case "openFolder":
      return "Open folder";
    case "openKnownFolder":
      return "Known folder";
    case "openUrl":
      return "Open URL";
    case "copyText":
      return "Copy text";
    case "delay":
      return "Delay";
    case "notify":
      return "Notification";
    case "createNote":
      return "Create note";
    case "checkExists":
      return "Check path";
    case "confirm":
      return "Confirm";
    case "runCommand":
      return "Developer command";
    case "launchCollection":
      return "Launch collection";
  }
}
