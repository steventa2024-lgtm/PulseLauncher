export function sourceLabel(source: string): string {
  switch (source) {
    case "start_menu_user":
      return "Your Start Menu";
    case "start_menu_common":
      return "All Users Start Menu";
    case "desktop":
      return "Desktop";
    case "packaged":
      return "Windows app";
    case "manual":
      return "Added by you";
    default:
      return source;
  }
}

export function kindLabel(kind: string): string {
  switch (kind) {
    case "shortcut":
      return "Shortcut";
    case "executable":
      return "Program";
    case "packaged":
      return "Packaged app";
    case "uri":
      return "Link";
    default:
      return kind;
  }
}

export function formatBytes(size: number | null): string {
  if (size == null) return "Not available";
  if (size < 1024) return `${size} B`;
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`;
  return `${(size / (1024 * 1024)).toFixed(1)} MB`;
}

export function formatWhen(iso: string | null): string {
  if (!iso) return "Never";
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  return date.toLocaleString();
}

export function readinessLabel(state: string): string {
  if (state === "ready") return "Ready";
  if (state === "needs_setup") return "Needs setup";
  if (state === "unavailable") return "Unavailable";
  return state;
}

/** Move a keyboard selection without letting it leave the list. */
export function moveSelection(index: number, delta: number, count: number): number {
  if (count <= 0) return 0;
  const next = index + delta;
  if (next < 0) return 0;
  if (next >= count) return count - 1;
  return next;
}

export function targetText(app: { target: { kind: string; launchTarget: string; resolvedTarget: string | null } }): string {
  if (app.target.kind === "shortcut" && app.target.resolvedTarget) return app.target.resolvedTarget;
  return app.target.launchTarget;
}
