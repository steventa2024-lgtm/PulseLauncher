import type { ReactNode } from "react";

export function Icon({ src, name }: { src?: string | null; name: string }) {
  if (src) return <img className="app-icon" src={src} alt="" />;
  return (
    <span className="icon-fallback" aria-hidden="true">
      {name.slice(0, 1).toUpperCase()}
    </span>
  );
}

export function Empty({ title, body, action }: { title: string; body: string; action?: ReactNode }) {
  return (
    <div className="empty card" role="status">
      <h2>{title}</h2>
      <p>{body}</p>
      {action}
    </div>
  );
}

export function Chip({ kind, children }: { kind?: "ok" | "warn" | "bad"; children: ReactNode }) {
  return <span className={kind ? `chip ${kind}` : "chip"}>{children}</span>;
}
