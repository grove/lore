-- Lore interpretations of external records have independent, immutable history.
-- A current pointer describes the input revisions considered by Lore; it never
-- upgrades an upstream verification to a check of the working tree.
BEGIN IMMEDIATE;
CREATE TABLE cross_source_evaluations (
 input_signature TEXT PRIMARY KEY,
 pair_key TEXT NOT NULL,
 from_kind TEXT NOT NULL CHECK(from_kind IN ('observation','knowledge')),
 from_id TEXT NOT NULL,
 from_revision_id TEXT NOT NULL,
 to_kind TEXT CHECK(to_kind IN ('observation','knowledge')),
 to_id TEXT,
 to_revision_id TEXT,
 disposition TEXT NOT NULL,
 recorded_at TEXT NOT NULL,
 UNIQUE(input_signature,pair_key),
 CHECK((to_kind IS NULL AND to_id IS NULL AND to_revision_id IS NULL) OR
       (to_kind IS NOT NULL AND to_id IS NOT NULL AND to_revision_id IS NOT NULL))
);
CREATE INDEX cross_source_by_from ON cross_source_evaluations(from_id,from_revision_id);
CREATE INDEX cross_source_by_to ON cross_source_evaluations(to_id,to_revision_id);
CREATE TABLE cross_source_relations (
 id TEXT PRIMARY KEY,
 input_signature TEXT NOT NULL REFERENCES cross_source_evaluations(input_signature) ON DELETE RESTRICT,
 payload_json TEXT NOT NULL CHECK(json_valid(payload_json))
);
CREATE INDEX cross_source_relation_inputs ON cross_source_relations(input_signature);
CREATE TABLE cross_source_current (
 pair_key TEXT PRIMARY KEY,
 input_signature TEXT NOT NULL,
 FOREIGN KEY(input_signature,pair_key)
   REFERENCES cross_source_evaluations(input_signature,pair_key) ON DELETE RESTRICT
);
CREATE TABLE cross_source_review_context (
 review_id TEXT PRIMARY KEY REFERENCES review_items(id) ON DELETE RESTRICT,
 pair_key TEXT NOT NULL
);
CREATE TABLE cross_source_state (
 key TEXT PRIMARY KEY,
 value TEXT NOT NULL
);
CREATE TRIGGER no_cross_source_evaluation_update BEFORE UPDATE ON cross_source_evaluations
BEGIN SELECT RAISE(ABORT,'immutable cross-source interpretation'); END;
CREATE TRIGGER no_cross_source_evaluation_delete BEFORE DELETE ON cross_source_evaluations
BEGIN SELECT RAISE(ABORT,'immutable cross-source interpretation'); END;
CREATE TRIGGER no_cross_source_relation_update BEFORE UPDATE ON cross_source_relations
BEGIN SELECT RAISE(ABORT,'immutable cross-source relationship'); END;
CREATE TRIGGER no_cross_source_relation_delete BEFORE DELETE ON cross_source_relations
BEGIN SELECT RAISE(ABORT,'immutable cross-source relationship'); END;
CREATE TRIGGER no_cross_source_review_context_update BEFORE UPDATE ON cross_source_review_context
BEGIN SELECT RAISE(ABORT,'immutable cross-source review context'); END;
CREATE TRIGGER no_cross_source_review_context_delete BEFORE DELETE ON cross_source_review_context
BEGIN SELECT RAISE(ABORT,'immutable cross-source review context'); END;
PRAGMA user_version=7;
COMMIT;
