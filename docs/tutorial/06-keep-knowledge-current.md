# Lesson 6 — Keep knowledge current and reviewable

**Goal:** Learn how Lore responds to changing evidence, disagreements and task outcomes.

**Requires compiled knowledge** for commands below. Work only on the disposable classroom copy.

## Observe a baseline and subsequent change

```sh
lore baseline save first-day
lore changes --since first-day
```

The baseline saves a reference point; an unchanged comparison is expected until the retained registry changes. If you deliberately edit a classroom document, run `lore update` *with authorized inference* before `changes` or `guard` can compare its new retained meaning:

```sh
lore status
lore update
lore changes --since first-day
lore guard --since first-day --task "Assess the changed retry rule"
```

Neither `changes` nor `guard` silently refreshes project knowledge. Guardian provides **advice**, not proof of runtime safety.

## Handle review questions

```sh
lore --json review list
```

A review queue can be empty; **do not invent a review ID**. If a real typed candidate appears, inspect it using `lore --json review show ID` and consult [adjudication](../REVIEW_ADJUDICATION.md). Preview `lore --json review adjudicate ID --decision keep_separate --reason "Different scope"`; only apply a deliberate decision with the *returned exact digest* and `--apply --expect-context-digest DIGEST`. A status-only `review resolve` is not the same as a documentary adjudication. Neither action silently changes an accepted original ADR.

## Capture a task lesson safely

After real work, a calling tool can produce a structured `lore.task_outcome` receipt (schema 1). Review the [required fields](../TASK_FEEDBACK.md). Then:

```sh
lore --json feedback draft --receipt /path/to/a-real-task-outcome.json
```

A receipt is **not bundled in this course** because Lore's snapshot/evidence IDs and original file hashes must come from your actual initialized run. Fabricating these would defeat the safety contract. A draft is read-only. Adding `--output ... --write` explicitly creates a reviewable note in a permitted path, which you must inspect before importing with `lore update`. Supplied test claims and attestation hashes do not prove independent verification.

**Exercise:** Why does deleting a source not delete its entire evidence history, and why might a manually adjudicated interpretation become stale after source changes?

**Next:** [Everyday workflows](07-workflows.md).
