import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from "react";
import { Highlight, Kbd } from "../components/ui";
import { api, DesktopError } from "../lib/api";
import type { SearchHit } from "../lib/types";
import { moveSelection, retainSelection } from "../lib/text";

export function Launcher() {
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<SearchHit[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const ids = useMemo(() => hits.map((hit) => hit.id), [hits]);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  useEffect(() => {
    const timer = window.setTimeout(() => {
      void api.search(query).then((next) => {
        setHits(next);
        setSelectedId((current) => retainSelection(next.map((hit) => hit.id), current));
      }).catch((error: Error) => setMessage(error.message));
    }, 80);
    return () => window.clearTimeout(timer);
  }, [query]);

  async function activate(hit: SearchHit | undefined) {
    if (!hit || busy) return;
    setBusy(true);
    setMessage(null);
    try {
      if (hit.kind === "app" && hit.appId) {
        const outcome = await api.appsLaunch(hit.appId);
        setMessage(outcome.message);
        await api.hideLauncher();
        return;
      }
      const result = await api.searchActivate(query, hit.id);
      setMessage(result);
      if (hit.kind !== "workflow") await api.hideLauncher();
    } catch (error) {
      setMessage(error instanceof DesktopError ? error.message : "Activation failed.");
    } finally {
      setBusy(false);
    }
  }

  function onKeyDown(event: KeyboardEvent) {
    if (event.key === "ArrowDown") {
      event.preventDefault();
      setSelectedId((current) => moveSelection(ids, current, 1));
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      setSelectedId((current) => moveSelection(ids, current, -1));
    } else if (event.key === "Enter") {
      event.preventDefault();
      const hit = hits.find((item) => item.id === selectedId);
      if (event.ctrlKey && hit?.appId) {
        void api.openDetails(hit.appId).then(() => api.hideLauncher());
        return;
      }
      void activate(hit);
    } else if (event.key === "Escape") {
      void api.hideLauncher();
    } else if (event.key.toLowerCase() === "k" && event.ctrlKey) {
      event.preventDefault();
      setQuery((current) => (current.startsWith(">") ? current : "> "));
      inputRef.current?.focus();
    }
  }

  return (
    <div className="flex h-full flex-col bg-elevated text-foreground" onKeyDown={onKeyDown}>
      <header className="border-b border-line px-4 py-3">
        <div className="mb-2 flex items-center justify-between text-xs text-muted">
          <span>PulseLauncher</span>
          <span>@ apps · &gt; commands · / workflows</span>
        </div>
        <input
          ref={inputRef}
          aria-label="Search apps, commands, and workflows"
          className="w-full bg-transparent text-lg outline-none placeholder:text-muted"
          placeholder="Search or calculate"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
        />
      </header>
      <ul className="min-h-0 flex-1 overflow-auto p-2" role="listbox" aria-label="Search results">
        {hits.length === 0 ? (
          <li className="px-3 py-8 text-center text-sm text-muted">
            {message ?? "No matches yet. Discovered apps appear after the desktop scan."}
          </li>
        ) : (
          hits.map((hit) => {
            const selected = hit.id === selectedId;
            return (
              <li key={hit.id}>
                <button
                  role="option"
                  aria-selected={selected}
                  className={`flex w-full items-start gap-3 rounded-lg px-3 py-2 text-left ${selected ? "bg-panel" : "hover:bg-panel/70"}`}
                  onMouseEnter={() => setSelectedId(hit.id)}
                  onClick={() => void activate(hit)}
                >
                  <span className="mt-0.5 rounded border border-line px-1.5 py-0.5 text-[10px] uppercase text-muted">{hit.kind}</span>
                  <span className="min-w-0">
                    <span className="block truncate text-sm">
                      <Highlight text={hit.title} indices={hit.indices} />
                    </span>
                    <span className="block truncate text-xs text-muted">{hit.subtitle}</span>
                  </span>
                </button>
              </li>
            );
          })
        )}
      </ul>
      <footer className="flex flex-wrap items-center gap-3 border-t border-line px-4 py-2 text-xs text-muted">
        <span><Kbd>↑</Kbd> <Kbd>↓</Kbd> move</span>
        <span><Kbd>Enter</Kbd> open</span>
        <span><Kbd>Ctrl</Kbd>+<Kbd>Enter</Kbd> details</span>
        <span><Kbd>Ctrl</Kbd>+<Kbd>K</Kbd> commands</span>
        <span><Kbd>Esc</Kbd> close</span>
        {message ? <span className="text-foreground">{message}</span> : null}
      </footer>
    </div>
  );
}

export function ResultList({
  hits,
  selectedId,
}: {
  hits: SearchHit[];
  selectedId: string | null;
}) {
  return (
    <ul>
      {hits.map((hit) => (
        <li key={hit.id} data-selected={hit.id === selectedId ? "true" : "false"}>
          <Highlight text={hit.title} indices={hit.indices} />
        </li>
      ))}
    </ul>
  );
}
