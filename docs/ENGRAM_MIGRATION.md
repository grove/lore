# Native Engram retirement and historical compatibility

Issue [#73](https://github.com/grove/lore/issues/73) retires native Engram intake in the focused development line after 0.9.0-rc.1. Markdown, native OpenWiki and Beads remain supported.

## Compatibility decision

Keep the existing native storage format and `kind: engram` discriminator for historical reads. Do not convert recollections to Markdown, accepted decisions or policy. Registry migration 10 removes only Engram current pointers and current cross-source comparison pointers involving Engram endpoints. Immutable identities, every snapshot/evidence ID, original payloads, recollection/verification/lifecycle labels, native relationship judgments, last import diagnostics, review dispositions and historical cross-source interpretations remain intact. Previously withdrawn records stay withdrawn; migration never recreates current pointers.

Existing Engram import entries are accepted only when the same import ID and Engram kind already belong to this registry's project. They are ignored as inputs, including when their old path is missing, changed or malformed. Remove those entries when convenient (keep at least one supported source root/import in the configuration); retained evidence does not depend on keeping them. A fresh Engram import is rejected with a reviewed-Markdown alternative. No new Engram export is parsed, and changing an old entry's path or project filter cannot refresh history.

## Inventory and upgrade

The durable inventory is `native_imports` (project/import/kind), `native_records` (stable observation IDs), `native_snapshots` (all revisions and evidence IDs), `native_latest`/`native_current` (latest and current projections), and `cross_source_evaluations`/`cross_source_relations` (revision-bound interpretations in either direction). Source-native relationships live inside immutable snapshot `record_json`; their original judgment and activity labels are preserved independently of Lore's current projections. `cross_source_review_context` and review tables preserve review history.

Before upgrading, stop Lore writers and back up `lore.yml`, the complete configured state directory (normally `.lore/`) and any generated wiki together. To count Engram imports, observations and snapshots in your registry without reading source exports:

```sql
SELECT i.project_id, i.id, count(DISTINCT r.id) AS observations,
       count(s.id) AS snapshots
FROM native_imports i
LEFT JOIN native_records r ON r.import_id = i.id
LEFT JOIN native_snapshots s ON s.observation_id = r.id
WHERE i.kind = 'engram'
GROUP BY i.project_id, i.id;

SELECT s.evidence_id, s.id AS snapshot_id, r.id AS observation_id
FROM native_snapshots s JOIN native_records r ON r.id = s.observation_id
JOIN native_imports i ON i.id = r.import_id WHERE i.kind = 'engram';

SELECT e.* FROM cross_source_evaluations e
WHERE (e.from_kind = 'observation' AND e.from_id IN (
  SELECT r.id FROM native_records r JOIN native_imports i ON i.id = r.import_id WHERE i.kind = 'engram'
)) OR (e.to_kind = 'observation' AND e.to_id IN (
  SELECT r.id FROM native_records r JOIN native_imports i ON i.id = r.import_id WHERE i.kind = 'engram'
));
```

Run `lore status`, then `lore update` with the new binary. Update migrates a staged copy and publishes through the existing recoverable journal; a failure before publication leaves the published registry intact. No model is needed for the retirement itself; other changed sources or Markdown page synthesis can still invoke configured models. Run `lore audit` and resolve known `ne_…` IDs with `lore --json evidence ID`. Read-only evidence and context commands do not migrate or write a registry. Before update, older registries retain their last-import status; after migration, Engram evidence is historical (`current: false`). Context qualifies its source as a recollection rather than current implementation or accepted policy. Historical cross-source comparisons remain inspectable through the relationship history API with `active: false`; they are not emitted as current discrepancies.

## Future notes and rollback

For new information, deliberately review a Markdown note and add it under an ordinary configured source root. Label recollections as recollections, proposals as proposals and generated summaries as `material: derived`, with an optional `origin`. Preserve original citations; review alone is not proof of accepted policy. This path creates new documentary evidence IDs and never replaces old native evidence IDs.

Schema 10 is an intentional write compatibility boundary. To roll back, stop writers and restore the complete pre-upgrade backup with the prior binary; do not run an older updater against schema 10 or change `user_version` manually. Restore registry and generated output as a matched pair. Work imported after the backup must be replayed deliberately. Regression tests cover legacy revisions and withdrawal, migration/idempotence, historical reads/audit, failed staged updates and backup restoration.
