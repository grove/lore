-- Additive accounting: never manufacture tokens or bills for historical calls.
-- A run has measured coverage only when provider_usage_runs has an entry.
BEGIN IMMEDIATE;
CREATE TABLE provider_usage_runs (
  run_id TEXT PRIMARY KEY REFERENCES runs(id),
  summary_json TEXT NOT NULL CHECK(json_valid(summary_json))
);
CREATE TABLE provider_usage_events (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES provider_usage_runs(run_id),
  ordinal INTEGER NOT NULL CHECK(ordinal >= 0),
  event_json TEXT NOT NULL CHECK(json_valid(event_json)),
  UNIQUE(run_id,ordinal)
);
CREATE INDEX provider_usage_by_run ON provider_usage_events(run_id,ordinal);
PRAGMA user_version=8;
COMMIT;
