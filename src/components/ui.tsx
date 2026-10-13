import type { ReactNode } from "react";

export function Highlight({ text, positions }: { text: string; positions: number[] }) {
  if (positions.length === 0) return <>{text}</>;
  const marks = new Set(positions);
  const chars = Array.from(text);
  const parts: ReactNode[] = [];
  let buf = "";
  let marked = false;
  chars.forEach((ch, i) => {
    const on = marks.has(i);
    if (on !== marked && buf) {
      parts.push(marked ? <mark key={parts.length} className="mark">{buf}</mark> : <span key={parts.length}>{buf}</span>);
      buf = "";
    }
    marked = on;
    buf += ch;
  });
  if (buf) parts.push(marked ? <mark key={parts.length} className="mark">{buf}</mark> : <span key={parts.length}>{buf}</span>);
  return <>{parts}</>;
}

export function AppMark({ name, icon, size = 40 }: { name: string; icon?: string | null; size?: number }) {
  const letter = Array.from(name.trim() || "?")[0]?.toUpperCase() ?? "?";
  return (
    <span
      className="inline-flex shrink-0 items-center justify-center overflow-hidden rounded-xl border border-[var(--color-line)] bg-[var(--color-panel-2)]"
      style={{ width: size, height: size }}
      aria-hidden
    >
      {icon ? (
        <img src={`data:image/x-icon;base64,${icon}`} alt="" width={size - 8} height={size - 8} />
      ) : (
        <span className="text-[var(--color-accent-2)]" style={{ fontSize: size * 0.4 }}>{letter}</span>
      )}
    </span>
  );
}

export function Empty({ title, body, action }: { title: string; body: string; action?: ReactNode }) {
  return (
    <div className="card flex flex-col items-start gap-3 p-8">
      <h2 className="text-lg font-semibold">{title}</h2>
      <p className="max-w-xl text-[var(--color-muted)]">{body}</p>
      {action}
    </div>
  );
}

export function Modal({ title, children, onClose }: { title: string; children: ReactNode; onClose: () => void }) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4" role="presentation" onMouseDown={onClose}>
      <div
        role="dialog"
        aria-modal="true"
        aria-label={title}
        className="card max-h-[90vh] w-full max-w-lg overflow-auto p-5 shadow-[var(--shadow-overlay)]"
        onMouseDown={(e) => e.stopPropagation()}
      >
        <div className="mb-4 flex items-center justify-between gap-3">
          <h2 className="text-lg font-semibold">{title}</h2>
          <button className="btn btn-ghost" onClick={onClose} aria-label="Close dialog">Close</button>
        </div>
        {children}
      </div>
    </div>
  );
}

export function Field({ label, children, hint }: { label: string; children: ReactNode; hint?: string }) {
  return (
    <label className="flex flex-col gap-1 text-sm">
      <span>{label}</span>
      {children}
      {hint ? <span className="text-xs text-[var(--color-muted)]">{hint}</span> : null}
    </label>
  );
}
