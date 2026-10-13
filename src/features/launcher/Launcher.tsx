import { useEffect, useMemo, useRef, useState } from "react";
import { Search } from "lucide-react";
import { api, errorText } from "../../lib/api";
import { moveSelection } from "../../lib/text";
import type { SearchResult } from "../../lib/types";
import { AppMark, Highlight } from "../../components/ui";
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
  const [results, setResults] = useState<SearchResult[]>([]);
  const [index, setIndex] = useState(0);
  const [icons, setIcons] = useState<Record<string, string | null>>({});
  const [status, setStatus] = useState("");
  const input = useRef<HTMLInputElement>(null);
  const requestId = useRef(0);
  const selectedKey = results[index]?.key;
  const selectedKeyRef = useRef<string | undefined>(undefined);
  selectedKeyRef.current = selectedKey;

  useEffect(() => {
    input.current?.focus();
  }, []);

  useEffect(() => {
    const token = ++requestId.current;
    const handle = window.setTimeout(() => {
      const key = selectedKeyRef.current;
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
    <div className="flex h-full items-start justify-center bg-transparent p-3">
      <div className="card flex w-full max-w-[720px] flex-col overflow-hidden shadow-[var(--shadow-overlay)]" style={{ background: "color-mix(in srgb, var(--color-raised) 94%, transparent)" }}>
        <label className="flex items-center gap-3 border-b border-[var(--color-line)] px-4">
          <Search aria-hidden size={18} className="text-[var(--color-accent-2)]" />
          <input
            ref={input}
            className="h-14 w-full bg-transparent text-lg outline-none"
            placeholder="Search apps, commands, workflows"
            aria-label="Search PulseLauncher"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
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
              }
            }}
          />
        </label>
        <ul role="listbox" aria-label="Search results" className="scroll max-h-[360px]">
          {results.length === 0 ? (
            <li className="px-4 py-8 text-[var(--color-muted)]">
              {query ? "No matches." : "Favorites and recent apps appear here. Type to search everything PulseLauncher knows."}
            </li>
          ) : results.map((item, i) => (
            <li key={item.key} role="option" aria-selected={item.key === selectedKey}>
              <button
                className={`flex w-full items-center gap-3 px-4 py-2.5 text-left ${item.key === selectedKey ? "bg-[var(--color-panel-2)]" : ""}`}
                onMouseEnter={() => setIndex(i)}
                onClick={() => void activate(item)}
              >
                <AppMark name={item.title} icon={item.iconKey ? icons[item.iconKey] : null} size={32} />
                <span className="min-w-0 flex-1">
                  <span className="block truncate"><Highlight text={item.title} positions={item.highlights} /></span>
                  <span className="block truncate text-sm text-[var(--color-muted)]">{item.subtitle}</span>
                </span>
                <span className="pill">{KIND[item.kind] ?? item.kind}</span>
                {item.missing ? <span className="pill pill-danger">Missing</span> : null}
              </button>
            </li>
          ))}
        </ul>
        <footer className="flex items-center justify-between gap-3 border-t border-[var(--color-line)] px-4 py-2 text-xs text-[var(--color-muted)]">
          <span>{status || hint}</span>
          <span className="flex gap-1"><kbd className="kbd">@</kbd> apps <kbd className="kbd">&gt;</kbd> commands <kbd className="kbd">/</kbd> workflows</span>
        </footer>
      </div>
      <ConfirmPrompt onNotice={(_title, body) => setStatus(body)} />
    </div>
  );
}
