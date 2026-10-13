import { useEffect, useMemo, useRef, useState } from "react";
import { emit } from "@tauri-apps/api/event";
import { api, errorText } from "../../lib/api";
import { useUi } from "../../lib/store";
import { moveSelection } from "../../lib/text";
import type { SearchResult } from "../../lib/types";
import { HotkeyPill, SearchBox, StudioHeader } from "../../components/chrome";
import { AppMark, Highlight } from "../../components/ui";
import { fixtureSearch, studioFixtureEnabled } from "../fixture/studio";
import { ConfirmPrompt } from "../workflows/ConfirmPrompt";

const KIND: Record<string, string> = {
  app: "App",
  command: "Command",
  workflow: "Workflow",
  file: "File",
  folder: "Folder",
  calculator: "Calculator",
  web_search: "Web",
};

export function Launcher() {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SearchResult[]>(() => (studioFixtureEnabled() ? fixtureSearch("") : []));
  const [index, setIndex] = useState(0);
  const [icons, setIcons] = useState<Record<string, string | null>>({});
  const [status, setStatus] = useState("");
  const input = useRef<HTMLInputElement>(null);
  const requestId = useRef(0);
  const selectedKey = results[index]?.key;
  const selectedKeyRef = useRef<string | undefined>(undefined);
  selectedKeyRef.current = selectedKey;

  const prefs = useUi((s) => s.prefs);

  useEffect(() => {
    input.current?.focus();
    if (studioFixtureEnabled()) return;
    api.getPreferences().then((next) => useUi.getState().setPrefs(next)).catch(() => undefined);
  }, []);

  useEffect(() => {
    const token = ++requestId.current;
    const handle = window.setTimeout(() => {
      const key = selectedKeyRef.current;
      if (studioFixtureEnabled()) {
        const found = fixtureSearch(query);
        setResults(found);
        const kept = key ? found.findIndex((item) => item.key === key) : -1;
        setIndex(kept >= 0 ? kept : 0);
        return;
      }
      api.search(query).then((found) => {
        if (token !== requestId.current) return;
        setResults(found);
        const kept = key ? found.findIndex((item) => item.key === key) : -1;
        setIndex(kept >= 0 ? kept : 0);
        const ids = found.map((item) => item.iconKey).filter((id): id is string => !!id);
        if (ids.length) {
          api.appIcons(ids).then((pairs) => {
            if (token !== requestId.current) return;
            setIcons((prev) => {
              const next = { ...prev };
              for (const [id, icon] of pairs) next[id] = icon;
              return next;
            });
          }).catch(() => undefined);
        }
      }).catch((error) => {
        if (token !== requestId.current) return;
        setStatus(errorText(error));
      });
    }, 40);
    return () => window.clearTimeout(handle);
  }, [query]);

  const hint = useMemo(() => "↑↓ select · Enter open · Esc close · Ctrl+K commands", []);

  async function activate(item: SearchResult | undefined) {
    if (!item) return;
    if (studioFixtureEnabled()) {
      setStatus("Fixture screenshot mode. Nothing is launched and nothing is written to the database.");
      return;
    }
    try {
      if (item.kind === "app" && item.appId) {
        const outcome = await api.launchApp(item.appId);
        setStatus(outcome.message);
        await api.hideLauncher();
      } else if ((item.kind === "command" || item.kind === "workflow") && item.workflowId) {
        const run = await api.runWorkflow(item.workflowId);
        setStatus(run.error ?? `Finished: ${run.status}`);
      } else if ((item.kind === "file" || item.kind === "folder") && item.path) {
        const outcome = await api.openIndexed(item.path);
        setStatus(outcome.message);
        await api.hideLauncher();
      } else if (item.kind === "calculator" && item.value) {
        await api.copyText(item.value);
        setStatus(`Copied ${item.value}`);
      } else if (item.kind === "web_search" && item.value) {
        const outcome = await api.openUrl(item.value);
        setStatus(outcome.message);
        await api.hideLauncher();
      }
    } catch (error) {
      setStatus(errorText(error));
    }
  }

  return (
    <div
      className={`relative flex h-full items-start justify-center p-3 ${studioFixtureEnabled() || prefs.translucency ? "bg-transparent" : "bg-[var(--color-bg)]"}`}
      style={studioFixtureEnabled() ? { background: "radial-gradient(900px 420px at 18% 10%, #4d8dff 0%, transparent 58%), radial-gradient(700px 380px at 88% 90%, #163a86 0%, transparent 52%), linear-gradient(165deg, #071433 0%, #12356f 46%, #081426 100%)" } : undefined}
    >
      {studioFixtureEnabled() ? <p className="pointer-events-none absolute bottom-3 left-3 text-xs text-white/80">Fixture wallpaper. Not a Windows desktop and not shipped as the overlay background.</p> : null}
      <div className="card flex w-full max-w-[720px] flex-col overflow-hidden shadow-[var(--shadow-overlay)]" style={{ background: prefs.translucency ? "color-mix(in srgb, var(--color-raised) 92%, transparent)" : "var(--color-raised)" }}>
        <StudioHeader
          hotkey={prefs.launcherHotkey}
          showShortcut={prefs.showShortcuts}
          onSettings={() => { void api.showMain().then(() => emit("open-settings")).catch(() => undefined); }}
        />
        <div className="p-3">
          <SearchBox
            inputRef={input}
            value={query}
            onChange={setQuery}
            placeholder="Type to search apps, files, commands, or ask AI…"
            label="Search PulseLauncher"
            onKeyDown={(e) => {
              if (e.key === "ArrowDown") {
                e.preventDefault();
                setIndex((i) => moveSelection(i, 1, results.length));
              } else if (e.key === "ArrowUp") {
                e.preventDefault();
                setIndex((i) => moveSelection(i, -1, results.length));
              } else if (e.key === "Enter") {
                e.preventDefault();
                void activate(results[index]);
              } else if (e.key === "Escape") {
                void api.hideLauncher();
              } else if (e.key === "k" && (e.ctrlKey || e.metaKey)) {
                e.preventDefault();
                setQuery((q) => (q.startsWith(">") ? q : "> "));
              } else if (e.altKey && e.key >= "1" && e.key <= "5") {
                e.preventDefault();
                const row = results[Number(e.key) - 1];
                if (row) void activate(row);
              }
            }}
          />
        </div>
        <ul role="listbox" aria-label="Search results" className="scroll max-h-[360px] px-2 pb-2">
          {results.length === 0 ? (
            <li className="px-3 py-8 text-sm text-[var(--color-muted)]">
              {query ? "No matches." : "Favorites and recent apps appear here. Type to search everything PulseLauncher knows."}
            </li>
          ) : results.map((item, i) => (
            <li key={item.key} role="option" aria-selected={item.key === selectedKey} className="mb-1">
              <button
                className={`flex w-full items-center gap-3 rounded-xl px-3 py-2 text-left ${item.key === selectedKey ? "row-selected" : "border border-transparent"}`}
                onMouseEnter={() => setIndex(i)}
                onClick={() => void activate(item)}
              >
                <AppMark name={item.title} icon={prefs.showAppIcons && item.iconKey ? icons[item.iconKey] : null} size={36} />
                <span className="min-w-0 flex-1">
                  <span className="block truncate font-medium"><Highlight text={item.title} positions={item.highlights} /></span>
                  <span className="block truncate text-xs text-[var(--color-muted)]">{item.subtitle}</span>
                </span>
                {prefs.showShortcuts ? (
                  <HotkeyPill value={item.key === selectedKey ? "Enter" : i < 5 ? `Alt+${i + 1}` : KIND[item.kind] ?? item.kind} />
                ) : null}
                {item.missing ? <span className="pill pill-danger">Missing</span> : null}
              </button>
            </li>
          ))}
        </ul>
        <footer className="px-4 py-2 text-xs text-[var(--color-muted)]">{status || hint}</footer>
      </div>
      <ConfirmPrompt onNotice={(_title, body) => setStatus(body)} />
    </div>
  );
}
