import { listen } from "@tauri-apps/api/event";
import { AppWindow, Keyboard, LayoutGrid, Palette, Settings, Workflow } from "lucide-react";
import { useEffect } from "react";
import { Button } from "./components/ui";
import { api, inTauri } from "./lib/api";
import type { ScanReport, WorkflowRun } from "./lib/types";
import { Launcher } from "./views/Launcher";
import { AppsView } from "./views/Apps";
import { CommandsView } from "./views/Commands";
import { DetailsView } from "./views/Details";
import { GalleryView } from "./views/Gallery";
import { SettingsView } from "./views/Settings";
import { applyPreferences, useShell, type ShellView } from "./store";

const nav: { id: ShellView; label: string; icon: typeof LayoutGrid }[] = [
  { id: "apps", label: "Apps & Tools", icon: LayoutGrid },
  { id: "commands", label: "Commands", icon: Workflow },
  { id: "settings", label: "Settings", icon: Settings },
  { id: "gallery", label: "Design gallery", icon: Palette },
];

function isLauncherWindow(): boolean {
  return new URLSearchParams(window.location.search).get("view") === "launcher";
}

export function App() {
  if (isLauncherWindow()) return <Launcher />;
  return <Dashboard />;
}

function Dashboard() {
  const view = useShell((state) => state.view);
  const setView = useShell((state) => state.setView);
  const openApp = useShell((state) => state.openApp);
  const toast = useShell((state) => state.toast);
  const notify = useShell((state) => state.notify);
  const confirm = useShell((state) => state.confirm);
  const setConfirm = useShell((state) => state.setConfirm);
  const setPreferences = useShell((state) => state.setPreferences);
  const setLiveRun = useShell((state) => state.setLiveRun);

  useEffect(() => {
    if (!inTauri()) return;
    void api.preferencesGet().then((preferences) => {
      setPreferences(preferences);
      applyPreferences(preferences);
    }).catch((error: Error) => notify(error.message));
    let disposed = false;
    const unlistens: Array<() => void> = [];
    void listen<ScanReport>("scan-finished", () => notify("App scan finished.")).then((fn) => {
      if (disposed) fn();
      else unlistens.push(fn);
    });
    void listen<{ title: string; body: string }>("app-notification", (event) => {
      notify(`${event.payload.title}: ${event.payload.body}`);
    }).then((fn) => {
      if (disposed) fn();
      else unlistens.push(fn);
    });
    void listen<{ appId: string }>("open-details", (event) => openApp(event.payload.appId)).then((fn) => {
      if (disposed) fn();
      else unlistens.push(fn);
    });
    void listen<WorkflowRun>("workflow-progress", (event) => setLiveRun(event.payload)).then((fn) => {
      if (disposed) fn();
      else unlistens.push(fn);
    });
    void listen<{ runId: string; message: string }>("workflow-confirm", (event) => setConfirm(event.payload)).then((fn) => {
      if (disposed) fn();
      else unlistens.push(fn);
    });
    return () => {
      disposed = true;
      unlistens.forEach((fn) => fn());
    };
  }, [notify, openApp, setConfirm, setLiveRun, setPreferences]);

  useEffect(() => {
    if (!toast) return;
    const timer = window.setTimeout(() => notify(null), 5000);
    return () => window.clearTimeout(timer);
  }, [toast, notify]);

  return (
    <div className="shell bg-ink text-foreground">
      <aside className="flex gap-2 overflow-auto border-b border-line bg-elevated p-3 md:flex-col md:border-b-0 md:border-r">
        <div className="hidden px-2 py-3 md:block">
          <div className="text-xs font-semibold uppercase tracking-[0.16em] text-bright">ZeroPulse</div>
          <div className="text-lg font-semibold">PulseLauncher</div>
        </div>
        <Button className="justify-start" onClick={() => void api.showLauncher().catch((error: Error) => notify(error.message))}>
          <Keyboard size={16} aria-hidden /> Launcher
        </Button>
        {nav.map((item) => {
          const Icon = item.icon;
          const active = view === item.id || (item.id === "apps" && view === "details");
          return (
            <button
              key={item.id}
              className={`flex items-center gap-2 rounded-lg px-3 py-2 text-left text-sm ${active ? "bg-panel text-foreground" : "text-muted hover:text-foreground"}`}
              aria-current={active ? "page" : undefined}
              onClick={() => setView(item.id)}
            >
              <Icon size={16} aria-hidden />
              {item.label}
            </button>
          );
        })}
        <div className="mt-auto hidden px-2 text-xs text-muted md:block">
          <AppWindow size={14} className="mb-1" aria-hidden />
          Local database. No account.
        </div>
      </aside>
      <main className="min-h-0 min-w-0 overflow-auto p-[var(--space)]">
        {view === "apps" ? <AppsView /> : null}
        {view === "details" ? <DetailsView /> : null}
        {view === "commands" ? <CommandsView /> : null}
        {view === "settings" ? <SettingsView /> : null}
        {view === "gallery" ? <GalleryView /> : null}
      </main>
      {toast ? (
        <div role="status" className="fixed bottom-4 right-4 z-30 max-w-sm rounded-card border border-line bg-panel px-4 py-3 text-sm shadow-lift">
          {toast}
        </div>
      ) : null}
      {confirm ? (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/55 p-4">
          <div role="alertdialog" aria-modal="true" aria-label="Confirm workflow step" className="w-full max-w-md rounded-card border border-line bg-elevated p-5">
            <h2 className="text-lg font-semibold">Confirm this step</h2>
            <p className="mt-2 text-sm text-muted">{confirm.message}</p>
            <div className="mt-4 flex justify-end gap-2">
              <Button onClick={() => void api.workflowsConfirm(confirm.runId, false).finally(() => setConfirm(null))}>Cancel step</Button>
              <Button variant="primary" onClick={() => void api.workflowsConfirm(confirm.runId, true).finally(() => setConfirm(null))}>Continue</Button>
            </div>
          </div>
        </div>
      ) : null}
    </div>
  );
}
