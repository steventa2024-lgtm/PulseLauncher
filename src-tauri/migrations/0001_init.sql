-- PulseLauncher schema v1
CREATE TABLE apps (
  id               TEXT PRIMARY KEY,
  identity_key     TEXT NOT NULL UNIQUE,
  name             TEXT NOT NULL,
  custom_name      TEXT,
  kind             TEXT NOT NULL,
  source           TEXT NOT NULL,
  launch_target    TEXT NOT NULL,
  resolved_target  TEXT,
  arguments        TEXT NOT NULL DEFAULT '[]',
  link_arguments   TEXT,
  working_dir      TEXT,
  icon_path        TEXT,
  icon_index       INTEGER NOT NULL DEFAULT 0,
  favorite         INTEGER NOT NULL DEFAULT 0,
  hidden           INTEGER NOT NULL DEFAULT 0,
  portable         INTEGER NOT NULL DEFAULT 0,
  category         TEXT,
  status           TEXT NOT NULL DEFAULT 'unknown',
  sort_order       INTEGER NOT NULL DEFAULT 0,
  preview_asset_id TEXT,
  created_at       TEXT NOT NULL,
  updated_at       TEXT NOT NULL,
  last_seen_at     TEXT
);
CREATE INDEX idx_apps_source ON apps(source);

CREATE TABLE app_tags (
  app_id TEXT NOT NULL REFERENCES apps(id) ON DELETE CASCADE,
  tag    TEXT NOT NULL,
  PRIMARY KEY (app_id, tag)
);

CREATE TABLE collections (
  id         TEXT PRIMARY KEY,
  name       TEXT NOT NULL,
  system_key TEXT UNIQUE,
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL
);

CREATE TABLE collection_items (
  collection_id TEXT NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
  app_id        TEXT NOT NULL REFERENCES apps(id) ON DELETE CASCADE,
  sort_order    INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (collection_id, app_id)
);

CREATE TABLE launch_events (
  id          INTEGER PRIMARY KEY AUTOINCREMENT,
  app_id      TEXT REFERENCES apps(id) ON DELETE SET NULL,
  workflow_id TEXT,
  label       TEXT NOT NULL,
  at          TEXT NOT NULL,
  success     INTEGER NOT NULL,
  process_id  INTEGER,
  message     TEXT NOT NULL DEFAULT ''
);
CREATE INDEX idx_launch_events_app ON launch_events(app_id, at);

CREATE TABLE workflows (
  id               TEXT PRIMARY KEY,
  kind             TEXT NOT NULL DEFAULT 'workflow',
  name             TEXT NOT NULL,
  definition       TEXT NOT NULL,
  template_key     TEXT UNIQUE,
  template_version INTEGER,
  trusted          INTEGER NOT NULL DEFAULT 1,
  hotkey           TEXT,
  created_at       TEXT NOT NULL,
  updated_at       TEXT NOT NULL
);

CREATE TABLE workflow_runs (
  id          TEXT PRIMARY KEY,
  workflow_id TEXT NOT NULL,
  name        TEXT NOT NULL,
  status      TEXT NOT NULL,
  started_at  TEXT NOT NULL,
  finished_at TEXT,
  steps       TEXT NOT NULL DEFAULT '[]',
  error       TEXT
);
CREATE INDEX idx_workflow_runs_wf ON workflow_runs(workflow_id, started_at);

CREATE TABLE preferences (
  id    INTEGER PRIMARY KEY CHECK (id = 1),
  value TEXT NOT NULL
);

CREATE TABLE file_index (
  path   TEXT PRIMARY KEY,
  name   TEXT NOT NULL,
  is_dir INTEGER NOT NULL,
  root   TEXT NOT NULL
);

CREATE TABLE preview_assets (
  id         TEXT PRIMARY KEY,
  app_id     TEXT NOT NULL REFERENCES apps(id) ON DELETE CASCADE,
  file_path  TEXT NOT NULL,
  source     TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE TABLE scans (
  id          INTEGER PRIMARY KEY AUTOINCREMENT,
  started_at  TEXT NOT NULL,
  finished_at TEXT NOT NULL,
  report      TEXT NOT NULL
);
