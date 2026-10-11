# Lore 0.9 empirical usefulness implementation report

Engineering tooling is implemented. The real pilot and holdout are
`blocked_external`; coding productivity remains `unmeasured`. Rust, SQLite,
context schemas, release policy and `0.9.0-rc.1` remain unchanged. No paid
inference or real-model smoke has been initiated.

## Delivered engineering

The sequential PRs add a pinned Codex proposal adapter; seal-compatible binary,
prompt/schema/environment/model observation contracts; independent decision-task
intake and source-preserving checker controls; a thin prepare/status/report
workflow; and explicit spending/billing enforcement gates. All use the existing
case API, provider ledger, isolated proposal loop, six-arm assessor and paired
bootstrap. There are no new dependencies or alternative benchmark engine.

| PR | Scope | Verified head | Merged revision | Offline Python tests | Exact-head CI |
| --- | --- | --- | --- | --- | --- |
| [A: baseline/status](https://github.com/grove/lore/pull/59) | evaluation: record empirical baseline and read-only study readiness | `4c90582e82648fcc9dab4f1a2aa38a81285150a1` | `0b64838cdae8c372b3ca07bb6305a2f931c99df3` | 242 run, 1 skipped; OK | [guardian-integrity](https://github.com/grove/lore/actions/runs/38173476834/job/114570819938) · [test (ubuntu-latest)](https://github.com/grove/lore/actions/runs/38173476855/job/114570820306) · [test (macos-latest)](https://github.com/grove/lore/actions/runs/38173476855/job/114570820116) · [test (windows-latest)](https://github.com/grove/lore/actions/runs/38173476855/job/114570820318) |
| [B: Codex adapter](https://github.com/grove/lore/pull/60) | Empirical 0.9 B: pinned Codex proposal adapter | `d11739e82919c1aeaa5fb5473df6fa95f6cc0c44` | `04ab59525f4e95ad94869f7581b6ada99220a3fc` | 250 run, 1 skipped; OK | [guardian-integrity](https://github.com/grove/lore/actions/runs/38176825034/job/114580865668) · [test (ubuntu-latest)](https://github.com/grove/lore/actions/runs/38176825027/job/114580866235) · [test (macos-latest)](https://github.com/grove/lore/actions/runs/38176825027/job/114580866063) · [test (windows-latest)](https://github.com/grove/lore/actions/runs/38176825027/job/114580866331) |
| [C: sealed protocol](https://github.com/grove/lore/pull/61) | Empirical 0.9 C: seal Codex execution and observed model identities | `bce2b986b132d2e20239800d6dbfc4f006e802bd` | `d7dcd179d8b7adeeb5cd5b9c8a6b20d4df2f19ea` | 254 run, 1 skipped; OK | [guardian-integrity](https://github.com/grove/lore/actions/runs/38178512808/job/114585905968) · [test (ubuntu-latest)](https://github.com/grove/lore/actions/runs/38178512798/job/114585906061) · [test (macos-latest)](https://github.com/grove/lore/actions/runs/38178512798/job/114585906063) · [test (windows-latest)](https://github.com/grove/lore/actions/runs/38178512798/job/114585905877) |
| [D: independent intake](https://github.com/grove/lore/pull/62) | Empirical 0.9 D: independent decision-task intake and controls | `2493d75e495317749e8691a06fb4b7a589cb48a8` | `28add597e1b3224c7b06a69e5a7ed3184673489d` | 262 run, 1 skipped; OK | [guardian-integrity](https://github.com/grove/lore/actions/runs/38180237208/job/114591047877) · [test (ubuntu-latest)](https://github.com/grove/lore/actions/runs/38180237269/job/114591048492) · [test (macos-latest)](https://github.com/grove/lore/actions/runs/38180237269/job/114591048326) · [test (windows-latest)](https://github.com/grove/lore/actions/runs/38180237269/job/114591048144) |
| [E: spending/report workflow](https://github.com/grove/lore/pull/63) | Empirical 0.9 E: bounded authorization and reproducible operator reports | `e67cc3a926573a8fde4298f572aa9fa1aaeb6baa` | `42b61ae1791763a17146a7099b21312a9b36e8da` | 269 run, 1 skipped; OK | [guardian-integrity](https://github.com/grove/lore/actions/runs/38182182265/job/114596879535) · [test (ubuntu-latest)](https://github.com/grove/lore/actions/runs/38182182262/job/114596879769) · [test (macos-latest)](https://github.com/grove/lore/actions/runs/38182182262/job/114596879459) · [test (windows-latest)](https://github.com/grove/lore/actions/runs/38182182262/job/114596879661) |


Every implementation PR requires Linux, macOS, Windows and Guardian success at
its exact reviewed head before merge. Guardian verifies frozen source integrity;
it is engineering evidence, not an independent empirical outcome review.
Superseded PR B checks were cancelled after fixes to avoid duplicate CI work;
only the final head's checks count. The final handoff PR is checked the same way.

The machine-readable [blocked-study receipt](blocked-study-2026-10-11.json) binds
these checks and collector bytes. PR F records this handoff only; its genuine
data milestone remains blocked. No existing result was replaced.

## Available evidence

- Immutable baseline: merged stabilization #58, exact source/binary/toolchain and
  corpus identities, formatting, strict Clippy, 515 Rust tests, release CLI smoke,
  and the original offline evaluator regressions.
- Public decision controls: all 24 correct/wrong/unimplemented checks pass with
  source bytes preserved; zero inference. These eight public cases cannot qualify
  as independent pilot or held-out tasks.
- Final offline regression: 269 Python tests run successfully (268 passed, one expected skip).
  Fake Codex execution covers successful and wrong proposals, strict JSON/path/
  size contracts, refusal, unknown/ambiguous usage, model/version/grant mismatches,
  timeouts, private output and retained failure journals. The full proposal/checker
  loop executes without hosted inference.
- Intake fixtures exercise private original/gold/wrong material, provenance,
  renamed public overlap, source leakage, independent declaration bindings,
  source-preserving controls, sealed intake tampering and pilot/holdout overlap.
  Their authored local identities are explicitly synthetic, never human evidence.
- Workflow fixtures verify 0/36 and 5/36 reporting, refused/timed-out/cancelled/
  interrupted attempts, unknown tokens/USD, source tampering, budget denial,
  existing positive/negative paired estimates, read-only reporting and sanitized
  artifacts. No adverse or unstarted assignments become fabricated successes.
- The saved offline `not_run` receipt checks report mechanics on public inputs.
  Its 36 slots are an offline denominator, not a registered independent pilot.
  The separate blocked-study receipt records zero genuine attempted assignments.

## Exact external blockers

1. No eligible independent pilot has been supplied: six decision-dependent tasks
   from at least three genuinely separate pinned upstream projects, licenses,
   original snapshots/documents, transformed inputs and external checkers are
   needed. Task and checker authors must be distinct. Two gold reviewers per task
   must be distinct from each other, authors and operator, with time-stamped,
   hash-bound captures and declared conflicts. Published/restoration fixtures and
   coding-agent-generated declarations cannot substitute for those people.
2. No qualifying untouched holdout is available: at least 30 additional tasks,
   provenance/checker/gold captures and a sealed pilot binding must be registered
   before any data-driven tuning. Renaming public or pilot sources does not help.
3. No real configuration has authenticated provider availability and actual
   backend revision. The installed CLI/version/help check proves local CLI
   compatibility only. Credentials were not inspected or tested. Supply one
   authorized coding model/revision/effort and one fixed Lore configuration,
   then obtain authenticated availability captures and per-invocation Codex
   model/revision observations where the CLI omits them.
4. No independently enforced study worker/captures are available. Supply a POSIX
   runner with OS-enforced checker/arm separation, actual tool/token/wall limits,
   an active egress allowlist/capture, private retention and authenticated
   provider/runtime/egress evidence reviewed by two independent people. Fresh
   directories, environment scrubbing and CLI sandbox flags are insufficient.
5. There is no explicit spending authorization or captured externally enforced
   finite dollar cap covering all Lore/coding/repair/provider work. Fill the
   sealed budget only after approval, provide actual billing-enforcement capture,
   and reconcile observed bills. Unknown USD is null; no token-price estimate
   becomes an invoice or enforceable dollar guarantee.
6. No actual 36-arm assignment pilot, blinded final/earlier-answer reviews or
   external post-run execution audit exists. Therefore there are no validated
   real failures to triage, no measured fixes to propose, no holdout outcomes,
   paired empirical intervals or stable-release/productivity approval.

## Next actions

An external study coordinator should obtain the task/checker/authorship/gold
material and untouched holdout first, using
[the author handoff](../../DECISION_TASK_AUTHORING_09.md). The operator should
then provision the authenticated worker/model/billing controls and obtain
explicit bounded spending approval. Follow
[the operator commands](../../EMPIRICAL_USEFULNESS_09.md) to inspect/control/pin/
seal/preflight a fresh private study. An authorized bounded smoke precedes the
full 36-assignment pilot; neither is authorized by this engineering report.

After the actual pilot, retain every invocation and all Lore initialization,
warm-up, served context, coding, repairs, checking, failures and waiting. Obtain
blinded independent answer reviews and the external audit before assessing and
triaging. Make only evidence-bound fixes, then run all six arms on the untouched
at-least-30-task holdout (180 initial assignments). Report conditional paired 95%
intervals, negative outcomes and missingness. Interpret +10 percentage points
success or 20% lower all-in time/cost under noninferior correctness and unchanged
critical constraints; release approval remains a separate explicit decision.

This is a file-proposal study. Interactive, tool-enabled field validation needs
its own preregistered protocol and cannot be silently counted toward this gate.

## Reproducing the engineering verification

The baseline receipt records exact commands, log hashes, toolchains and the
initial sandbox loopback failure followed by a successful host offline rerun.
For each implementation head, the relevant local Python tests passed before
publication; all three OS suites and Guardian passed before merge. Final checks:

```sh
python3.12 -m unittest discover -s evaluation/tests -v
python3.12 evaluation/decision_work_09.py validate --output /ABS/FRESH-PUBLIC-CONTROLS
git diff --check
```

The final suite ran 269 tests: 268 passed, one expected skip, no failures.
The controls rerun passed all 24 controls on all eight public cases. The saved
receipt includes the actual command arguments and log/content hashes. Rust
product files, Cargo dependencies and existing product tests have no changes
relative to stabilization. Historical result files are preserved. No real-model
compatibility, human independence, external runtime enforcement or billed-dollar
reconciliation was checked; these remain external study prerequisites.
