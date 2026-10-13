import type { AppDetails, AppEntry, SearchResult, WorkflowView } from "../../lib/types";

/** Screenshot harness only. Production windows never set this query. */
export function studioFixtureEnabled(): boolean {
  return new URLSearchParams(window.location.search).get("fixture") === "studio";
}

function app(id: string, name: string, category: string, favorite = false): AppEntry {
  return {
    id,
    name,
    originalName: name,
    source: "manual",
    target: {
      kind: "executable",
      launchTarget: `fixture:${id}`,
      resolvedTarget: null,
      arguments: [],
      linkArguments: null,
      workingDir: null,
    },
    iconPath: null,
    iconIndex: 0,
    favorite,
    hidden: false,
    portable: true,
    category,
    tags: ["fixture"],
    collectionIds: [],
    status: "ok",
    metadata: { version: null, publisher: "Fixture", description: "Screenshot fixture", productName: name, fileSize: null, modifiedAt: null },
    launchCount: 0,
    lastLaunchedAt: null,
    sortOrder: 0,
    createdAt: "",
    updatedAt: "",
    lastSeenAt: null,
    previewAssetId: null,
  };
}

export const FIXTURE_APPS: AppEntry[] = [
  app("fx-cut", "PulseCut", "ZeroPulse", true),
  app("fx-flow", "PulseFlow", "ZeroPulse", true),
  app("fx-open", "OpenPulse", "ZeroPulse"),
  app("fx-self", "PulseLauncher", "ZeroPulse", true),
  app("fx-cursor", "Cursor", "Development"),
  app("fx-code", "VS Code", "Development"),
  app("fx-git", "GitHub Desktop", "Development"),
  app("fx-docker", "Docker Desktop", "Development", true),
  app("fx-chrome", "Chrome", "Productivity"),
  app("fx-files", "File Explorer", "System"),
  app("fx-term", "Terminal", "System"),
  app("fx-settings", "Settings", "System"),
];

export function fixtureDetails(id: string): AppDetails | null {
  const found = FIXTURE_APPS.find((item) => item.id === id);
  if (!found) return null;
  return { app: found, events: [], previews: [] };
}

const LAUNCHER_SAMPLE = ["fx-cut", "fx-flow", "fx-open", "fx-cursor", "fx-files"];

export function fixtureSearch(query: string): SearchResult[] {
  const q = query.trim().toLowerCase().replace(/^[>@/#=?]+/, "").trim();
  const pool = q ? FIXTURE_APPS : FIXTURE_APPS.filter((item) => LAUNCHER_SAMPLE.includes(item.id));
  const rows = pool.filter((item) => !q || item.name.toLowerCase().includes(q) || (item.category ?? "").toLowerCase().includes(q));
  if (!q) {
    rows.sort((a, b) => LAUNCHER_SAMPLE.indexOf(a.id) - LAUNCHER_SAMPLE.indexOf(b.id));
  }
  return rows.map((item) => ({
    key: `app:${item.id}`,
    kind: "app",
    title: item.name,
    subtitle: `${item.category} · fixture`,
    score: 1,
    highlights: [],
    appId: item.id,
    workflowId: null,
    path: null,
    value: null,
    favorite: item.favorite,
    missing: false,
    iconKey: item.id,
  }));
}

export function fixtureWorkflows(): WorkflowView[] {
  const names = [
    ["copy-date", "Copy Today's Date", "Copies the local date when the stored template is ready."],
    ["open-downloads", "Open Downloads", "Opens the Downloads folder when the OS provides one."],
    ["open-terminal", "Open Terminal", "Needs a terminal path before it can run."],
    ["quick-note", "Timestamped Quick Note", "Writes a note after a folder is chosen."],
    ["check-then-launch", "Check Then Launch", "Asks before launching a path you name."],
  ] as const;
  return names.map(([key, name, what]) => ({
    schema: 1,
    id: `fixture:${key}`,
    name,
    description: what,
    whatItDoes: what,
    prerequisites: "Fixture row. It is not stored and it cannot run.",
    kind: "command",
    templateKey: key,
    templateVersion: 1,
    params: [],
    values: {},
    steps: [],
    hotkey: null,
    trusted: false,
    readiness: { state: "needs_setup", details: ["Fixture screenshot row. It cannot run."] },
    lastRun: null,
  }));
}
