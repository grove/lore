-- Runtime projections are mutable; source observations and semantic history are not.
BEGIN IMMEDIATE;
CREATE TABLE lore_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE source_current (
  source_id TEXT PRIMARY KEY REFERENCES sources(id),
  source_revision_id TEXT NOT NULL REFERENCES source_revisions(id),
  content_digest TEXT NOT NULL
);
CREATE TABLE current_sections (
  section_id TEXT PRIMARY KEY REFERENCES source_sections(id),
  source_id TEXT NOT NULL REFERENCES sources(id),
  section_key TEXT NOT NULL,
  section_revision_id TEXT NOT NULL REFERENCES section_revisions(id),
  input_digest TEXT NOT NULL,
  UNIQUE(source_id,section_key)
);
CREATE TABLE active_assertions (
  assertion_revision_id TEXT PRIMARY KEY REFERENCES assertion_revisions(id),
  section_id TEXT NOT NULL REFERENCES source_sections(id)
);
CREATE INDEX active_by_section ON active_assertions(section_id);
CREATE TABLE assertion_details (
  assertion_revision_id TEXT PRIMARY KEY REFERENCES assertion_revisions(id),
  proposal_json TEXT NOT NULL,
  lineage_key TEXT NOT NULL
);
CREATE INDEX assertion_lineage ON assertion_details(lineage_key);
CREATE TABLE knowledge_details (
  knowledge_id TEXT PRIMARY KEY REFERENCES knowledge_units(id),
  topic_id TEXT NOT NULL REFERENCES topics(id),
  subject TEXT NOT NULL,
  scope TEXT NOT NULL,
  effective_at TEXT NOT NULL,
  base_lifecycle TEXT NOT NULL
);
CREATE TABLE assertion_assignments (
  assertion_revision_id TEXT PRIMARY KEY REFERENCES assertion_revisions(id),
  knowledge_id TEXT NOT NULL REFERENCES knowledge_units(id)
);
CREATE INDEX assignments_by_unit ON assertion_assignments(knowledge_id);
CREATE TABLE knowledge_current (
  knowledge_id TEXT PRIMARY KEY REFERENCES knowledge_units(id),
  revision_id TEXT NOT NULL UNIQUE REFERENCES knowledge_revisions(id),
  input_digest TEXT NOT NULL
);
CREATE TABLE relation_assertions (
  relation_id TEXT PRIMARY KEY REFERENCES knowledge_relations(id),
  assertion_revision_id TEXT NOT NULL REFERENCES assertion_revisions(id)
);
CREATE TABLE wiki_pages (
  path TEXT PRIMARY KEY,
  input_digest TEXT NOT NULL,
  output_digest TEXT NOT NULL,
  content TEXT NOT NULL
);
CREATE TABLE model_calls (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES runs(id),
  task TEXT NOT NULL,
  provider TEXT NOT NULL,
  model TEXT NOT NULL,
  input_digest TEXT NOT NULL,
  cache_hit INTEGER NOT NULL CHECK(cache_hit IN (0,1)),
  duration_ms INTEGER NOT NULL
);
CREATE TABLE reconciliation_log (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES runs(id),
  assertion_revision_id TEXT NOT NULL REFERENCES assertion_revisions(id),
  operation_json TEXT NOT NULL
);
CREATE VIRTUAL TABLE knowledge_fts USING fts5(knowledge_id UNINDEXED, statement, subject, topic);
CREATE INDEX support_by_assertion ON knowledge_support(assertion_revision_id);
CREATE INDEX support_by_unit ON knowledge_support(knowledge_revision_id);
CREATE INDEX evidence_by_source ON evidence_snapshots(source_id);
CREATE INDEX relations_to ON knowledge_relations(to_revision_id);
CREATE INDEX relations_from ON knowledge_relations(from_revision_id);
CREATE TABLE sealed_assertions (id TEXT PRIMARY KEY REFERENCES assertion_revisions(id));
CREATE TABLE sealed_knowledge (id TEXT PRIMARY KEY REFERENCES knowledge_revisions(id));
INSERT INTO sealed_assertions SELECT id FROM assertion_revisions;
INSERT INTO sealed_knowledge SELECT id FROM knowledge_revisions;

CREATE TRIGGER section_source_integrity BEFORE INSERT ON section_revisions
WHEN (SELECT source_id FROM source_sections WHERE id=NEW.section_id) <>
     (SELECT source_id FROM source_revisions WHERE id=NEW.source_revision_id)
BEGIN SELECT RAISE(ABORT,'section belongs to a different source'); END;
CREATE TRIGGER snapshot_section_integrity BEFORE INSERT ON evidence_snapshots
WHEN NEW.section_revision_id IS NOT NULL AND
     NEW.source_revision_id <> (SELECT source_revision_id FROM section_revisions WHERE id=NEW.section_revision_id)
BEGIN SELECT RAISE(ABORT,'snapshot belongs to a different source revision'); END;
CREATE TRIGGER active_assertion_integrity BEFORE INSERT ON active_assertions
WHEN (SELECT source_id FROM source_sections WHERE id=NEW.section_id) <>
     (SELECT source_id FROM assertion_revisions WHERE id=NEW.assertion_revision_id)
BEGIN SELECT RAISE(ABORT,'active assertion belongs to a different source'); END;
CREATE TRIGGER assignment_project_integrity BEFORE INSERT ON assertion_assignments
WHEN (SELECT project_id FROM knowledge_units WHERE id=NEW.knowledge_id) <>
     (SELECT r.project_id FROM assertion_revisions a JOIN sources s ON s.id=a.source_id JOIN source_roots r ON r.id=s.root_id WHERE a.id=NEW.assertion_revision_id)
BEGIN SELECT RAISE(ABORT,'cross-project assignment'); END;
CREATE TRIGGER sealed_assertion_evidence BEFORE INSERT ON assertion_evidence
WHEN EXISTS(SELECT 1 FROM sealed_assertions WHERE id=NEW.assertion_revision_id)
BEGIN SELECT RAISE(ABORT,'assertion revision is sealed'); END;
CREATE TRIGGER sealed_unit_support BEFORE INSERT ON knowledge_support
WHEN EXISTS(SELECT 1 FROM sealed_knowledge WHERE id=NEW.knowledge_revision_id)
BEGIN SELECT RAISE(ABORT,'knowledge revision is sealed'); END;
CREATE TRIGGER no_section_revision_update BEFORE UPDATE ON section_revisions
BEGIN SELECT RAISE(ABORT,'immutable section revision'); END;
CREATE TRIGGER no_section_revision_delete BEFORE DELETE ON section_revisions
BEGIN SELECT RAISE(ABORT,'immutable section revision'); END;
CREATE TRIGGER no_assertion_details_update BEFORE UPDATE ON assertion_details
BEGIN SELECT RAISE(ABORT,'immutable assertion details'); END;
CREATE TRIGGER no_assertion_details_delete BEFORE DELETE ON assertion_details
BEGIN SELECT RAISE(ABORT,'immutable assertion details'); END;
CREATE TRIGGER no_assertion_evidence_update BEFORE UPDATE ON assertion_evidence
BEGIN SELECT RAISE(ABORT,'immutable assertion evidence'); END;
CREATE TRIGGER no_assertion_evidence_delete BEFORE DELETE ON assertion_evidence
BEGIN SELECT RAISE(ABORT,'immutable assertion evidence'); END;
CREATE TRIGGER no_support_update BEFORE UPDATE ON knowledge_support
BEGIN SELECT RAISE(ABORT,'immutable support history'); END;
CREATE TRIGGER no_support_delete BEFORE DELETE ON knowledge_support
BEGIN SELECT RAISE(ABORT,'immutable support history'); END;
CREATE TRIGGER no_relation_update BEFORE UPDATE ON knowledge_relations
BEGIN SELECT RAISE(ABORT,'immutable relationship'); END;
CREATE TRIGGER no_relation_delete BEFORE DELETE ON knowledge_relations
BEGIN SELECT RAISE(ABORT,'immutable relationship'); END;
CREATE TRIGGER no_assignment_update BEFORE UPDATE ON assertion_assignments
BEGIN SELECT RAISE(ABORT,'immutable assignment history'); END;
CREATE TRIGGER no_assignment_delete BEFORE DELETE ON assertion_assignments
BEGIN SELECT RAISE(ABORT,'immutable assignment history'); END;
PRAGMA user_version=2;
COMMIT;
