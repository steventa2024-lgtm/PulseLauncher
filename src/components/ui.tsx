import type { ButtonHTMLAttributes, ReactNode } from "react";
import { highlightParts, statusLabel } from "../lib/text";

export function cx(...parts: Array<string | false | null | undefined>): string {
  return parts.filter(Boolean).join(" ");
}

export function Button({
  variant = "ghost",
  className,
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & { variant?: "primary" | "ghost" | "danger" }) {
  const styles = {
    primary: "bg-soft text-inktext hover:brightness-110",
    ghost: "border border-line bg-panel text-foreground hover:border-bright",
    danger: "border border-danger/50 bg-panel text-danger hover:bg-danger/10",
  }[variant];
  return (
    <button
      className={cx(
        "inline-flex items-center justify-center gap-2 rounded-lg px-3 py-2 text-sm font-semibold disabled:cursor-not-allowed disabled:opacity-50",
        styles,
        className,
      )}
      {...props}
    />
  );
}

export function Field({ label, hint, children }: { label: string; hint?: string; children: ReactNode }) {
  return (
    <label className="block text-sm">
      <span className="mb-1 block font-medium">{label}</span>
      {children}
      {hint ? <span className="mt-1 block text-xs text-muted">{hint}</span> : null}
    </label>
  );
}

export const controlClass =
  "w-full rounded-lg border border-line bg-ink px-3 py-2 text-sm text-foreground outline-none focus:ring-2 focus:ring-accent";

export function Banner({ tone = "info", children }: { tone?: "info" | "warn" | "danger" | "ok"; children: ReactNode }) {
  const toneClass = {
    info: "border-line text-muted",
    warn: "border-warn/50 text-warn",
    danger: "border-danger/50 text-danger",
    ok: "border-ok/50 text-ok",
  }[tone];
  return <div className={cx("rounded-card border bg-panel px-3 py-2 text-sm", toneClass)}>{children}</div>;
}

export function EmptyState({ title, body, action }: { title: string; body: string; action?: ReactNode }) {
  return (
    <div className="rounded-card border border-dashed border-line bg-panel px-6 py-10 text-center">
      <h2 className="text-lg font-semibold">{title}</h2>
      <p className="mx-auto mt-2 max-w-lg text-sm text-muted">{body}</p>
      {action ? <div className="mt-4">{action}</div> : null}
    </div>
  );
}

export function StatusBadge({ status }: { status: string }) {
  const tone =
    status === "ready" || status === "completed" || status === "accepted"
      ? "text-ok border-ok/40"
      : status === "needsSetup" || status === "running"
        ? "text-warn border-warn/40"
        : status === "unavailable" || status === "failed" || status === "rejected" || status === "missing"
          ? "text-danger border-danger/40"
          : "text-muted border-line";
  return <span className={cx("rounded-full border px-2 py-0.5 text-xs font-semibold", tone)}>{statusLabel(status)}</span>;
}

export function Highlight({ text, indices }: { text: string; indices: number[] }) {
  return (
    <>
      {highlightParts(text, indices).map((part, index) =>
        part.hit ? (
          <mark key={index} className="bg-transparent font-semibold text-bright">
            {part.text}
          </mark>
        ) : (
          <span key={index}>{part.text}</span>
        ),
      )}
    </>
  );
}

export function Dialog({ title, children, onClose }: { title: string; children: ReactNode; onClose: () => void }) {
  return (
    <div className="fixed inset-0 z-40 flex items-center justify-center bg-black/55 p-4" onMouseDown={onClose}>
      <div
        role="dialog"
        aria-modal="true"
        aria-label={title}
        className="max-h-[90vh] w-full max-w-xl overflow-auto rounded-card border border-line bg-elevated p-5 shadow-lift"
        onMouseDown={(event) => event.stopPropagation()}
      >
        <div className="mb-4 flex items-start justify-between gap-3">
          <h2 className="text-lg font-semibold">{title}</h2>
          <Button aria-label="Close dialog" onClick={onClose}>
            Close
          </Button>
        </div>
        {children}
      </div>
    </div>
  );
}

export function Kbd({ children }: { children: ReactNode }) {
  return <kbd className="rounded border border-line bg-ink px-1.5 py-0.5 text-[11px] text-muted">{children}</kbd>;
}
