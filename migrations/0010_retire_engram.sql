-- Freeze Engram history without rewriting identities, payloads or judgments.
BEGIN IMMEDIATE;
DELETE FROM cross_source_current
WHERE input_signature IN (
  SELECT e.input_signature FROM cross_source_evaluations e
  WHERE (e.from_kind = 'observation' AND e.from_id IN (
    SELECT r.id FROM native_records r JOIN native_imports i ON i.id = r.import_id WHERE i.kind = 'engram'
  )) OR (e.to_kind = 'observation' AND e.to_id IN (
    SELECT r.id FROM native_records r JOIN native_imports i ON i.id = r.import_id WHERE i.kind = 'engram'
  ))
);
DELETE FROM native_current WHERE observation_id IN (
  SELECT r.id FROM native_records r JOIN native_imports i ON i.id = r.import_id WHERE i.kind = 'engram'
);
PRAGMA user_version=10;
COMMIT;
