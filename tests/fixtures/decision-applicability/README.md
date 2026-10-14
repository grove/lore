# Decision applicability regression matrix

`matrix.json` supplies the original documentary passages to
`tests/decision_applicability.rs`. Each `before` / `after` tuple is
`[exact passage, kind, original lifecycle, scope]`, captured in `ADR-001.md` /
`ADR-002.md` respectively. The fixture model deliberately suggests the relation
in `suggested`; the existing documentary guards must produce `relation` and
`old_lifecycle`. A filename alone does not confer acceptance on the second record.

| Case | Expected current decision and applicability | Historical status | Contradiction |
| --- | --- | --- | --- |
| Production / staging | Signature required in production; isolated staging replay may omit it. | Both passages have current support in separate scopes. | Inapplicable across environments. |
| Component A / B | Signature required for A; B may omit it. | Both passages have current support in separate component scopes. | Inapplicable across components. |
| Accepted ADR / later proposal | Original signature decision remains accepted; proposed replacement remains proposed even with explicit replacement wording. | Original acceptance and both current passages survive. | No accepted-policy contradiction or supersession. |
| Closed work | Signature decision remains accepted; completion is a reported outcome. | Both passages survive with distinct source roles. | Completion cannot establish replacement policy. |
| Rejected approach / new issue | Unsigned releases remain rejected; the new issue remains open work, without an accepted replacement. | Rejection rationale remains current negative knowledge. | An issue does not contradict an accepted decision or erase rejection. |
| Grounded replacement | Two signatures replace one, with an exact predecessor-naming witness. | Original decision becomes superseded while its original acceptance and source support remain inspectable. | Explicit supersession, not a competing current contradiction. |
| Recency only | Both accepted passages remain separate; inspect the missing replacement evidence. | A newer source does not rewrite the original lifecycle. | Unresolved replacement review, not fabricated supersession. |
| Same-scope accepted constraints | Preserve both incompatible signature requirements and their conflict review. | Both currently supported passages remain inspectable. | Actual documentary contradiction; runtime behavior is unverified. |

The adjacent paths exercise native evidence and sequential changes without a
second fixture knowledge model:

| Test / original source passages | Expected current decision, scope and exception | History / authority | Contradiction |
| --- | --- | --- | --- |
| `a_b_a_keeps_distinct_history_and_requires_explicit_replacement`: “MySQL is the selected database.” → “ADR-002 replaces ADR-001: PostgreSQL is the selected database.” → the original MySQL passage, or “ADR-003 supersedes ADR-002: MySQL is the selected database.” | All are production decisions. A return to A has a distinct identity; B is superseded only in the explicit variant. | A's original event remains superseded; all source revisions and replacement witnesses survive. | Repeated A wording does not prove replacement of B. |
| `engine::cross_source::tests::applicability_matrix_keeps_scope_authority_and_exact_revisions`: accepted “The worker concurrency limit is three.” versus OpenWiki “The worker concurrency limit is five.” | Production worker intent remains three. Staging or scheduler observations are inapplicable; same-scope implementation reports require a source check. | Documentary acceptance is intent; the native claim is current only in its imported snapshot at `src/workers.rs`, `upstream-file-version-1`. | Possible for matching scopes; inapplicable for known environment/component mismatch. |
| Same cross-source matrix: unknown environment, with either “The worker concurrency limit is five.” or “The worker concurrency limit is three.” | Confirm the environment and current revisions before deciding agreement or disagreement. | Missing environment cannot be manufactured by the model or identical wording. | Unresolved possibility, never an established agreement or universal blocker. |
| `shared_code_evidence_is_identified_without_independent_confirmation`: two OpenWiki copies of “The worker concurrency limit is three.” | Production worker reports share the same `src/workers.rs` locator and opaque revision. | Both source-owned snapshots remain separate; shared provenance is not independent confirmation. No accepted policy is manufactured. | No contradiction; duplication is explicitly qualified. |
| `ambiguous_and_proposed_inputs_cannot_manufacture_accepted_supersession`: accepted “MySQL is the selected database.” versus “PostgreSQL may replace the ledger database.”, followed by a human supersession interpretation | Accepted MySQL and the proposal retain their original roles. | Advisory review acceptance remains `human_advisory_interpretation`; it creates no documentary relation. | Unresolved review, without authoritative supersession. |

No exceptions are fabricated when the retained passages contain none. The staging
replay row records its explicit exception; the rejected row records its rationale.
The lens tests assert the original lifecycle and retained source evidence rather
than treating capture time as an effective decision date. Superseded decision
status is distinct from withdrawn documentary support.

The matrix checks exact excerpts, digests, evidence revisions, primary material,
active source support, and both endpoints. Conflict and supersession groups are
tested at 16000, 4000, 2000 and 1000 tokens: either the complete evidence pair fits,
or the whole pair is omitted with an omission count. Native shared-evidence groups
receive the same complete-group check. Documentary endpoints remain inspectable
through the real `lore evidence` CLI, and conflicts through `lore review`.
`tests/cross_source_update.rs` separately covers immutable historical native
evidence, review reopening, changed inputs and reconciliation-contract upgrades.
`verification_qualifier_does_not_itself_block_a_preserving_change` in
`tests/decision_contract.rs` rejects turning a check into a policy blocker.
`missing_policy_requires_evidence_without_inventing_a_policy_blocker` in
`tests/decision_runtime.rs` requires an explicit uncertainty and a zero-call
fallback when no relevant retained evidence exists.

These are offline source-integrity contracts. They add no runtime execution,
authority category, inferred source passage, migration, or alternate registry.
