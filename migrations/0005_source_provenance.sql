-- Provenance belongs to the observed source revision. Changing an import's
-- declared material or origin must never rewrite old evidence qualifications.
BEGIN IMMEDIATE;
CREATE TABLE source_revision_provenance (
  source_revision_id TEXT PRIMARY KEY REFERENCES source_revisions(id) ON DELETE RESTRICT,
  root_id TEXT NOT NULL REFERENCES source_roots(id) ON DELETE RESTRICT,
  configured_path TEXT NOT NULL,
  material TEXT NOT NULL CHECK(material IN ('primary','derived')),
  origin TEXT CHECK(origin IS NULL OR (length(trim(origin))>0 AND length(origin)<=2048))
);
-- Legacy revisions intentionally have no row: their material defaults to
-- primary, while an unrecorded historical root path/origin stays unknown.
CREATE TRIGGER provenance_source_integrity BEFORE INSERT ON source_revision_provenance
WHEN NEW.root_id <> (
  SELECT s.root_id FROM source_revisions r JOIN sources s ON s.id=r.source_id
  WHERE r.id=NEW.source_revision_id
)
BEGIN SELECT RAISE(ABORT,'provenance belongs to a different source root'); END;
CREATE TRIGGER provenance_must_precede_evidence BEFORE INSERT ON source_revision_provenance
WHEN EXISTS(SELECT 1 FROM evidence_snapshots WHERE source_revision_id=NEW.source_revision_id)
  OR EXISTS(SELECT 1 FROM source_revision_provenance WHERE source_revision_id=NEW.source_revision_id)
BEGIN SELECT RAISE(ABORT,'source provenance must be captured before evidence and only once'); END;
CREATE TRIGGER no_source_provenance_update BEFORE UPDATE ON source_revision_provenance
BEGIN SELECT RAISE(ABORT,'immutable source provenance'); END;
CREATE TRIGGER no_source_provenance_delete BEFORE DELETE ON source_revision_provenance
BEGIN SELECT RAISE(ABORT,'immutable source provenance'); END;
PRAGMA user_version=5;
COMMIT;
