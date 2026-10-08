-- Lore SQLite schema v1: immutable source observations, revisable understanding.
-- Open every database connection with foreign_keys enabled.
PRAGMA foreign_keys=ON;
BEGIN IMMEDIATE;

CREATE TABLE projects (
  id TEXT PRIMARY KEY, name TEXT NOT NULL, created_at TEXT NOT NULL
);
CREATE TABLE source_roots (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE RESTRICT,
  configured_path TEXT NOT NULL
);
CREATE TABLE sources (
  id TEXT PRIMARY KEY,
  root_id TEXT NOT NULL REFERENCES source_roots(id) ON DELETE RESTRICT,
  relative_path TEXT NOT NULL,
  removed_at TEXT
);
CREATE UNIQUE INDEX unique_active_path ON sources(root_id,relative_path)
  WHERE removed_at IS NULL;

CREATE TABLE source_revisions (
  id TEXT PRIMARY KEY,
  source_id TEXT NOT NULL REFERENCES sources(id) ON DELETE RESTRICT,
  observed_path TEXT NOT NULL,
  content_digest TEXT NOT NULL,
  observed_at TEXT NOT NULL,
  UNIQUE(id,source_id)
);
CREATE TABLE source_sections (
  id TEXT PRIMARY KEY,
  source_id TEXT NOT NULL REFERENCES sources(id) ON DELETE RESTRICT
);
CREATE TABLE section_revisions (
  id TEXT PRIMARY KEY,
  section_id TEXT NOT NULL REFERENCES source_sections(id) ON DELETE RESTRICT,
  source_revision_id TEXT NOT NULL REFERENCES source_revisions(id) ON DELETE RESTRICT,
  heading_path_json TEXT NOT NULL,
  content_digest TEXT NOT NULL,
  line_start INTEGER,
  line_end INTEGER,
  CHECK((line_start IS NULL AND line_end IS NULL) OR
       (line_start IS NOT NULL AND line_end IS NOT NULL AND line_start>0 AND line_end>=line_start))
);
CREATE TABLE evidence_snapshots (
  id TEXT PRIMARY KEY,
  source_id TEXT NOT NULL,
  source_revision_id TEXT NOT NULL,
  section_revision_id TEXT REFERENCES section_revisions(id) ON DELETE RESTRICT,
  exact_excerpt TEXT NOT NULL CHECK(length(exact_excerpt)>0),
  context_before TEXT NOT NULL DEFAULT '',
  context_after TEXT NOT NULL DEFAULT '',
  excerpt_digest TEXT NOT NULL,
  line_start INTEGER,
  line_end INTEGER,
  captured_at TEXT NOT NULL,
  FOREIGN KEY(source_revision_id,source_id)
    REFERENCES source_revisions(id,source_id) ON DELETE RESTRICT,
  CHECK((line_start IS NULL AND line_end IS NULL) OR
        (line_start IS NOT NULL AND line_end IS NOT NULL AND line_start>0 AND line_end>=line_start)),
  UNIQUE(id,source_id,source_revision_id)
);

CREATE TABLE source_assertions (
  id TEXT PRIMARY KEY,
  source_id TEXT NOT NULL REFERENCES sources(id) ON DELETE RESTRICT,
  created_at TEXT NOT NULL,
  UNIQUE(id,source_id)
);
CREATE TABLE assertion_revisions (
  id TEXT PRIMARY KEY,
  assertion_id TEXT NOT NULL,
  source_id TEXT NOT NULL,
  source_revision_id TEXT NOT NULL,
  revision_number INTEGER NOT NULL CHECK(revision_number>0),
  statement TEXT NOT NULL CHECK(length(trim(statement))>0),
  modality TEXT NOT NULL,
  effective_at TEXT,
  observed_at TEXT NOT NULL,
  model_id TEXT NOT NULL,
  prompt_version TEXT NOT NULL,
  FOREIGN KEY(assertion_id,source_id)
    REFERENCES source_assertions(id,source_id) ON DELETE RESTRICT,
  FOREIGN KEY(source_revision_id,source_id)
    REFERENCES source_revisions(id,source_id) ON DELETE RESTRICT,
  UNIQUE(assertion_id,revision_number),
  UNIQUE(id,source_id,source_revision_id)
);
CREATE TABLE assertion_evidence (
  assertion_revision_id TEXT NOT NULL,
  evidence_id TEXT NOT NULL,
  source_id TEXT NOT NULL,
  source_revision_id TEXT NOT NULL,
  PRIMARY KEY(assertion_revision_id,evidence_id),
  FOREIGN KEY(assertion_revision_id,source_id,source_revision_id)
    REFERENCES assertion_revisions(id,source_id,source_revision_id) ON DELETE RESTRICT,
  FOREIGN KEY(evidence_id,source_id,source_revision_id)
    REFERENCES evidence_snapshots(id,source_id,source_revision_id) ON DELETE RESTRICT
);

CREATE TABLE knowledge_units (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE RESTRICT,
  created_at TEXT NOT NULL
);
CREATE TABLE knowledge_revisions (
  id TEXT PRIMARY KEY,
  knowledge_id TEXT NOT NULL REFERENCES knowledge_units(id) ON DELETE RESTRICT,
  revision_number INTEGER NOT NULL CHECK(revision_number>0),
  statement TEXT NOT NULL CHECK(length(trim(statement))>0),
  kind TEXT NOT NULL,
  lifecycle TEXT NOT NULL DEFAULT 'unknown',
  support_state TEXT NOT NULL CHECK(support_state IN
    ('current_documentary_support','historical_only','unsupported','needs_review')),
  temporal_scope TEXT NOT NULL DEFAULT 'unknown',
  created_at TEXT NOT NULL,
  UNIQUE(knowledge_id,revision_number)
);
CREATE TABLE knowledge_support (
  knowledge_revision_id TEXT NOT NULL REFERENCES knowledge_revisions(id) ON DELETE RESTRICT,
  assertion_revision_id TEXT NOT NULL REFERENCES assertion_revisions(id) ON DELETE RESTRICT,
  relation TEXT NOT NULL CHECK(relation IN ('supports','challenges','context')),
  PRIMARY KEY(knowledge_revision_id,assertion_revision_id,relation)
);
CREATE TABLE knowledge_relations (
  id TEXT PRIMARY KEY,
  from_revision_id TEXT NOT NULL REFERENCES knowledge_revisions(id) ON DELETE RESTRICT,
  to_revision_id TEXT NOT NULL REFERENCES knowledge_revisions(id) ON DELETE RESTRICT,
  relation TEXT NOT NULL CHECK(relation IN
    ('elaborates','contradicts','suspected_conflict','supersedes','related_to')),
  evidence_id TEXT REFERENCES evidence_snapshots(id) ON DELETE RESTRICT,
  CHECK(from_revision_id<>to_revision_id),
  CHECK(relation<>'supersedes' OR evidence_id IS NOT NULL)
);

CREATE TABLE topics (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE RESTRICT,
  slug TEXT NOT NULL,
  title TEXT NOT NULL,
  UNIQUE(project_id,slug)
);
CREATE TABLE topic_units (
  topic_id TEXT NOT NULL REFERENCES topics(id) ON DELETE RESTRICT,
  knowledge_id TEXT NOT NULL REFERENCES knowledge_units(id) ON DELETE RESTRICT,
  PRIMARY KEY(topic_id,knowledge_id)
);
CREATE TABLE review_items (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE RESTRICT,
  reason TEXT NOT NULL,
  status TEXT NOT NULL CHECK(status IN ('pending','resolved','dismissed'))
);
CREATE TABLE runs (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE RESTRICT,
  phase TEXT NOT NULL,
  source_inventory_digest TEXT,
  started_at TEXT NOT NULL,
  finished_at TEXT
);
CREATE TABLE publications (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL UNIQUE REFERENCES runs(id) ON DELETE RESTRICT,
  wiki_digest TEXT NOT NULL,
  published_at TEXT NOT NULL
);

-- Append-only historical data. A future explicit privacy purge requires a
-- separate audited workflow, not ordinary UPDATE or DELETE statements.
CREATE TRIGGER forbid_snapshot_update BEFORE UPDATE ON evidence_snapshots
  BEGIN SELECT RAISE(ABORT,'immutable evidence'); END;
CREATE TRIGGER forbid_snapshot_delete BEFORE DELETE ON evidence_snapshots
  BEGIN SELECT RAISE(ABORT,'immutable evidence'); END;
CREATE TRIGGER forbid_source_revision_update BEFORE UPDATE ON source_revisions
  BEGIN SELECT RAISE(ABORT,'immutable source revision'); END;
CREATE TRIGGER forbid_source_revision_delete BEFORE DELETE ON source_revisions
  BEGIN SELECT RAISE(ABORT,'immutable source revision'); END;
CREATE TRIGGER forbid_assertion_revision_update BEFORE UPDATE ON assertion_revisions
  BEGIN SELECT RAISE(ABORT,'immutable assertion revision'); END;
CREATE TRIGGER forbid_assertion_revision_delete BEFORE DELETE ON assertion_revisions
  BEGIN SELECT RAISE(ABORT,'immutable assertion revision'); END;
CREATE TRIGGER forbid_knowledge_revision_update BEFORE UPDATE ON knowledge_revisions
  BEGIN SELECT RAISE(ABORT,'immutable knowledge revision'); END;
CREATE TRIGGER forbid_knowledge_revision_delete BEFORE DELETE ON knowledge_revisions
  BEGIN SELECT RAISE(ABORT,'immutable knowledge revision'); END;

PRAGMA user_version=1;
COMMIT;
