import type { KeyboardEvent, ReactNode, Ref } from "react";
import { Search, Settings } from "lucide-react";
import type { Notice } from "../lib/types";

export function formatHotkey(value: string): string {
  return value.replace(/\s*\+\s*/g, " + ");
}

export function PulseBrand({ onClick }: { onClick?: () => void }) {
  const inner = (
    <>
      <img src="/pulselauncher.svg" width={28} height={28} alt="" />
      <span className="text-[15px] font-semibold tracking-tight">
        Pulse<span className="text-[var(--color-accent)]">Launcher</span>
      </span>
    </>
  );
  if (!onClick) return <div className="flex items-center gap-2">{inner}</div>;
  return (
    <button className="flex items-center gap-2 rounded-lg" onClick={onClick}>
      {inner}
    </button>
  );
}

export function HotkeyPill({ value }: { value: string }) {
  return <span className="kbd px-2 py-1 text-xs">{formatHotkey(value)}</span>;
}

export function StudioHeader({
  hotkey,
  showShortcut,
  onBrand,
  onSettings,
  notices,
  onDismiss,
}: {
  hotkey: string;
  showShortcut: boolean;
  onBrand?: () => void;
  onSettings?: () => void;
  notices?: Notice[];
  onDismiss?: (id: number) => void;
}) {
  return (
    <header className="flex items-center gap-3 border-b border-[var(--color-line)] bg-[var(--color-raised)] px-4 py-2.5">
      <PulseBrand onClick={onBrand} />
      {notices && notices.length > 0 ? (
        <div className="ml-auto flex max-h-16 min-w-0 flex-1 flex-col gap-1 overflow-y-auto">
          {[...notices].reverse().map((notice) => (
            <button
              key={notice.id}
              className="card px-3 py-1.5 text-left"
              onClick={() => onDismiss?.(notice.id)}
            >
              <span className="block text-sm font-medium">{notice.title}</span>
              <span className="block truncate text-xs text-[var(--color-muted)]">{notice.body}</span>
            </button>
          ))}
        </div>
      ) : <span className="ml-auto" />}
      {showShortcut ? <HotkeyPill value={hotkey || "Alt+Space"} /> : null}
      {onSettings ? (
        <button className="btn btn-ghost h-8 w-8 px-0" aria-label="Settings" onClick={onSettings}>
          <Settings size={16} />
        </button>
      ) : null}
    </header>
  );
}

export function SearchBox({
  value,
  onChange,
  placeholder,
  label,
  onKeyDown,
  inputRef,
}: {
  value: string;
  onChange: (value: string) => void;
  placeholder: string;
  label: string;
  onKeyDown?: (event: KeyboardEvent<HTMLInputElement>) => void;
  inputRef?: Ref<HTMLInputElement>;
}) {
  return (
    <label className="search-ring flex items-center gap-2 rounded-xl px-3">
      <Search size={16} aria-hidden className="shrink-0 text-[var(--color-muted)]" />
      <input
        ref={inputRef}
        className="h-11 w-full border-0 bg-transparent text-[15px] outline-none"
        placeholder={placeholder}
        aria-label={label}
        value={value}
        onChange={(event) => onChange(event.target.value)}
        onKeyDown={onKeyDown}
      />
    </label>
  );
}

export function ToggleSwitch({ label, checked, onChange, hint }: { label: string; checked: boolean; onChange: (value: boolean) => void; hint?: string }) {
  return (
    <label className="flex items-center justify-between gap-3 py-1 text-sm">
      <span>
        {label}
        {hint ? <span className="mt-0.5 block text-xs text-[var(--color-muted)]">{hint}</span> : null}
      </span>
      <button
        type="button"
        role="switch"
        aria-checked={checked}
        aria-label={label}
        className="switch shrink-0"
        data-on={checked ? "true" : "false"}
        onClick={() => onChange(!checked)}
      >
        <i />
      </button>
    </label>
  );
}

export function SidebarItem({ active, icon, label, count, onClick }: { active: boolean; icon?: ReactNode; label: string; count?: number; onClick: () => void }) {
  return (
    <button className="nav-item" aria-current={active ? "page" : undefined} onClick={onClick}>
      {icon}
      <span className="min-w-0 flex-1 text-left leading-snug">{label}</span>
      {count !== undefined ? <span className="text-xs text-[var(--color-faint)]">{count}</span> : null}
    </button>
  );
}
