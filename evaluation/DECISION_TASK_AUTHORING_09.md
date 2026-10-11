# Independent decision-task handoff

This intake tool checks structure and byte bindings. It cannot create independent
participants, authenticate people, establish a historical fact from a name, or
turn an offline fixture into empirical evidence. Keep private originals, answers,
checker diagnostics and captures outside every model-visible source snapshot.

## Existing case contract

Supply `cases.json` with top-level `schema_version: 1`, `cases`, `pilot: true`,
`fixture_only: false`, and `independent_projects: true`. Those declarations are
necessary and insufficient. Pilot requires exactly six tasks from at least three
separately pinned `owner/repo` projects. Holdout uses `pilot: false`, `held_out:
true`, and at least 30 separate tasks; it must not share pilot or public source
bytes/task contracts. Conservative overlap checks may reject common files too;
do not relax or evade them by renaming sources.

Each case retains Lore's schema-1 fields:

```json
{
  "id": "decision-case-01",
  "project": "project-01",
  "source_root": "sources/decision-case-01",
  "task": "A precise programming goal, without the hidden answer",
  "editable_files": ["src/policy.py"],
  "test_command": ["{python}", "{manifest}/private/check_case_01.py", "{workspace}"],
  "verification_files": ["private/check_case_01.py", "private/check_data_01.json"],
  "expected_sources": ["docs:decision.md"],
  "critical_constraints": ["The accepted decision applies only to the reviewed scope"],
  "upstream": {
    "repository": "OWNER/REPO",
    "commit": "SUPPLY_THE_REAL_FULL_40_HEX_COMMIT",
    "files_sha256": {}
  },
  "input_sha256": "SUPPLY_THE_TRANSFORMED_SNAPSHOT_DIGEST"
}
```

This is an incomplete authoring example, not executable independent material.
Use canonical relative POSIX paths and argv, not shell commands. `{python}`,
`{manifest}` and `{workspace}` are substituted by the existing checker runner.
Pin every checker/helper/data file. Include all original source documents in
`source_root`; baseline receives them too. Do not copy verification files,
reference implementations or private review records into that directory.
Only editable files may differ from the original snapshot. No retroactive ADRs.

## Additional intake evidence

`decision_task_intake_09.py prepare` delegates to the existing experiment
preparer and creates incomplete `PREREGISTRATION.json`, `EXPERIMENT.json` and
`DECISION_INTAKE.json`. It does not run a checker or infer a model. Each intake
case has:

- `case_id`, actual `source_license`, and `original_root`: a separately copied,
  licensed source snapshot whose complete file hashes equal `upstream.files_sha256`.
- `decision_kind`: `accepted_vs_proposed`, `scoped_supersession`, `rare_exception`,
  `documentation_vs_implementation`, `rejected_design`, or `source_change_or_narrowed_grant`.
- `supporting_sources`: original document paths inside the visible snapshot.
  Reviewers must verify that this distinction is decision-dependent, plausible,
  predates the task and is supported by the pinned upstream/history captures.
- `authorship`: entries with `role` (`task` or `checker`), `person_id`,
  offset-bearing `authored_at`, and `capture: {path, sha256}`. Captures must match
  the independently accountable authors in preregistration and precede it.
- `control_files`: `correct` and `wrong` maps of editable paths to private,
  complete UTF-8 file proposals. The wrong control must realistically violate
  the reviewed decision; reviewers verify this, not an automatic classifier.
- `controls_capture: {path, sha256}`: the retained control receipt, bound to
  these task, checker, original snapshot and proposal bytes.

Paths for original/control files are relative to the case manifest. Capture paths
are relative to the private study bundle. Gold reviews use the existing
`independent_gold_reviews` contract: two distinct reviewers, distinct from task
author, checker author and operator, with timestamped, hash-bound, blinded,
conflict-declared captures of `task_contract_sha256`. Authenticated independent
readiness and post-run reviews remain separate requirements.

```sh
# Pin a draft's missing input/original hashes; never silently correct a bad pin.
# The output is a fresh sibling of the draft, preserving existing relative paths.
python3 evaluation/decision_task_intake_09.py manifest \
  --cases /ABS/INPUT/cases-draft.json --intake /ABS/INPUT/DECISION_INTAKE.json \
  --output /ABS/INPUT/cases.json

python3 evaluation/decision_task_intake_09.py prepare \
  --cases /ABS/INPUT/cases.json --output /ABS/PRIVATE-STUDY

# Fill the intake declarations in PRIVATE-STUDY before selected control execution.
# Execute only after arranging the independently enforced checker sandbox.
python3 evaluation/decision_task_intake_09.py validate-controls \
  --cases /ABS/INPUT/cases.json --intake /ABS/PRIVATE-STUDY/DECISION_INTAKE.json \
  --output /ABS/PRIVATE-STUDY/controls.json --execute-checkers

# Bind controls.json in each controls_capture, then collect real gold reviews.
python3 evaluation/decision_task_intake_09.py inspect \
  --cases /ABS/INPUT/cases.json --bundle /ABS/PRIVATE-STUDY
```

Controls run original, correct, wrong and unimplemented variants on fresh copies.
Original/correct must pass, wrong/missing must fail, coverage must remain equal,
and no checker may change source bytes. No diagnostic output is sent to a model.
Control execution has no provider calls, but executes operator-selected code.
Inspection and manifest pinning are read-only with respect to source/checkers.

For holdout inspection additionally pass `--pilot-prepared /ABS/PILOT/prepared.json`.
This checks conservative content overlap without reading holdout outcomes. The
sealed protocol separately requires the completed, reviewed pilot and untouched
holdout registration before tuning. `candidate_unreviewed` is useful progress;
`eligible_independent_pilot` describes accepted input declarations/captures,
not authenticated execution or a completed empirical study.
