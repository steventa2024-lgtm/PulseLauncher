import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, errorText } from "./lib/api";
import { notify, useUi } from "./lib/store";
import type { ScanReport, UserPreferences } from "./lib/types";
import { StudioHeader } from "./components/chrome";
import { AppsView } from "./features/apps/AppsView";
import { DetailsView } from "./features/details/DetailsView";
import { FIXTURE_APPS, studioFixtureEnabled } from "./features/fixture/studio";
import { Launcher } from "./features/launcher/Launcher";
import { SettingsView } from "./features/settings/SettingsView";
import { ConfirmPrompt } from "./features/workflows/ConfirmPrompt";
import { WorkflowsView } from "./features/workflows/WorkflowsView";

type Page = "apps" | "workflows" | "settings";

function resolvedTheme(theme: string) {
  if (theme === "system") {
    return window.matchMedia("(prefers-color-scheme: light)").matches ? "light" : "dark";
  }
  return theme === "light" ? "light" : "dark";
}

function applyAppearance(prefs: UserPreferences) {
  const root = document.documentElement;
  root.dataset.theme = resolvedTheme(prefs.theme);
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
  const params = new URLSearchParams(window.location.search);
  const requested = params.get("page");
  const [page, setPage] = useState<Page>(requested === "workflows" || requested === "settings" ? requested : "apps");
  const [detailsId, setDetailsId] = useState<string | null>(params.get("details"));

  useEffect(() => {
    applyAppearance(prefs);
    if (prefs.theme !== "system") return;
    const media = window.matchMedia("(prefers-color-scheme: light)");
    const onChange = () => applyAppearance(prefs);
    media.addEventListener("change", onChange);
    return () => media.removeEventListener("change", onChange);
  }, [prefs]);

  useEffect(() => {
    if (launcher) {
      document.body.style.background = "transparent";
      document.documentElement.style.background = "transparent";
    }
  }, [prefs, launcher]);

  useEffect(() => {
    if (launcher) return;
    if (studioFixtureEnabled()) {
      useUi.getState().setBridge("connected");
      useUi.getState().setApps(FIXTURE_APPS);
      useUi.getState().setPrefs({ ...useUi.getState().prefs, onboardingComplete: true });
      return;
    }
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
      listen("open-settings", () => setPage("settings")).catch(() => () => undefined),
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

  const fixture = studioFixtureEnabled();

  return (
    <div className="flex h-full flex-col bg-[var(--color-bg)] text-[var(--color-ink)]">
      <StudioHeader
        hotkey={prefs.launcherHotkey}
        showShortcut={prefs.showShortcuts}
        onBrand={() => { setDetailsId(null); setPage("apps"); }}
        onSettings={() => { setDetailsId(null); setPage("settings"); }}
        notices={notices}
        onDismiss={(id) => useUi.getState().dismiss(id)}
      />
      <main className="min-h-0 flex-1 overflow-auto p-4">
        {fixture ? (
          <p className="mb-3 rounded-xl border border-[var(--color-accent)] bg-[var(--color-panel)] px-4 py-2 text-sm">
            Fixture screenshot mode. These names are not installed and are not written to the database.
          </p>
        ) : null}
        {bridge === "unavailable" ? (
          <p className="mb-3 rounded-xl border border-[var(--color-line)] bg-[var(--color-panel)] px-4 py-3 text-sm text-[var(--color-muted)]">
            The desktop bridge is not connected. Lists stay empty because PulseLauncher does not invent apps. Open the PulseLauncher window to scan, launch, and save.
          </p>
        ) : null}
        {info && !info.windowsNative ? (
          <p className="mb-3 rounded-xl border border-[var(--color-warn)] bg-[var(--color-panel)] px-4 py-2 text-sm">
            This host is {info.platform}. Start Menu discovery, packaged apps, and the Windows launch path are inactive here. Adding a program still registers a real file.
          </p>
        ) : null}
        {!prefs.onboardingComplete && page === "apps" && !detailsId && !fixture ? (
          <section className="card mb-3 p-4">
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
            icon={prefs.showAppIcons ? icons[detailsId] : null}
            onBack={() => setDetailsId(null)}
            onChanged={() => void refreshCatalog()}
            onNotice={notify}
          />
        ) : page === "apps" ? (
          <AppsView
            apps={apps}
            collections={collections}
            icons={prefs.showAppIcons ? icons : {}}
            onOpen={setDetailsId}
            onChanged={() => void refreshCatalog()}
            onNotice={notify}
            onOpenWorkflows={() => setPage("workflows")}
            onOpenSettings={() => setPage("settings")}
          />
        ) : page === "workflows" ? (
          <WorkflowsView apps={apps} onNotice={notify} />
        ) : (
          <SettingsView prefs={prefs} onPrefs={(next) => useUi.getState().setPrefs(next)} onNotice={notify} />
        )}
      </main>
      <ConfirmPrompt onNotice={notify} />
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
