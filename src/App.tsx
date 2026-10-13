import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { AppsView } from "./features/apps/AppsView";
import { DetailsView } from "./features/details/DetailsView";
import { Gallery } from "./features/gallery/Gallery";
import { Launcher } from "./features/launcher/Launcher";
import { SettingsView } from "./features/settings/SettingsView";
import { WorkflowsView } from "./features/workflows/WorkflowsView";
import * as api from "./lib/api";
import { applyPrefs, useUi, type Page } from "./lib/store";

const desktop = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export function App() {
  const view = new URLSearchParams(window.location.search).get("view");
  if (view === "launcher") return <Launcher />;
  if (view === "gallery") return <Gallery />;
  return <Dashboard />;
}

function Dashboard() {
  const page = useUi((s) => s.page);
  const setPage = useUi((s) => s.setPage);
  const openApp = useUi((s) => s.openApp);
  const selected = useUi((s) => s.selectedAppId);
  const notice = useUi((s) => s.notice);
  const setNotice = useUi((s) => s.setNotice);
  const prefs = useUi((s) => s.prefs);
  const setPrefs = useUi((s) => s.setPrefs);
  const [confirm, setConfirm] = useState<string | null>(null);
  const [onboarding, setOnboarding] = useState(false);

  useEffect(() => {
    if (!desktop) return;
    void api.getPreferences().then((p) => {
      setPrefs(p);
      applyPrefs(p);
      setOnboarding(!p.onboardingDone);
    }).catch((e) => setNotice(api.errorText(e)));
    const unlisten: Promise<() => void>[] = [];
    unlisten.push(listen<string>("open-app", (event) => openApp(event.payload)));
    unlisten.push(listen<{ title: string; body: string }>("app-notice", (event) => setNotice(`${event.payload.title}: ${event.payload.body}`)));
    unlisten.push(listen<string>("workflow-confirm", (event) => setConfirm(event.payload)));
    unlisten.push(listen("scan-finished", () => setNotice("Scan finished.")));
    return () => {
      void Promise.all(unlisten).then((stops) => stops.forEach((stop) => stop()));
    };
  }, []);

  const items: { id: Page; label: string }[] = [
    { id: "apps", label: "Apps & Tools" },
    { id: "commands", label: "Commands" },
    { id: "settings", label: "Settings" },
    { id: "gallery", label: "Gallery" },
  ];

  return (
    <div className="app-shell">
      <nav className="nav" aria-label="Primary">
        <div className="brand">
          <span className="mark" aria-hidden="true"><i /><i /><i /></span>
          <div>
            <strong>PulseLauncher</strong>
            <span>ZeroPulse</span>
          </div>
        </div>
        {items.map((item) => (
          <button key={item.id} className="item" aria-current={page === item.id ? "page" : undefined} onClick={() => setPage(item.id)}>
            <span className="label">{item.label}</span>
          </button>
        ))}
        <div style={{ marginTop: "auto" }} className="tiny">Offline · no account</div>
        <button className="btn-ghost" onClick={() => void api.quitApp()}>Quit</button>
      </nav>
      <main className="main">
        {notice && (
          <p className="notice" role="status">
            {notice} <button className="btn-ghost" onClick={() => setNotice(null)}>Dismiss</button>
          </p>
        )}
        {page === "apps" && <AppsView />}
        {page === "commands" && <WorkflowsView />}
        {page === "settings" && <SettingsView />}
        {page === "details" && <DetailsView appId={selected} />}
        {page === "gallery" && <Gallery />}
      </main>
      {confirm && (
        <div className="overlay" role="presentation">
          <div className="dialog" role="alertdialog" aria-label="Confirm workflow step">
            <h2>Confirm</h2>
            <pre className="log">{confirm}</pre>
            <div className="row">
              <button className="btn" onClick={() => { void api.confirmWorkflow(true); setConfirm(null); }}>Continue</button>
              <button className="btn-ghost" onClick={() => { void api.confirmWorkflow(false); setConfirm(null); }}>Cancel</button>
            </div>
          </div>
        </div>
      )}
      {onboarding && prefs && (
        <div className="overlay">
          <div className="dialog" role="dialog" aria-labelledby="welcome-title">
            <h2 id="welcome-title">Welcome to PulseLauncher</h2>
            <p>This is a local launcher. It does not sign you in, phone home, or invent applications.</p>
            <ul>
              <li>Alt+Space opens search. If that key is busy, Ctrl+Alt+Space is offered.</li>
              <li>The first scan reads Start Menu shortcuts only. It does not launch anything it finds.</li>
              <li>Portable tools are added with Add App, including other ZeroPulse programs you actually have.</li>
              <li>Commands are templates until you configure them. “Needs setup” means a prerequisite is missing.</li>
            </ul>
            <button
              className="btn"
              onClick={() =>
                void api.completeOnboarding().then(() => {
                  setOnboarding(false);
                  setPrefs({ ...prefs, onboardingDone: true });
                })
              }
            >
              Continue
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
