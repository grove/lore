# Decision-dependent work in 0.9

Eight public development fixtures cover accepted decisions versus proposals,
scoped supersession, rare exceptions, reports versus policy, implementation/doc
conflict, rejected approaches, A→B→A source support, and narrower caller grants.
Every project and document is explicitly authored fixture material. These cases
are neither independent upstream tasks nor an untouched holdout.

```sh
python3 evaluation/decision_work_09.py prepare --output /absolute/new/study
python3 evaluation/decision_work_09.py validate --output /absolute/new/controls
python3 -m unittest discover -s evaluation/tests -p test_decision_work_09.py -v
```

Use Python 3.10 or newer. Preparation reuses `adaptive_tasks.py`, retaining the
six original arms and their existing command, input, repair, usage, review and
isolation contracts. `adaptive_no_reuse` disables combined caches; it is not a
memory-only ablation. No new experimental launcher or production agent is added.

Each source tree contains original fixture documentation and an incomplete
implementation. `controls.json` and `check.py` live outside all agent inputs and
are bound as external checker files. Validation executes 24 authored controls:
eight correct implementations, eight plausible decision mistakes and eight
unimplemented bodies. All correct implementations must pass, all wrong/missing
ones must fail, and no checker may change source bytes. This is an offline
engineering test, not independent checker authorship or model execution.

The common reviewer packet now explicitly prompts for missed mandatory rules,
exceptions, lifecycle/scope mistakes, unsupported claims, incorrect readiness,
wrong implementation seams, incomplete validation and repeated investigation.
Reviewers record exact observations in the existing bound fields; no automated
classifier invents an independent judgment. Existing assessor controls cover
duplicate attempts/reviewers, fabricated costs and excluded failures. New tests
check original-source parity for every arm, relabeled public tasks, checker
byte leakage and altered task/checker pins.

## Genuine pilot shortfall

**Eligible independently reviewed decision-heavy tasks: 0 of 6 required.**
The retained three upstream source slices (boltons, more-itertools and
python-dotenv) have exact commits, licenses and original byte hashes in
[`coding-real-v1/manifest.json`](corpora/coding-real-v1/manifest.json). Their
existing six pilot tasks restore removed functions. They were authored publicly
by the implementation agent, lack independent task/checker/gold reviews and
do not supply an independently established decision-history comparison. They
remain useful restoration controls; relabeling them would not satisfy 0.9.

No upstream ADR or history has been fabricated. Supply six independently selected
decision-dependent tasks across at least three separately pinned projects, with
original licensed decision/history materials, file hashes, disjoint task/checker
authors and two gold reviews. Use the unchanged `experiment_08.py` pilot path;
its sealed model, exact prompt, tool, permission, cost, OS isolation and external
egress gates still apply. A larger study retains the 30-task untouched-holdout
rule. The eight new public fingerprints are recognized even after task relabeling.

No genuine pilot was launched. Provider usage and billed cost for that unrun
study remain unknown, and no success-rate, effort, transfer or general benefit
claim follows from these controls. See the [acceptance ledger](results/acceptance-09.json).
