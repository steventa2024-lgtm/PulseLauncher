import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { AppWindow, Search, Settings, Workflow } from "lucide-react";
import { api, errorText } from "./lib/api";
import { notify, useUi } from "./lib/store";
import type { ScanReport, UserPreferences } from "./lib/types";
import { AppsView } from "./features/apps/AppsView";
import { DetailsView } from "./features/details/DetailsView";
import { Launcher } from "./features/launcher/Launcher";
import { SettingsView } from "./features/settings/SettingsView";
import { WorkflowsView } from "./features/workflows/WorkflowsView";

type Page = "apps" | "workflows" | "settings";

const NAV: { id: Page; label: string; icon: typeof AppWindow }[] = [
  { id: "apps", label: "Apps & Tools", icon: AppWindow },
  { id: "workflows", label: "Commands & Workflows", icon: Workflow },
  { id: "settings", label: "Settings", icon: Settings },
];

function applyAppearance(prefs: UserPreferences) {
  const root = document.documentElement;
  root.dataset.theme = prefs.theme === "light" ? "light" : "dark";
  root.dataset.density = prefs.density === "compact" ? "compact" : "comfortable";
  root.dataset.motion = prefs.reduceMotion ? "reduce" : "full";
  root.dataset.contrast = prefs.highContrast ? "high" : "normal";
  root.style.setProperty("--text-scale", String(prefs.textScale));
  if (/^#[0-9a-fA-F]{6}$/.test(prefs.accent)) {
    root.style.setProperty("--color-accent", prefs.accent);
  }
}

export function App() {
  const launcher = new URLSearchParams(window.location.search).get("view") === "launcher";
  const prefs = useUi((s) => s.prefs);
  const apps = useUi((s) => s.apps);
  const collections = useUi((s) => s.collections);
  const icons = useUi((s) => s.icons);
  const notices = useUi((s) => s.notices);
  const info = useUi((s) => s.info);
  const bridge = useUi((s) => s.bridge);
  const [page, setPage] = useState<Page>("apps");
  const [detailsId, setDetailsId] = useState<string | null>(null);

  useEffect(() => {
    applyAppearance(prefs);
    if (launcher) {
      document.body.style.background = "transparent";
      document.documentElement.style.background = "transparent";
    }
  }, [prefs, launcher]);

  useEffect(() => {
    if (launcher) return;
    let cancelled = false;
    async function load() {
      try {
        const next = await api.getPreferences();
        if (cancelled) return;
        useUi.getState().setPrefs(next);
        useUi.getState().setBridge("connected");
        useUi.getState().setInfo(await api.appInfo());
        await refreshCatalog();
      } catch (error) {
        if (!cancelled) {
          useUi.getState().setBridge("unavailable");
          notify("Desktop bridge", errorText(error));
        }
      }
    }
    void load();
    const subs = [
      listen<{ title: string; body: string }>("app-notice", (event) => notify(event.payload.title, event.payload.body)).catch(() => () => undefined),
      listen<ScanReport>("scan-finished", (event) => {
        const report = event.payload;
        const extra = report.errors[0] ? ` ${report.errors[0]}` : "";
        notify("Scan finished", `Found ${report.found}. Added ${report.added}, updated ${report.updated}.${extra}`);
        void refreshCatalog();
      }).catch(() => () => undefined),
    ];
    return () => {
      cancelled = true;
      subs.forEach((pending) => void pending.then((unlisten) => unlisten()));
    };
  }, [launcher]);

  if (launcher) return <Launcher />;

  const title = detailsId ? "App details" : NAV.find((item) => item.id === page)?.label ?? "PulseLauncher";

  return (
    <div className="flex h-full bg-[var(--color-bg)] text-[var(--color-ink)]">
      <aside className="flex w-[220px] shrink-0 flex-col border-r border-[var(--color-line)] bg-[var(--color-raised)] p-3">
        <div className="mb-5 flex items-center gap-2 px-2">
          <img src="/pulselauncher.svg" width={32} height={32} alt="" />
          <div>
            <div className="font-semibold leading-tight">PulseLauncher</div>
            <div className="text-xs text-[var(--color-muted)]">ZeroPulse</div>
          </div>
        </div>
        <nav className="flex flex-col gap-1" aria-label="Main">
          {NAV.map((item) => {
            const Icon = item.icon;
            const active = !detailsId && page === item.id;
            return (
              <button
                key={item.id}
                className={`btn justify-start ${active ? "border-[var(--color-accent)] bg-[var(--color-panel)]" : "btn-ghost"}`}
                aria-current={active ? "page" : undefined}
                onClick={() => { setDetailsId(null); setPage(item.id); }}
              >
                <Icon size={16} /> {item.label}
              </button>
            );
          })}
        </nav>
        <div className="mt-auto px-2 text-xs text-[var(--color-faint)]">
          <p>{apps.length} apps · {collections.length} collections</p>
          <p className="mt-1">{info ? info.platform : bridge === "unavailable" ? "Desktop bridge unavailable" : "Connecting"}</p>
        </div>
      </aside>
      <div className="flex min-w-0 flex-1 flex-col">
        <header className="flex items-start gap-3 border-b border-[var(--color-line)] px-5 py-3">
          <h1 className="py-1.5 text-lg font-semibold">{title}</h1>
          {notices.length > 0 ? (
            <div className="ml-auto flex w-80 max-w-[40%] flex-col gap-2">
              {notices.map((notice) => (
                <button
                  key={notice.id}
                  className="card p-3 text-left"
                  onClick={() => useUi.getState().dismiss(notice.id)}
                >
                  <span className="block font-medium">{notice.title}</span>
                  <span className="block text-sm text-[var(--color-muted)]">{notice.body}</span>
                </button>
              ))}
            </div>
          ) : null}
          <button className={`btn shrink-0 ${notices.length > 0 ? "" : "ml-auto"}`} onClick={() => void api.showLauncher().catch((error) => notify("Launcher", errorText(error)))}>
            <Search size={16} /> Open launcher
          </button>
        </header>
        <main className="min-h-0 flex-1 overflow-auto p-5">
          {bridge === "unavailable" ? (
            <p className="mb-4 rounded-xl border border-[var(--color-line)] bg-[var(--color-panel)] px-4 py-3 text-sm text-[var(--color-muted)]">
              The desktop bridge is not connected. Lists stay empty because PulseLauncher does not invent apps. Open the PulseLauncher window to scan, launch, and save.
            </p>
          ) : null}
          {info && !info.windowsNative ? (
            <p className="mb-4 rounded-xl border border-[var(--color-warn)] bg-[var(--color-panel)] px-4 py-3 text-sm">
              This host is {info.platform}. Start Menu discovery, packaged apps, and the Windows launch path are inactive here. Adding a program still registers a real file.
            </p>
          ) : null}
          {!prefs.onboardingComplete && page === "apps" && !detailsId ? (
            <section className="card mb-4 p-4">
              <h2 className="text-lg font-semibold">On this computer only</h2>
              <p className="mt-1 max-w-3xl text-sm text-[var(--color-muted)]">
                PulseLauncher keeps its database next to your user data. There is no account. On Windows it reads Start Menu shortcuts and can launch what it finds. It never uninstalls a program. If a scan finds nothing, add a program yourself.
              </p>
              <button className="btn btn-primary mt-3" onClick={() => void finishOnboarding(prefs)}>Continue</button>
            </section>
          ) : null}
          {detailsId ? (
            <DetailsView
              id={detailsId}
              collections={collections}
              icon={icons[detailsId]}
              onBack={() => setDetailsId(null)}
              onChanged={() => void refreshCatalog()}
              onNotice={notify}
            />
          ) : page === "apps" ? (
            <AppsView
              apps={apps}
              collections={collections}
              icons={icons}
              onOpen={setDetailsId}
              onChanged={() => void refreshCatalog()}
              onNotice={notify}
            />
          ) : page === "workflows" ? (
            <WorkflowsView apps={apps} onNotice={notify} />
          ) : (
            <SettingsView prefs={prefs} onPrefs={(next) => useUi.getState().setPrefs(next)} onNotice={notify} />
          )}
        </main>
      </div>
    </div>
  );
}

async function refreshCatalog() {
  const [apps, collections] = await Promise.all([api.listApps(false), api.listCollections()]);
  useUi.getState().setApps(apps);
  useUi.getState().setCollections(collections);
  if (apps.length) {
    useUi.getState().mergeIcons(await api.appIcons(apps.map((app) => app.id)));
  }
}

async function finishOnboarding(prefs: UserPreferences) {
  const next = { ...prefs, onboardingComplete: true };
  try {
    await api.savePreferences(next);
    useUi.getState().setPrefs(next);
  } catch (error) {
    useUi.getState().setPrefs(next);
    notify("Onboarding", errorText(error));
  }
}
