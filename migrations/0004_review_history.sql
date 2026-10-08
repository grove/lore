-- Review state is a projection; observations and transitions are retained.
BEGIN IMMEDIATE;
CREATE TABLE review_context (
 review_id TEXT PRIMARY KEY REFERENCES review_items(id) ON DELETE RESTRICT,
 review_key TEXT NOT NULL,
 category TEXT NOT NULL CHECK(category IN ('relationship','replacement','reaffirmation','conflict','ambiguity','other')),
 assertion_revision_id TEXT REFERENCES assertion_revisions(id) ON DELETE RESTRICT,
 target_unit_id TEXT REFERENCES knowledge_units(id) ON DELETE RESTRICT
);
CREATE INDEX review_by_assertion ON review_context(assertion_revision_id);
CREATE INDEX review_by_target ON review_context(target_unit_id);
CREATE TABLE review_events (
 sequence INTEGER PRIMARY KEY AUTOINCREMENT,
 id TEXT NOT NULL UNIQUE,
 review_id TEXT NOT NULL REFERENCES review_items(id) ON DELETE RESTRICT,
 from_status TEXT CHECK(from_status IN ('pending','resolved','dismissed')),
 to_status TEXT NOT NULL CHECK(to_status IN ('pending','resolved','dismissed')),
 actor_type TEXT NOT NULL CHECK(actor_type IN ('automatic','user','migration')),
 actor TEXT NOT NULL,
 reason_code TEXT NOT NULL,
 note TEXT NOT NULL,
 evidence_id TEXT REFERENCES evidence_snapshots(id) ON DELETE RESTRICT,
 context_digest TEXT NOT NULL,
 recorded_at TEXT NOT NULL
);
CREATE INDEX review_events_by_review ON review_events(review_id,sequence);
CREATE TRIGGER review_event_previous_state BEFORE INSERT ON review_events
WHEN NEW.from_status IS NOT NULL AND NEW.from_status <> (SELECT status FROM review_items WHERE id=NEW.review_id)
BEGIN SELECT RAISE(ABORT,'review status changed concurrently'); END;
CREATE TRIGGER project_review_state AFTER INSERT ON review_events
BEGIN UPDATE review_items SET status=NEW.to_status WHERE id=NEW.review_id; END;
CREATE TRIGGER no_review_event_update BEFORE UPDATE ON review_events
BEGIN SELECT RAISE(ABORT,'immutable review event'); END;
CREATE TRIGGER no_review_event_delete BEFORE DELETE ON review_events
BEGIN SELECT RAISE(ABORT,'immutable review event'); END;
CREATE TRIGGER no_review_context_update BEFORE UPDATE ON review_context
BEGIN SELECT RAISE(ABORT,'immutable review context'); END;
CREATE TRIGGER no_review_context_delete BEFORE DELETE ON review_context
BEGIN SELECT RAISE(ABORT,'immutable review context'); END;
CREATE TRIGGER review_state_requires_event BEFORE UPDATE OF status ON review_items
WHEN NEW.status <> OLD.status AND NOT EXISTS(
 SELECT 1 FROM review_events e WHERE e.review_id=OLD.id
 AND e.sequence=(SELECT max(sequence) FROM review_events WHERE review_id=OLD.id)
 AND e.from_status=OLD.status AND e.to_status=NEW.status
)
BEGIN SELECT RAISE(ABORT,'review disposition requires a history event'); END;
PRAGMA user_version=4;
COMMIT;
