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
