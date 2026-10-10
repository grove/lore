# Lore 0.8 acceptance ledger

Baseline: `0ba8b17f7e63726a8d81def2b60af18573d4640c`, merged Lore 0.7.0.
The [implementation plan](../docs/V08_IMPLEMENTATION_PLAN.md) defines the eight
packages and the engineering versus empirical release gates. This file records
work in progress; it does not declare 0.8 complete.

## Evidence rules

Every gate has an explicit `passed`, `failed`, or `not_run`/`unmeasured` status.
Only executed checks receive passing counts. Missing provider usage and billing
remain null. Real coding outcomes require pinned models, independent task/checker
review and external execution/egress evidence. Human outcomes require actual
consenting participants and independent assessors. Synthetic controls are
engineering evidence with their own scope.

The 0.7 Guardian source-restoration failures are retained unchanged. A new clean
run alone does not identify the writer or fix the archived failure. Full source
integrity and independent alert-quality assessment are separate gates.

## Executed starting evidence

The untouched 0.7 runtime passed all **174 Python tests**, formatting and strict
Clippy through the matching compiler driver. Five actual frozen-registry CLI
captures preserve schemas 2, 3, 4, 5 and the default 4 with unchanged source/state
bytes and zero provider calls. The standard local Rust command failed when the
environment/Cargo restored the generated CLI executable to mode 0644; eight
child-process launches reported `EACCES`. Running the identical compiled CLI
test executable after restoring its execute bit passed all eight. The failed
standard run is retained, and standard cross-platform CI is still required.

A 60-transition control ran **zero Lore subprocesses** and observed source
restoration in 40 events under the unchanged byte oracle (42 under the stronger
metadata/event check). Kernel filesystem events show temporary-file writes and
renames back into source paths, including `.rsync-tmp`. This establishes an
external writer for this reproduction. The writer PID and its identity in the
historical 0.7 captures are not authenticated. The first collector was a
development version, not frozen at process start; the receipt explicitly retains
that limit. Source-clean full replays in a separately isolated runner remain
required. See the [receipt and exact artifacts](results/baseline-08/engineering.json).

## Package acceptance

| Package | Engineering gate | Current evidence |
| --- | --- | --- |
| 1. Baseline | Reproducible source, contract, environment and command pins | See [baseline evidence](results/README-08.md) and the machine-readable acceptance ledger. |
| 2. Guardian | Every source transition and read-command interval intact; writer fixed or positively isolated | Pending investigation; historical full replay failures remain failed. |
| 3. Metering | Every provider attempt counted; unknown usage/cost retained | Pending implementation. |
| 4. First run | Bounded exact documentary help without config, models, writes or egress | Pending implementation. |
| 5. Adaptive default | Schema 5 default parity; pinned 2/3/4 and all grants/budgets preserved | Pending compatibility gate. |
| 6. Coding studies | Complete preflight, six arms, attempt/cost retention and tamper checks | Pending engineering wrapper; actual productivity unmeasured. |
| 7. Human workflow | Source-only orientation and consent/assessment integrity | Pending engineering checks; actual participants: 0. |
| 8. Release | Full local and Linux/macOS/Windows integration plus truthful documentation | Not run on integrated 0.8 code. |

## Empirical outcomes

| Outcome | Status | Missing prerequisite |
| --- | --- | --- |
| Six-task, six-arm real-model pilot | `not_run` | Explicitly configured coding/Lore model, approved provider access, independent task/checker reviews and an audited isolated execution environment. |
| Full held-out coding study | `not_run` | Pilot prerequisites plus independently authored/reviewed untouched held-out tasks. |
| Coding productivity | `unmeasured` | Actual matched, independently checked attempts and complete resource ledger. |
| Human first contribution and transfer | `unmeasured` | Actual consenting participants, sessions, patches and independent assessments. |
| Guardian precision/recall and burden | `unmeasured` | Source-clean model assessments, independent event/advisory review and runtime audit. |

No tuning of retrieval or controller algorithms is attributed to an unrun study.
