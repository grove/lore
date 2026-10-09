# Lore 0.6 decision-quality evaluation

Lore 0.6 adds an executable four-arm comparison of completed coding tasks. It
reuses the existing snapshot, configuration, coding-agent, verification and
review-binding machinery in `coding_tasks.py`. The release target is a **15%
relative reduction in material decision mistakes compared with Lore 0.5** when
the baseline has mistakes. This is a target; no improvement has been measured
by the checked-in fixtures.

| Arm | Coding-agent input |
| --- | --- |
| `baseline` | The task and original project sources |
| `fast` | Identical task and sources plus deterministic schema-2 context |
| `lore05` | Identical task and sources plus the retained schema-3 Lore 0.5 intelligent contract |
| `lore06` | Identical task and sources plus schema-4 decision guidance with explicitly enabled source inspection |

The same pinned coding-agent command, settings, allowed edits and verification
command apply to all four arms. Execution order is shuffled. Every implementation
starts from the same original bytes. Each context arm uses an isolated copy of
the freshly initialized Lore project, so one arm does not warm another arm's
guidance or embedding cache. Initial context and repeat-context latency remain
separate. The first context measurement includes the recorded isolation cost.

The comparison uses 0.5's compatibility implementation in the same Lore binary;
record the binary hash and configuration for reproducibility. It does not claim
that a schema number alone authenticates an arbitrary historical release.

## Prepare without inference

```bash
python evaluation/decision_tasks.py prepare \
  --output evaluation-results/decision-prepared
```

The bundled payment, ledger and release cases are development fixtures. Known
fixture hashes and bundled-case provenance keep them fixture-only even if a
manifest or report is relabeled. Preparing them performs no model calls, coding
agent execution or test execution. Offline tests exercise the evaluator and
real fixture verification subprocesses; they do not establish model quality.

## Run a real four-arm comparison

Build Lore and provide an explicit coding-agent command following the existing
[coding-agent protocol](INTELLIGENCE.md#coding-agent-protocol). For example:

```bash
python evaluation/decision_tasks.py run \
  --lore-binary target/release/lore \
  --provider ollama --model YOUR_PINNED_LOCAL_LORE_MODEL \
  --agent-location local \
  --agent-id 'YOUR_PINNED_CODING_MODEL; recorded settings' \
  --agent-command '["python3", "/absolute/path/to/lore/evaluation/coding_agent.py", "--provider", "ollama", "--model", "YOUR_PINNED_CODING_MODEL"]' \
  --cases /absolute/path/to/held-out-cases.json \
  --output evaluation-results/decision-local-01
```

The schema-4 arm uses `--inspect`. Add `--investigate` for a separately labeled
comparison of deeper investigation. Snapshots get an explicit
`context.inspection.root: "."` because copied evaluation source directories do
not require Git metadata. Inspection starts at the copied input snapshot and
does not see verification scripts kept outside that snapshot. The normal Lore
inspection privacy rules still apply.

Hosted documentary inference requires `--allow-hosted`. Sending freshly inspected
checkout material to a hosted Lore model additionally requires
`--allow-checkout-egress`. One flag does not imply the other. Hosted coding-agent
access continues to require the existing explicit agent-location and hosted
opt-in settings; that agent receives the same complete input source snapshot
in every arm. The included coding adapter also enforces its own hosted opt-in.
The runner never supplies credentials in report files.

The agent and operator-selected verification commands execute as ordinary
subprocesses with the operator's privileges. This is the existing evaluation
execution boundary, separate from Lore context's read-only inspection boundary.
The evaluator does not execute commands discovered in project documents, install
an agent, or treat source text as executable instructions. Run untrusted candidate
programs in an independently isolated environment.

## Meaningful held-out projects

Use at least **three independently selected real projects** with meaningful
tasks. The case manifest retains the schema documented in
[the 0.5 evaluation guide](INTELLIGENCE.md#blind-human-review-and-assessment):
use `source_root` for each original project snapshot, keep `docs/` and relevant
implementation files in it, and specify externally maintained verification
commands and any `verification_files`. Each project requires a distinct source
fingerprint. Set `fixture_only: false`, `held_out: true`, and
`independent_projects: true` only when those declarations are accurate.

The existing snapshot limits remain 100 files and 5 MB per selected source
snapshot. A carefully selected project slice can fit those limits; document
what it excludes and do not claim complete-repository coverage. Source selection,
project independence, holdout discipline and test adequacy need researcher
judgment. Fingerprints make changes detectable; they cannot authenticate those
judgments. Adding copies of the same project under different names does not
satisfy the distinct-source requirement.

## Measurements

| Measurement | How it is recorded |
| --- | --- |
| Correct completion of the evaluated attempt | Actual results from the operator-selected correctness checks |
| Preservation of material constraints | Executable constraint checks plus concrete missed-constraint annotations |
| Material decision mistakes | Specific human-reviewed mistakes and counterexamples in each answer |
| Unnecessary blocking | Explicit human judgment of whether a useful permitted change was needlessly blocked |
| Revision of incorrect hypotheses | Reviewed counts of identified incorrect hypotheses and those actually revised |
| Implementation choice quality | Independently assigned 0–3 score with notes |
| Latency | Measured preparation, isolation, first context, coding attempt and verification time; repeat context separately |
| Model calls, token usage, billed cost | Actual available usage records; missing components stay `null` |
| Time to correct completion | `null` / unmeasured in this one-attempt runner |

One implementation attempt's total wall time is **not** a measurement of the
iterative time needed to reach a correct completion. The latter needs an
explicitly instrumented iterative protocol. A failing attempt is recorded as
failed; its latency is not relabeled as time to success. Request counts do not
imply token counts or billed dollars. Each Lore arm bears the same full shared
preparation cost, and upstream OpenWiki/Engram/Beads generation cost remains
outside the measured scope.

## Exact observation and source integrity

The assessor independently recomputes every local observation's full-file
SHA-256 hash, exact 1-based line range, verbatim excerpt and content-addressed ID
from the bound original source snapshot. Compact UTF-8 JSON of
`[path,start_line,end_line,excerpt,content_hash]` defines the observation ID hash.
CRLF, UTF-8 and a final line without LF remain exact. The verifier does not rely
on a model statement that an observation was grounded.

Every local observation reference must resolve to a complete observation in the
same response. Imported observation IDs remain distinct and resolve to their
retained imported records. Documentary and imported evidence references use the
existing `lore evidence` resolver records. Explicit local observation IDs in a
coding answer must exist in the supplied context.

The runner records source content hashes and modification times before and after
context collection. Assessment checks those records against the actual copied
checkout and original sources again. Registry and generated-wiki hashes must
also remain unchanged. Original source snapshots, allowed implementation bytes,
agent input, answers, verification scripts and complete context responses remain
bound to each sample. Editing an observation and updating only a summary hash
does not repair its source binding.

Static implementation and test declarations do not prove runtime or deployed
behavior. The separate executable checks establish only the conditions those
selected checks actually exercise.

## Independent review and external integrity audit

Every answer gets **two initially unfilled human-review slots**. Supply distinct
reviewer identities, dates, explicit blinding confirmation and concrete notes.
Reviewers record material mistakes, missed constraints, unnecessary blocking,
incorrect and revised hypotheses, implementation quality and any severe
unsupported project assertions. The evaluator does not generate these scores.
It averages ordinary counts across reviewers per answer, preserving the actual
counterexamples and any disagreement in the review packets. A severe assertion
reported by either reviewer prevents that integrity gate from passing.

Give reviewers the anonymous answers and common source snapshot while keeping
the arm assignment and metrics separate. Answers may reveal their arm; reviewers
must leave blinding false when that happens. Each review binds the complete
metrics and exact answer hash, task and constraint list. Changed source, answer,
context, model identity, costs or execution details invalidate the old review.

The run also produces `INTEGRITY_AUDIT.json`, initially **unmeasured**. Response
metadata saying no checkout was sent is a contract assertion, not independent
network evidence. To complete the audit, an identified auditor must review
independently collected network/process/filesystem captures, provide the exact
capture paths and SHA-256 hashes, and explicitly assess:

- Zero unauthorized checkout egress.
- Zero unrequested command execution by the inspected context workflow.
- No accidental source modification.

The audit is bound to the complete run and capture bytes. Missing captures,
unfilled audit judgments or stale bindings cannot pass the release gate.
Capture interpretation and auditor identity remain accountable human
attestations. Do not mark them measured solely because a unit test passed or a
model claimed compliance.

```bash
python evaluation/decision_tasks.py assess evaluation-results/decision-local-01
```

`comparison_complete`, `human_review_complete`, `independent_validation_complete`
and `release_gates_passed` are separate results. A complete mechanical comparison
can have failed coding attempts and pending human review. Every independent
comparison needs actual intelligent responses in both intelligent arms;
fallbacks are visible and cannot establish an intelligent-quality improvement.

When the 0.5 reviewed material-mistake baseline is positive, the target is
`(mistakes_05 - mistakes_06) / mistakes_05 >= 0.15`. A zero baseline yields a
`null` ratio and a separate zero-baseline/no-regression result. It never yields
an invented percentage improvement. Release gates additionally require three
independent real held-out projects, complete independent reviews, valid source
contracts, a completed external integrity audit, zero severe unsupported 0.6
project assertions, and passing 0.6 task checks.

**Default inspection benefit remains unestablished.** This harness records the
evidence for a decision about usefulness and overhead, but one successful run
does not automatically enable default inspection. Lore 0.6 ships inspection as
opt-in until independent evaluation justifies that change. No live-model score,
15% improvement, cost reduction, or independent release acceptance is claimed
by the code and fixtures alone.

```bash
python -m unittest discover -s evaluation/tests -v
```
