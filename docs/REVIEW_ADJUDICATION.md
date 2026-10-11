# Evidence-bound review adjudication

`review resolve`, `dismiss` and `reopen` remain status-only. Use adjudication to record an interpretation of one typed candidate. Run `lore update` first, then preview:

```sh
lore --json review adjudicate REVIEW_ID --decision keep_separate \
  --reason 'The scopes differ' --actor reviewer --actor-kind user
```

Repeat with `--apply --expect-context-digest DIGEST` using the preview's exact digest. `reject_relation` also dismisses only this candidate. `accept_relation` requires `--relation supersedes|reaffirms|contradicts` and `--evidence EVIDENCE_ID`. The excerpt must be active and from the candidate's exact source revision. Ambiguous generic questions, mismatched categories, stale source/output state and concurrent review changes fail closed. Actor strings are attribution, not authenticated identity; agent attribution is supported explicitly.

Acceptance confirms authority only when the existing validated documentary edge belongs to this exact assertion, evidence, target and relation. Otherwise it records a **human advisory interpretation**. This conservative implementation reuses documentary validation and never constructs new authoritative edges, merges units, or rewrites source history. An opinion cannot make a proposal into an accepted ADR. Approval of a new documentary decision belongs in an authorized source document followed by ordinary update.

Events are append-only in schema 9; a SQL view rebuilds current interpretations. Context, decisions and cases carry bounded `manual_adjudication` qualifications on the affected units. Review pages and audit expose attribution, evidence, scope and authority. Review dismissal suppresses the exact review question; it does not hide documentary conflict. Existing source citations, permission ceilings and output budgets still apply. No model or network calls are made by this operation.

The digest binds documentary registry/source fingerprints, candidate/target revisions and review history. Update invalidates interpretations on changed documentary state and reopens the review, retaining history. Reverting the source bytes does not resurrect a prior interpretation. The full documentary fingerprint is deliberately conservative: unrelated documentary changes also require a new preview. Narrowing caller inspection permissions is not a documentary change.

Preview is read-only. Apply uses the existing lock, staged database, transactional publication and recovery mechanism; output edits or changed sources are never overwritten silently. Identical immediate retries do not append events. `lore update` migrates existing schema-8 projects even when sources are unchanged; read-only commands do not migrate them. Source schema and evidence identifiers remain unchanged.
