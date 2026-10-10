# Choose shared-intelligence changes from recorded failures

Lore 0.7 supplies an operator-side triage command for the six-arm coding study.
It calls the existing adaptive assessor before reading outcomes, so invalid
source snapshots, incomplete arm matching, altered checkers or inconsistent
attempt ledgers cannot become tuning evidence.

```bash
python evaluation/failure_triage.py triage /absolute/adaptive-run
python evaluation/failure_triage.py validate-change /absolute/adaptive-run /absolute/proposal.json
python evaluation/failure_triage.py validate-change /absolute/adaptive-run /absolute/proposal.json \
  --holdout-prepared /absolute/prepared-holdout
```

The output groups recorded functional and critical-constraint failures,
correction loops, time exhaustion, provider fallback and independently reviewed
decision mistakes. Each observation carries its case, sample, arm, source
identity, attempted corrections and charged cost. Missing billing stays null.
These are observed categories, not automatic causal explanations.

`annotation_count` counts completed annotations whose answer and pilot bindings
validate. `independent_review_count` counts those annotations only after the
shared adaptive answer-review validator also checks the separate review captures,
their digests, their exact answer/study target and their matching reviewer
identities. The pilot operator, task authors and checker authors are excluded by
the same preregistration rules used by the adaptive assessor. Names and completed
forms alone produce zero independent reviews and cannot add an independently
reviewed failure classification. The counts describe accountable declarations
with retained artifacts; they are not authenticated proof of distinct humans.

A proposal binds the exact pilot metrics digest, existing shared engine modules,
failing sample IDs, a bounded implementation change, expected corrected behavior,
cost risk and proposed held-out case IDs. Successful pilot tasks are also excluded
from the holdout. Distinct IDs make a proposal reviewable but do not establish
that its task or source content is new. The command does not modify Lore, execute
a repository command, or expose hidden checker results to an agent.

```json
{
  "schema_version": 1,
  "pilot_metrics_sha256": "digest-returned-by-triage",
  "modules": ["src/context/adaptive.rs"],
  "failing_samples": ["sample-from-the-actual-pilot"],
  "expected_corrected_behavior": "State the exact observed error that should disappear.",
  "bounded_change": "Describe one change to the existing controller.",
  "cost_risk": "Describe how additional investigation and warm-up costs will be checked.",
  "heldout_plan": "Run the preregistered distinct tasks with the same model, grants and budgets.",
  "heldout_case_ids": ["a-distinct-preregistered-task"]
}
```

## Bind the prepared holdout before tuning

Without `--holdout-prepared`, validation returns
`heldout_plan_unverified: true` and keeps the proposal's
`algorithm_tuning_ready: false`, including when the pilot itself is fully
validated. Extra independence flags in the proposal cannot satisfy this gate.

The optional argument accepts an existing coding/adaptive preparation directory
or its `prepared.json`. Preparation must have retained the original case manifest,
source snapshots, declared upstream repository and full commit, original source
hashes, transformed input digest and preparation-time checker-file identities.
Validation rechecks the case-manifest bytes, complete task definitions, actual
snapshot bytes and selected checker/helper/executable hashes. It reads preparation
artifacts without invoking the checker or opening the holdout's outcome files.

The overlap check uses every pilot case, including successful tasks. It compares
task text, critical constraints and editable paths after removing case/project
IDs and storage roots; whitespace and casing are normalized for this conservative
comparison. It also compares complete source snapshots and the multiset of their
file-content hashes, so renaming a task or its source paths cannot create new
holdout content. Reusing an identical pilot snapshot for a differently described
task is conservatively rejected. Public bundled fixtures remain ineligible even
if their flags are changed.

A successful preparation check reports `content_bound_holdout_plan` together with
the prepared-manifest, case, task-contract and source digests. It establishes
bound content and checked pilot non-overlap. It does not establish independent
authorship, lack of prior exposure, semantic novelty of a paraphrased task or
independent holdout review. The latter remains explicitly
`independent_review_complete: false`; the `held_out` and `independent_projects`
flags do not prove it. Source pinning retains the declared upstream identity and
its byte bindings; independent source review remains a separate study gate.

## Evidence gates

`proposal_complete` means the proposal is bound and reviewable.
The triage report's readiness describes the pilot evidence. In `validate-change`,
`algorithm_tuning_ready` additionally requires the validated prepared holdout
described above, as well as a real-model pilot, independent validation and a
passing captured execution/egress audit. It remains false for public fixtures,
missing review captures, absent providers, unmeasured audits and unverified
holdout content.
`measured_improvement` remains false until an actual matched intervention and
independent holdout are evaluated. Producing a plan is not that experiment.

No adaptive algorithm or prompt is changed merely to make an unrun study look
complete. The 0.7 release report states whether the pilot occurred, lists any
actual observed failures, and keeps task success, critical violations and full
investigation cost as separate measures. This preserves the existing schemas,
deterministic `--fast` path and standing permission ceilings while making future
tuning concretely traceable to the work it is supposed to improve.

The focused protocol tests exercise renamed pilot tasks, changed source/checker/
manifest bytes, unopened outcome files, missing source pins, annotations without
independence captures, mismatched reviewers, reused or altered captures and
excluded study authors. These are integrity regressions, not actual model or
human study results.
