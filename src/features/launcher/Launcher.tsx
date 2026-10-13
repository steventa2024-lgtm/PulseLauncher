import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { highlightParts } from "../../lib/highlight";
import * as api from "../../lib/api";
import type { SearchResult } from "../../lib/types";

export function Launcher() {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SearchResult[]>([]);
  const [sel, setSel] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [hint, setHint] = useState<string | null>(null);
  const input = useRef<HTMLInputElement>(null);

  useEffect(() => {
    input.current?.focus();
  }, []);

  useEffect(() => {
    let live = true;
    const timer = window.setTimeout(() => {
      api
        .searchApps(query)
        .then((rows) => {
          if (!live) return;
          setResults(rows);
          setSel(0);
          setError(null);
        })
        .catch((e: unknown) => {
          if (live) setError(api.errorText(e));
        });
    }, 40);
    return () => {
      live = false;
      window.clearTimeout(timer);
    };
  }, [query]);

  async function activate(row: SearchResult) {
    setHint(null);
    try {
      const action = row.action;
      if (action.type === "launchApp") {
        const result = await api.launchApp(action.appId);
        setHint(result.message);
        if (result.outcome !== "accepted") return;
      } else if (action.type === "copyText") {
        await api.copyText(action.text);
        setHint(`Copied ${action.text}`);
      } else if (action.type === "webSearch") {
        await api.openUrl(action.url);
      } else if (action.type === "openPath") {
        await api.openPath(action.path);
      } else if (action.type === "runWorkflow") {
        await api.showMain();
        await api.runWorkflow(action.workflowId, {});
      }
      await api.hideLauncher();
    } catch (e) {
      setError(api.errorText(e));
    }
  }

  function onKey(event: KeyboardEvent<HTMLInputElement>) {
    if (event.key === "ArrowDown") {
      event.preventDefault();
      setSel((i) => Math.min(results.length - 1, i + 1));
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      setSel((i) => Math.max(0, i - 1));
    } else if (event.key === "Enter" && event.altKey && results[sel]?.appId) {
      event.preventDefault();
      void api.openDetails(results[sel].appId as string);
    } else if (event.key === "Enter" && results[sel]) {
      event.preventDefault();
      void activate(results[sel]);
    } else if (event.key === "Escape") {
      event.preventDefault();
      void api.hideLauncher();
    } else if (event.key.toLowerCase() === "k" && event.ctrlKey) {
      event.preventDefault();
      setQuery((q) => (q.startsWith(">") ? q : `>${q}`));
    }
  }

  return (
    <div className="launcher">
      <div className="palette" role="dialog" aria-label="PulseLauncher">
        <input
          ref={input}
          aria-label="Search apps, commands and workflows"
          placeholder="Search apps, commands, workflows…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={onKey}
          autoFocus
        />
        <div className="tiny" style={{ padding: "0 16px 8px" }}>
          <span className="kbd">↑↓</span> move <span className="kbd">Enter</span> open <span className="kbd">Alt+Enter</span> details{" "}
          <span className="kbd">Esc</span> close <span className="kbd">Ctrl+K</span> commands · <span className="kbd">@</span> apps{" "}
          <span className="kbd">/</span> workflows <span className="kbd">=</span> calc <span className="kbd">?</span> web
        </div>
        {error && (
          <p className="notice" role="alert">
            {error}
          </p>
        )}
        {hint && <p className="notice">{hint}</p>}
        <div className="results" role="listbox" aria-label="Results">
          {results.length === 0 && !error && <div className="empty">No matches. Register an app from Apps & Tools, or type =2+2.</div>}
          {results.map((row, index) => (
            <button
              key={row.id}
              role="option"
              aria-selected={index === sel}
              aria-label={`${row.title}. ${row.subtitle}`}
              className="result"
              onMouseEnter={() => setSel(index)}
              onClick={() => void activate(row)}
            >
              <span className="chip">{row.category}</span>
              <span>
                <strong>
                  {highlightParts(row.title, row.matchIndices).map((part, i) =>
                    part.hit ? (
                      <mark className="hit" key={i}>
                        {part.text}
                      </mark>
                    ) : (
                      <span key={i}>{part.text}</span>
                    ),
                  )}
                </strong>
                <div className="tiny">{row.section ? `${row.section} · ` : ""}{row.subtitle}</div>
              </span>
              {row.favorite && <span className="chip">Pinned</span>}
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}
