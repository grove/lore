# Adaptive coding-task comparison

`adaptive_tasks.py` connects the explicit schema-5 experience to the existing
actual coding-task runner. It invokes the operator's coding-agent command,
applies its proposed changes to a separate source copy, and runs the selected
independent correctness and constraint checks. It also creates bound, blinded
review packets for two reviewers per attempted implementation.

This is executable evaluation infrastructure. The checked-in tests use labeled
offline protocol doubles and actual agent/checker subprocesses; they establish
neither real model quality nor better coding productivity. No real adaptive
coding-agent outcome has been measured by adding this protocol.

Lore 0.8 adds a [sealed preflight and retention wrapper](EXPERIMENT_08.md) around
this runner. It checks configured providers, independent registration, external
isolation/audit readiness and immutable command/source/model pins before launch,
and retains every planned assignment when execution aborts. The six arms and
post-run independent outcome gates below remain unchanged.

## Comparison arms

| Arm | Context supplied to the coding agent |
| --- | --- |
| `baseline` | The original source files and task, with no Lore context. |
| `fast` | Explicit `lore context --fast`, schema 2. |
| `lore05` | Explicit `lore context --schema-version 3`. |
| `lore06` | Explicit schema 4; inspection and investigation are requested when the study grants inspection. |
| `adaptive_no_reuse` | Explicit schema 5 with `--no-cache` on both context requests. |
| `adaptive_reuse` | Explicit schema 5 with its normal disposable cache policy. |

The existing `coding_tasks.py` and `decision_tasks.py` commands keep their
original arms and behavior. The new module reuses their source preparation,
agent input format, executable verification, evidence resolver and review
validation. It reuses the shared experience's schema-5, source-relationship and
exact static-observation validators.

**The public no-reuse control is `--no-cache`.** It disables combined adaptive
response and investigation reuse, and can affect other optional cache paths.
This experiment cannot attribute a difference solely to investigative memory.
A separate memory-only ablation would need a narrower production control.

### Matched warm-up and served request

Each case is initialized once. Every Lore arm receives a separate byte-identical
copy of that initialized project, including the same retained registry and
model configuration. No arm reads another arm's disposable cache.

Within each arm, the runner first requests context for the unsolved task, then
requests it again with exactly the same arguments. The second response is the
one supplied to the coding agent. The first response is retained as the
`warmup_response`; it is never an earlier coding answer or a solved patch. Both
requests occur before any coding agent or checker runs for that case. Context
arm order and coding-agent arm order are separately shuffled, with the context
collection order recorded in each sample.

This part measures **same-task reuse**. The separate `adaptive_sequences.py`
protocol below measures task B after a distinct task A, related and unrelated
source revisions, and a narrower grant. Its context results are not substituted
for executed coding outcomes. Repair iterations reuse the already collected
context, so they do not silently introduce additional investigation in one arm.

The independent checker and answer keys should be outside the selected source
tree, as in the bundled manifests. The runner never adds checker programs,
checker results or candidate patches to warm-up requests. Original project
files remain inputs to all arms; an operator who puts answer keys or the
independent checker inside that input tree has compromised the study. Source
fingerprints and the retained agent request make that input discipline
reviewable; this is not a filesystem sandbox.

## Commands

Preparation uses no model or Lore subprocess:

```bash
python3 evaluation/adaptive_tasks.py prepare \
  --output evaluation-results/adaptive-prepared-01
```

For an actual local-model run, build Lore and provide an available Lore model
and a coding-agent model. Replace the absolute adapter path and model
placeholders below. The coding-agent command must be a JSON argument array;
it is executed from a separate, initially empty working directory.

```bash
cargo build --release --locked

python3 evaluation/adaptive_tasks.py run \
  --lore-binary target/release/lore \
  --provider ollama --model YOUR_LORE_MODEL \
  --allow-inspection \
  --agent-command '["python3", "/ABSOLUTE/PATH/TO/lore/evaluation/coding_agent.py", "--provider", "ollama", "--model", "YOUR_CODING_MODEL"]' \
  --agent-id 'YOUR_CODING_MODEL; pinned settings; file-proposal adapter' \
  --agent-location local \
  --max-tokens 6000 --timeout 3600 \
  --max-attempts 3 --attempt-budget-seconds 7200 --seed 7 \
  --output evaluation-results/adaptive-local-01
```

Use `--cases /absolute/path/cases.json` for independently reviewed projects.
The schema-1 manifest and executable check format are the same as the
[existing coding-task protocol](INTELLIGENCE.md). A case identifies the source
snapshot, task, allowed editable files, critical conditions, and independent
test command. The source-copy limits and path protections are inherited from
that protocol. Public bundled fixtures remain recognized by source content
even if their labels claim that they are held out.

Provider, decision model, optional embedding model, role-specific endpoints and
reasoning options are passed through the existing configuration builder. These
settings stay identical across the copied Lore arms. The coding-agent command
and declared identity remain identical across all six arms; reported provider
model differences are rejected when those identities are available. A generic
external adapter's model, tools and actual inference are still operator
attestations, not authenticated facts.

Assessment performs no inference and does not rerun a checker:

```bash
python3 evaluation/adaptive_tasks.py assess evaluation-results/adaptive-local-01
```

`prepare` exits successfully after creating a fresh preparation directory.
`run` and `assess` exit with code 0 when the mechanical comparison is complete,
code 2 when its mechanical gates fail, and code 1 for invalid input or failed
execution. A passing mechanical exit code does not mean that a coding task
passed, a source interpretation was correct, or a productivity benefit exists.

## Permissions and comparable inputs

Every Lore, agent and checker subprocess receives its own environment
dictionary. Ambient `LORE_INSPECTION_ROOT`, `LORE_ALLOW_HOSTED_EGRESS` and
`LORE_ALLOW_CHECKOUT_EGRESS` are removed before applying this run's options.
The parent process's environment is never changed. Credentials can be
inherited privately; reports contain only the declared Lore grants, not the
complete environment.

`--allow-inspection` sets a root for the particular copied project or candidate
workspace. Without it, schema 4 and schema 5 receive `--no-inspect` and the
assessor rejects ungranted static observations. `--allow-hosted` supplies the
hosted Lore grant; a hosted coding-agent adapter must also receive its own
documented opt-in. `--allow-checkout-egress` requires both study options and
adds the same checkout consent to both adaptive arms. Repository configuration
cannot broaden these grants.

These are **declared Lore permissions**, which the Lore implementation consumes.
Environment variables do not sandbox the external coding agent or independent
checker. Their selected commands execute with the operator's privileges.
Filesystem, network, tool restrictions and actual model execution require
independent enforcement or accountable capture review outside this harness.
The generated `INTEGRITY_AUDIT.json` starts unmeasured and requires bound capture
files before an audit can pass.

The assessor checks source content hashes, copied configurations, cold registry
identity, distinct context copies, equal output budgets, exact declared grants,
and equal effective schema-5 capabilities. Both adaptive responses must name
the same project and registry revision. The agent receives the complete served
context and original files through the existing bound request contract. The
maximum token option limits Lore output, not the total original-source payload
or an arbitrary external agent's tools. Keep those external budgets fixed in
the pinned agent command.

Existing OpenWiki, Engram and Beads imports remain available through the same
case preparation. This six-arm protocol does not create a separate OpenWiki-only
agent arm or isolate the cost of producing upstream snapshots. The
[cross-source evaluation](CROSS_SOURCE.md) retains its separate source/tool
comparison; a study that claims an OpenWiki-specific agent benefit needs matched
upstream-only task attempts too.

## Recorded measurements and interpretation

Each context record keeps the complete warm-up and served responses, hashes,
CLI arguments, declared grants, exact evidence resolutions, original source
and registry state, and disposable-cache fingerprints. Generated static
observation references are checked against exact source bytes and line ranges.
The additive source-relationship manifest is checked against retained resolver
responses, preserving native qualifications and documentary revision bindings.

Each task retains every proposed implementation, agent request/response hash,
checker executable/helper hashes, independent correctness and constraint
results, and reviewer bindings. Assessment derives those bindings again from
the saved artifacts. Empty check sets, duplicate check IDs, non-boolean results,
missing kinds, changed checker programs and changed check coverage are rejected.
Assessment does not execute candidate code again or authenticate a researcher's
wholesale replacement of a report and all its source artifacts.

| Measurement | Meaning and limit |
| --- | --- |
| Passed tasks and failed constraint checks | Actual selected checker results against the submitted implementation. A failed task can coexist with a mechanically valid comparison. |
| Material mistakes and missed conditions | Concrete final-answer annotations averaged across at least two blinded reviewers, with separate reviews for earlier failed implementations. Claims of independence require bound captures excluding the operator and task/checker authors; names alone are insufficient. |
| Warm-up and served model calls | Calls reported by the real CLI for each request; the schema-5 envelope is unwrapped correctly. Unknown provider tokens and actual billing stay null. |
| Complete context size | Full Python-serialized response byte size plus the CLI's reported whole-output token budget. The byte metric is not a token estimate or a second independent tokenizer. |
| Served context latency | Wall time for the second CLI request. It includes the selected command's retrieval, inspection and revalidation work; no latency threshold establishes a win. |
| Warm-up-inclusive recorded cost | Initialization is charged in full to each Lore arm; context-copy time, both context requests, every coding/checker attempt and measured iteration overhead are included. The baseline has no Lore initialization charge. Supplied upstream snapshot creation and independent review labor are unmeasured. |
| Guidance cache hits | A hit must match the unsolved warm-up revision and non-prunable decision premises, retain the same trace, have saved cache data, and report zero served model calls. Complete optional items may pack differently; both responses undergo full source validation. Schema 3 retains its older public preferred-approach comparison. A cache hit is not a correctness result. |
| Retained investigation reuse | A served hit with `cache_reused` and a retained nonempty investigative trace. Merely enabling cache, returning fallback, or retaining no trace does not count. |

The summary reports signed reuse-minus-no-reuse differences in served context
calls, served context time, warm-up-inclusive recorded task time and passed
tasks. Negative or inconclusive findings remain visible. A missing cost
component remains unknown; calls do not imply billed dollars or provider token
counts. Review completeness and independently reviewed task attempts are
separate from fixture results.

### Bounded repair and hidden-checker hygiene

The default remains one attempt. `--max-attempts` permits 1–5 attempts and
`--attempt-budget-seconds` bounds the complete coding/checking loop. Every arm
gets identical allowances. The runner stops on the first independently correct
implementation, the attempt limit, or the wall-time limit. Each failed checked
attempt can receive only strings declared in the case's `repair_feedback`
mapping before execution, or a generic failure message. The runner never sends
raw checker IDs, diagnostics, exceptions, hidden test inputs or answer keys to
the coding adapter. Feedback is capped at eight messages; each declared message
is bounded. Subsequent requests contain the current allowed source edits and
the original task/context, plus a schema-1-compatible `repair` object.

`attempts/` retains each tested source copy; `attempt-records/` binds the exact
request, response, checker result, timing and implementation digest. Before each
adapter call, `attempt-invocations/` checkpoints a started invocation. Provider
errors, process failures, malformed responses and failures before verification
leave an `incomplete` record with its stage, elapsed time, known usage or null
usage, and error class. Such a failure aborts the run and cannot become a
completed checker outcome. Do not delete it or silently retry into the same
study directory. A completed run's assessor also validates every earlier
attempt, its cumulative costs, the stop condition and the exact checker identity.

`reviews/<sample>.json` reviews the final implementation and contains separate
`earlier_attempt_reviews` packets for all preceding failures. Each packet has
two blinded annotation slots and an `independence_evidence` list. Evidence
entries identify a reviewer, declare no conflicts and independent/blind review,
and bind a retained capture to `independence_target_sha256`. A capture must have
a relative path and exact SHA-256; the two reviewers cannot reuse identical
capture bytes. Reviewer identities must match the annotations. These remain
accountable evidence, not authentication of the reviewer's human identity.

### Preregistered pilot and full study

Preparation creates an incomplete `PREREGISTRATION.json`. Freeze a selected
cohort and model settings before collecting outcomes; then supply the completed
file with `run --preregistration /absolute/path/PREREGISTRATION.json`. Its bound
gold-review captures must live below `gold-review-captures/` beside that file;
the runner copies them into the study. The registration records exact source
repository/commit, original source hashes, transformed input digest, task and
checker contract, independent task/checker authors, and two independent gold
reviews. The model/revision, reasoning, temperature, tools, prompt digest,
command digest, attempts, wall time and external token/tool limits must be
pinned. Use the exact reported provider model identity; an alias or missing
model cannot substitute for a different observed model. Prompt identity must
be a SHA-256 digest. `lore.configuration_sha256_by_case` freezes each case's
complete generated Lore configuration before the study, covering its model
roles, reasoning, endpoint and permission settings. The assessor compares these
pins to the retained matched configurations. Model-weight/revision identity
inside a provider remains an accountable declaration. The harness enforces its
own path, output, attempt and wall-time limits;
provider identities and limits inside arbitrary external adapters require the
independent execution capture and audit.

Run six pilot tasks across three projects first. Fix concrete harness problems
on that pilot, freeze the study again, then use a separate full set of at least
30 held-out tasks across three real projects and all six arms: at least 180
initial coding attempts, plus recorded repairs. The
[real-source candidate guide](REAL_CODING_TASKS.md) provides 30 runnable public
candidates, exact source pins and a fixed six-task pilot subset. They remain
published debugging tasks with independent authorship and outcomes pending;
relabeling them cannot supply the held-out study.

The assessor separates mechanical comparison, completed annotations, independent
answer reviews, gold review/preregistration, true intelligent-mode coverage and
execution/egress audit. `independent_validation_complete` needs all of these,
actual nonzero coding-model calls on every attempt, and three distinct held-out
projects. `productivity_benefit_established` is always false: an analyst must
evaluate the preregistered effect and its uncertainty rather than treating a
mechanically valid run as a win.

### Paired outcomes and missing costs

`iteration_outcomes.per_task` retains all six arms for each matched task:
checker completion, critical-constraint failures, attempts/correction loops,
warm-up-inclusive time, time to first correct completion, reported provider
tokens/billing, source UTF-8 bytes, and whole-context token counts where the CLI
reports them. Source byte size is not presented as a tokenizer estimate. Failed
tasks remain in all-task elapsed-time statistics and have null time to first
correct completion. Unknown provider usage and billing remain null.

The baseline comparisons resample matched tasks, keeping their arm assignments
together, for 2,000 seeded percentile bootstrap draws and 95% intervals. Pass
rate, constraint failures and correction loops use mean paired differences;
time and cost use the ratio of arm medians. Usage coverage is explicit. Billing
comparisons over a complete-case subset cannot substitute for unknown costs.
Fewer than 30 paired tasks are flagged as small samples. Intervals are
conditional on these projects and do not remove correlation between tasks in
the same repository. The proposed product target is a 10-percentage-point pass
rate improvement, or a 20% time/cost reduction with non-inferior pass rate and
no increase in critical-constraint violations. No such outcome has been measured
by the checked-in control tests.

### Different-task, revision and permission sequence

`adaptive_sequences.py` has `prepare`, `run` and `assess` commands. The default
public payment sequence asks an explanation task A followed by a distinct
implementation task B, then repeats B after a meaningful same-size source edit,
an unrelated documentary edit, and removal of inspection/hosted/checkout grants.
The reuse and `--no-cache` arms start from copies of one initialized registry,
receive the same source changes and have identical effective capabilities at
each phase. The source updates call the actual `lore update` command. Each phase
retains its exact source snapshot, response, evidence resolutions, registry,
cache state, grants, usage and latency. Assessment replays the declared source
changes and rejects stale observations or wider-grant replay even when a
response hash has been recomputed. Initialization, refreshes, every context
request and measured protocol overhead are included in each arm's total.

```bash
python3 evaluation/adaptive_sequences.py prepare \
  --output evaluation-results/different-task-prepared

python3 evaluation/adaptive_sequences.py run \
  --lore-binary target/release/lore --provider ollama --model YOUR_LORE_MODEL \
  --allow-inspection --max-tokens 6000 --timeout 3600 --seed 7 \
  --output evaluation-results/different-task-01

python3 evaluation/adaptive_sequences.py assess evaluation-results/different-task-01
```

This companion records context/investigation behavior, not completed coding
work, and does not send solved patches or task-B gold into task A. A cache miss
can be correct after changed evidence; a hit is not proof of a correct patch.
Provider server-cache effects, genuine different-task model outcomes and human
learning remain separate measurements. Human contributions and independent
transfer use [HUMAN_ONBOARDING.md](HUMAN_ONBOARDING.md).

## Offline contract gate

```bash
python3 -m unittest discover -s evaluation/tests -p test_adaptive_tasks.py -v
python3 -m unittest discover -s evaluation/tests -p test_outcome_repairs.py -v
python3 -m unittest discover -s evaluation/tests -p test_adaptive_sequences.py -v
python3 -m unittest discover -s evaluation/tests -v
```

The focused fixtures exercise all six arms, positive and negative cache
bookkeeping, per-subprocess environments, exact observations, source-relationship
qualification, tampered arguments/grants/snapshots/requests/costs, actual failed
independent checks, and refusal to promote public fixtures to model-quality
evidence. No model credential is required, and these tests create no live-model
outcome report.
