# Lore 0.9 empirical usefulness

The Rust product remains `0.9.0-rc.1`. Engineering checks and public fixtures
do not establish empirical usefulness or authorize stable 0.9.

The formal experiment preserves `lore-experiment-08-v1` and all six
`adaptive-coding-v1` arms: baseline, fast, lore05, lore06, adaptive_no_reuse,
adaptive_reuse. Baseline receives the same original sources, including documents.
Every Lore initialization, warm-up and served request is charged to its arm.

Engineering readiness is separate from the missing real study: six independent
decision-dependent pilot tasks from at least three pinned projects (36 initial
assignments), then at least 30 separate held-out tasks (180 assignments), real
authorized models, external isolation/egress captures and blinded independent
reviews. Published decision fixtures and restoration candidates cannot qualify.

## Inspection (zero inference, zero checker execution)

```sh
python3 evaluation/study_09.py status /ABS/PRIVATE-STUDY
python3 evaluation/study_09.py doctor /ABS/PRIVATE-STUDY --format markdown
```

These commands delegate to the existing preflight gates and do not write.
Gate identifiers refer to [the sealed protocol](EXPERIMENT_08.md); inspect its
private `preflight` output for detailed operator diagnostics. An absent or
incomplete bundle remains `not_run`, with productivity `unmeasured`.

## Existing protocol handoff

Use fresh private absolute directories outside the checkout. `experiment_08.py`
provides `prepare --cases ... --output ...`, `pins BUNDLE`, `seal BUNDLE`,
`preflight BUNDLE`, `run BUNDLE`, and `assess BUNDLE`. Preparation and pinning
do not approve execution. Complete independently authored tasks, checker/gold
reviews and model/prompt declarations before sealing. Supply authenticated
provider availability, OS isolation, enforced tool/budget limits and egress
evidence before launch. Obtain explicit operator spending authorization before
any live inference. No paid inference is part of CI or this implementation.

After actual execution, independent blinded answer reviews (including earlier
failed attempts) and an external execution audit remain required. Use
`failure_triage.py triage BUNDLE/run` only on validated actual pilot evidence.
Register untouched holdout tasks before tuning. Never delete adverse results.

The target is +10 percentage points success, or at least 20% lower all-in
time/cost with noninferior correctness and no additional critical violations.
Pilot findings are diagnostic; held-out paired 95% intervals and the unchanged
release policy govern stronger claims. Unknown usage and billed USD stay null.
File-proposal experiments do not reproduce everyday interactive agent work.

See [decision controls](DECISION_WORK_09.md), [release gates](PRODUCT_QUALITY_09.md)
and [baseline receipts](results/empirical-09/README.md).

Engineering A–E is complete; pilot and holdout remain `blocked_external`.
See [the implementation report](results/empirical-09/IMPLEMENTATION_REPORT.md)
for merged PRs, exact-head verification, available offline evidence and the next
external study actions. No paid inference or empirical productivity claim is
part of that handoff.

## Pinned Codex proposal adapter

`codex_agent.py` accepts the existing schema-1 JSON request on stdin and emits
one validated complete-file proposal on stdout. It supports the reviewed
`codex-cli 0.162.1` only, on POSIX. Installations with another version require
an explicit compatibility review; there is no fallback. The schema is resolved
to the task's editable filenames; strict structured output requests every
editable file, including unchanged files. The runner applies proposals.

```sh
# Configuration example only: executing this command requires authorized spending.
python3 evaluation/codex_agent.py \
  --codex-bin /ABS/PINNED/codex --codex-sha256 SHA256 \
  --codex-version 'codex-cli 0.162.1' --model PINNED_MODEL \
  --reasoning high --timeout 600 --allow-hosted < PRIVATE_REQUEST.json
```

The executable, fixed prompt (`codex_agent.PROMPT`), schema, flags and environment
scrub policy are pinned. Version/help probes use no inference. Each invocation
uses a fresh private directory and `CODEX_HOME`, read-only CLI sandbox, disabled tools and no
workspace/user rules, profile or resumed session. Credentials must be supplied
privately through `OPENAI_API_KEY` or `CODEX_API_KEY`; existing login stores are
not copied into the study. These flags do not establish
OS isolation, managed configuration enforcement, egress limits or spending
authorization; independently authenticated worker captures are still required.

Documented terminal JSONL usage feeds the existing `LORE_USAGE_LEDGER`. Cached
input tokens are recorded separately without adding them twice. Missing or
ambiguous usage, internal provider request/retry counts, backend revision and
billed USD remain unknown. An observed CLI model is distinct from a requested
model; external observation must establish the actual backend. Raw events,
prompts and diagnostics are private and never printed on failure.

The flags and event shapes were checked against the installed version/help and
[official noninteractive documentation](https://learn.chatgpt.com/docs/non-interactive-mode).
Offline tests use a pinned fake executable; no real Codex inference has run.

## Sealed Codex configuration

Set `EXPERIMENT.json`'s `run.agent_command` to a JSON argv array containing
canonical Python and `evaluation/codex_agent.py` paths followed by the adapter
arguments above. Set `run.agent_location` to `hosted`, `run.allow_hosted` to true,
`agent.provider` to `openai`, `agent.endpoint` to `https://api.openai.com/v1`,
and declare the same coding model/revision/reasoning in preregistration. The
inner Codex timeout must be shorter than both the outer timeout and the attempt
wall budget, so it can flush its failure ledger before the runner stops it.

Write the exact `codex_agent.PROMPT` bytes to the private `agent.prompt_file`.
Run `experiment_08.py pins BUNDLE`, then copy `configuration.codex_adapter`
into `PREREGISTRATION.json`'s `agent.codex_adapter`. This pins the adapter,
CLI executable/version, schema, prompt, fixed flags/config and scrub policy.
Declare `tools: []` and `tool_call_limit: 0`. Seal and preflight check the same
identities again; editing a pin after seal requires a new study directory.
Generic private adapters retain their existing independent control route.
The direct OpenAI/Ollama adapter remains available.

Codex JSONL may omit the actual model or backend revision. Assessment never
substitutes the requested CLI model for observation. In `run/CODEX_IDENTITIES.json`,
retain `schema_version: 1`, `complete: true`, the canonical `metrics_sha256`,
an `authentication` method and hash-bound capture, and one `invocations` entry
per unique receipt `call_id`. Each entry needs the observed exact `model`,
`model_revision`, offset `observed_at`, and private hash-bound `capture`.
Two distinct independent `reviews` bind the digest of the whole observation
record excluding `reviews`; use the existing independent review shape and
exclude operator/task/checker authors. Authentication must come from the real
external observer. The Python harness verifies bindings and declarations; it
does not authenticate people or make a CLI model flag into backend evidence.

Offline fixtures exercise this contract and the actual proposal/checker loop,
including wrong answers and retained timeouts. They establish compatibility,
never model availability, independence, productivity or release eligibility.

Independent task preparation and author handoff are specified in
[DECISION_TASK_AUTHORING_09.md](DECISION_TASK_AUTHORING_09.md). Codex sealing
requires a completed, hash-bound `DECISION_INTAKE.json`; the public restoration
and decision fixtures cannot pass that intake by relabeling or renaming files.
Supporting decision documents retain original upstream bytes even if marked
editable. Capture review still requires actual independent people.

## Independent intake, spending and report workflow

Read [the external author contract](DECISION_TASK_AUTHORING_09.md) first. Collect
six private decision tasks from at least three pinned projects; separately
collect at least 30 untouched holdout tasks before tuning. Copy original source
documents byte for byte. Keep original solutions, controls, checkers and human
captures outside every source input. `candidate_unreviewed` is useful collection
progress, not execution readiness.

```sh
# Preparation, inspection and reporting execute no model or checker.
python3 evaluation/study_09.py prepare \
  --cases /ABS/INDEPENDENT-PILOT/cases.json --output /ABS/PILOT-STUDY
python3 evaluation/decision_task_intake_09.py inspect \
  --cases /ABS/INDEPENDENT-PILOT/cases.json --bundle /ABS/PILOT-STUDY
python3 evaluation/study_09.py doctor /ABS/PILOT-STUDY --format markdown

# Explicit checker execution on reviewed inputs, in the enforced study sandbox.
python3 evaluation/decision_task_intake_09.py validate-controls \
  --cases /ABS/INDEPENDENT-PILOT/cases.json \
  --intake /ABS/PILOT-STUDY/DECISION_INTAKE.json \
  --output /ABS/PILOT-STUDY/controls.json --execute-checkers

# Configure first; these commands cannot create external approvals or reviews.
python3 evaluation/experiment_08.py pins /ABS/PILOT-STUDY
python3 evaluation/experiment_08.py seal /ABS/PILOT-STUDY
python3 evaluation/experiment_08.py preflight /ABS/PILOT-STUDY

# Live run: only after explicit spending authorization and authenticated readiness.
python3 evaluation/experiment_08.py run /ABS/PILOT-STUDY

# Read-only assessment; optional output is a fresh private directory.
python3 evaluation/study_09.py report /ABS/PILOT-STUDY --format markdown
python3 evaluation/study_09.py report /ABS/PILOT-STUDY --output /ABS/PRIVATE-REPORT
```

Use Python 3.10 or newer; real execution is restricted to the independently
isolated POSIX worker.

`study_09.py` delegates preparation/preflight/assessment to the existing evaluator.
It never launches the study or signs reviews. `report` revalidates retained
bindings in memory (`assess(..., persist=False)`); explicit output writes only
`RESULT.json` and `REPORT.md` outside the checkout. Written report links resolve
to retained artifacts from that report directory; stdout uses bundle-relative
names. Unrun artifact links describe future handoff locations. Original `experiment_08.py assess` still persists
its private retention/result/triage artifacts. Inspection does not overwrite
historical results. Expired current readiness blocks another launch; a retained
study uses readiness bound to its recorded launch. The report distinguishes
current launch blockers from retained assessment qualification. Invalid bindings block qualification while retaining counts
where the journals remain readable.

Every newly sealed experiment needs `EXPERIMENT.json.budget`:

```json
{
  "operator_id": "EXACT_PREREGISTERED_OPERATOR",
  "authorized_at": "OFFSET_TIMESTAMP_BEFORE_LAUNCH",
  "max_billed_cost_usd": null,
  "enforcement_method": "ACTUAL_BILLING_GATEWAY_OR_PROVIDER_HARD_LIMIT",
  "capture": {"path": "budget-captures/authorization.txt", "sha256": "SHA256"}
}
```

This incomplete example deliberately fails. The real authorization must specify
a finite positive dollar cap and hash-bound operator approval. It is sealed with
all run/model/task pins. `READINESS.json.enforcement.billing_budget_enforced`
must be true, with a separate `billing_enforcement` capture proving the actual
external cap covers every enabled coding and Lore role, including retries,
initialization, warm-up and served context. The existing two external readiness
reviews bind these controls and authentication together. Without an available
gateway/provider limit and real captures, preflight fails closed. Python cannot
guarantee a dollar cap from token estimates. An authorization filename or true
boolean is not actual permission or proof of enforcement.

The additive budget field can be omitted when reading older specifications;
new sealing/launching requires approval. Historical sealed code identities are
never silently updated to newer evaluator code. Keep the original collector
revision for historical assessment. The direct coding adapters and Rust product
contracts remain available and unchanged.

Reports include all planned slots, started and attempted assignments, checked
outcomes, failures, missingness, observed wall time, arm costs, paired 95%
intervals and review/audit qualification. Initialization/warm-up/served context
are charged by the existing assessor. Unstarted work remains separate from
failed attempts. Provider ledgers are captured coverage, not complete invoices;
unknown tokens/USD stay null and billing remains `budget_unverifiable` until
observed billing can be reconciled externally. No price-table estimates or
unobserved zero-dollar assertions are inserted. Pilot conclusions stay
exploratory. Qualified holdout intervals still require independent interpretation;
the report cannot authorize a stable release or broad field productivity claim.

For troubleshooting, match public `doctor` gate identifiers with private
`experiment_08.py preflight` diagnostics. Missing intake requires real author
material; missing spending/readiness requires the operator and actual enforcement.
Hash mismatches require investigating the changed bytes and a fresh study,
not editing a sealed receipt. After an abort, retain the entire bundle and
never resume over its attempts. Raw code, prompts, events, private diagnostics
and credentials must stay in the private retained bundle; reports expose counts,
content identities and relative artifact links only.
