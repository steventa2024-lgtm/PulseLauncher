export interface TextPart {
  text: string;
  hit: boolean;
}

export function highlightParts(text: string, indices: number[]): TextPart[] {
  const chars = Array.from(text);
  const marks = new Set(indices);
  const parts: TextPart[] = [];
  let buffer = "";
  let hit = false;
  chars.forEach((char, index) => {
    const next = marks.has(index);
    if (buffer && next !== hit) {
      parts.push({ text: buffer, hit });
      buffer = "";
    }
    hit = next;
    buffer += char;
  });
  if (buffer) parts.push({ text: buffer, hit });
  return parts;
}

export function retainSelection(ids: string[], selectedId: string | null): string | null {
  if (selectedId && ids.includes(selectedId)) return selectedId;
  return ids[0] ?? null;
}

export function moveSelection(ids: string[], selectedId: string | null, delta: number): string | null {
  if (ids.length === 0) return null;
  const current = selectedId ? ids.indexOf(selectedId) : -1;
  const start = current < 0 ? 0 : current;
  const next = Math.min(ids.length - 1, Math.max(0, start + delta));
  return ids[next] ?? null;
}

export function argumentLines(text: string): string[] {
  return text
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line.length > 0);
}

export function formatBytes(value: number | null): string {
  if (value == null) return "Size unavailable";
  if (value < 1024) return `${value} B`;
  const units = ["KB", "MB", "GB"];
  let size = value / 1024;
  let unit = units[0];
  for (let index = 0; index < units.length; index += 1) {
    unit = units[index];
    if (size < 1024 || index === units.length - 1) break;
    size /= 1024;
  }
  return `${size.toFixed(size >= 10 ? 0 : 1)} ${unit}`;
}

export function sourceLabel(source: string): string {
  switch (source) {
    case "startMenuUser":
      return "Your Start Menu";
    case "startMenuCommon":
      return "Common Start Menu";
    case "desktopShortcuts":
      return "Desktop shortcut";
    case "manual":
      return "Manually registered";
    case "desktopFile":
      return "Desktop entry";
    case "packaged":
      return "Packaged app";
    case "uri":
      return "URI";
    default:
      return source;
  }
}

export function statusLabel(status: string): string {
  switch (status) {
    case "ready":
      return "Ready";
    case "needsSetup":
      return "Needs setup";
    case "unavailable":
      return "Unavailable";
    case "completed":
      return "Completed";
    case "running":
      return "Running";
    case "cancelled":
      return "Cancelled";
    case "failed":
      return "Failed";
    case "missing":
      return "Missing";
    default:
      return status;
  }
}

const ZEROPULSE = /zeropulse|pulsecut|pulseflow|openpulse|pulseapply/i;

export function matchesZeropulse(app: {
  id: string;
  displayName: string;
  publisher: string | null;
  launchTarget: string;
  tags: string[];
}, memberIds: ReadonlySet<string>): boolean {
  return (
    memberIds.has(app.id) ||
    ZEROPULSE.test(app.displayName) ||
    ZEROPULSE.test(app.publisher ?? "") ||
    ZEROPULSE.test(app.launchTarget) ||
    app.tags.some((tag) => ZEROPULSE.test(tag))
  );
}

export function newId(prefix: string): string {
  const cryptoApi = globalThis.crypto;
  const raw = cryptoApi?.randomUUID?.() ?? `${Date.now()}-${Math.random().toString(16).slice(2)}`;
  return `${prefix}-${raw}`;
}
