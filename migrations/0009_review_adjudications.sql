BEGIN IMMEDIATE;
CREATE TABLE review_adjudication_events (
 sequence INTEGER PRIMARY KEY AUTOINCREMENT,
 id TEXT NOT NULL UNIQUE,
 review_id TEXT NOT NULL REFERENCES review_items(id),
 event_kind TEXT NOT NULL CHECK(event_kind IN ('applied','invalidated')),
 payload_json TEXT NOT NULL CHECK(json_valid(payload_json))
);
CREATE INDEX adjudications_by_review ON review_adjudication_events(review_id,sequence);
CREATE VIEW review_adjudications AS
 SELECT e.* FROM review_adjudication_events e
 WHERE e.sequence=(SELECT max(sequence) FROM review_adjudication_events WHERE review_id=e.review_id)
 AND e.event_kind='applied';
CREATE TRIGGER no_adjudication_update BEFORE UPDATE ON review_adjudication_events
BEGIN SELECT RAISE(ABORT,'immutable adjudication event'); END;
CREATE TRIGGER no_adjudication_delete BEFORE DELETE ON review_adjudication_events
BEGIN SELECT RAISE(ABORT,'immutable adjudication event'); END;
PRAGMA user_version=9;
COMMIT;
