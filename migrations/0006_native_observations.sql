-- Native payloads and their normalized interpretation are immutable snapshots.
-- Current pointers are projections and can be withdrawn without losing history.
BEGIN IMMEDIATE;
CREATE TABLE native_imports (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE RESTRICT,
  kind TEXT NOT NULL CHECK(kind IN ('openwiki','engram','beads'))
);
CREATE TABLE native_import_state (
  import_id TEXT PRIMARY KEY REFERENCES native_imports(id) ON DELETE RESTRICT,
  configured_path TEXT NOT NULL,
  input_digest TEXT NOT NULL,
  warnings_json TEXT NOT NULL
);
CREATE TABLE native_records (
  id TEXT PRIMARY KEY,
  import_id TEXT NOT NULL REFERENCES native_imports(id) ON DELETE RESTRICT,
  native_id TEXT NOT NULL,
  UNIQUE(import_id,native_id)
);
CREATE TABLE native_snapshots (
  id TEXT PRIMARY KEY,
  observation_id TEXT NOT NULL REFERENCES native_records(id) ON DELETE RESTRICT,
  evidence_id TEXT NOT NULL UNIQUE,
  content_hash TEXT NOT NULL,
  normalized_hash TEXT NOT NULL,
  source_path TEXT NOT NULL,
  format TEXT NOT NULL,
  record_json TEXT NOT NULL,
  captured_at TEXT NOT NULL,
  UNIQUE(id,observation_id)
);
CREATE INDEX native_snapshots_by_record ON native_snapshots(observation_id,captured_at);
CREATE TABLE native_latest (
  observation_id TEXT PRIMARY KEY REFERENCES native_records(id) ON DELETE RESTRICT,
  snapshot_id TEXT NOT NULL REFERENCES native_snapshots(id) ON DELETE RESTRICT,
  FOREIGN KEY(snapshot_id,observation_id) REFERENCES native_snapshots(id,observation_id) ON DELETE RESTRICT
);
CREATE TABLE native_current (
  observation_id TEXT PRIMARY KEY REFERENCES native_records(id) ON DELETE RESTRICT,
  snapshot_id TEXT NOT NULL REFERENCES native_snapshots(id) ON DELETE RESTRICT,
  FOREIGN KEY(snapshot_id,observation_id) REFERENCES native_snapshots(id,observation_id) ON DELETE RESTRICT
);
CREATE VIRTUAL TABLE native_fts USING fts5(observation_id UNINDEXED,native_id,title,subject,statement,tags,links,component);
CREATE TRIGGER no_native_import_update BEFORE UPDATE ON native_imports
BEGIN SELECT RAISE(ABORT,'immutable native import identity'); END;
CREATE TRIGGER no_native_import_delete BEFORE DELETE ON native_imports
BEGIN SELECT RAISE(ABORT,'immutable native import identity'); END;
CREATE TRIGGER no_native_record_update BEFORE UPDATE ON native_records
BEGIN SELECT RAISE(ABORT,'immutable native record identity'); END;
CREATE TRIGGER no_native_record_delete BEFORE DELETE ON native_records
BEGIN SELECT RAISE(ABORT,'immutable native record identity'); END;
CREATE TRIGGER no_native_snapshot_update BEFORE UPDATE ON native_snapshots
BEGIN SELECT RAISE(ABORT,'immutable native observation snapshot'); END;
CREATE TRIGGER no_native_snapshot_delete BEFORE DELETE ON native_snapshots
BEGIN SELECT RAISE(ABORT,'immutable native observation snapshot'); END;
PRAGMA user_version=6;
COMMIT;
