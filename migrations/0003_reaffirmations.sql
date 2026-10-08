-- Preserve separately documented reaffirmations without collapsing them into
-- the original decision or rewriting the existing, immutable relation table.
BEGIN IMMEDIATE;
CREATE TABLE reaffirmation_links (
  id TEXT PRIMARY KEY,
  from_unit_id TEXT NOT NULL REFERENCES knowledge_units(id) ON DELETE RESTRICT,
  to_unit_id TEXT NOT NULL REFERENCES knowledge_units(id) ON DELETE RESTRICT,
  assertion_revision_id TEXT NOT NULL REFERENCES assertion_revisions(id) ON DELETE RESTRICT,
  evidence_id TEXT NOT NULL REFERENCES evidence_snapshots(id) ON DELETE RESTRICT,
  created_at TEXT NOT NULL,
  CHECK(from_unit_id <> to_unit_id),
  UNIQUE(from_unit_id,to_unit_id,assertion_revision_id)
);
CREATE INDEX reaffirmation_by_target ON reaffirmation_links(to_unit_id);
CREATE TRIGGER no_reaffirmation_update BEFORE UPDATE ON reaffirmation_links
BEGIN SELECT RAISE(ABORT,'immutable reaffirmation link'); END;
CREATE TRIGGER no_reaffirmation_delete BEFORE DELETE ON reaffirmation_links
BEGIN SELECT RAISE(ABORT,'immutable reaffirmation link'); END;
PRAGMA user_version=3;
COMMIT;
