# Reviewable task feedback

```sh
lore --json feedback draft --receipt task-outcome.json
lore feedback draft --receipt task-outcome.json --output docs/proposed-task-note.md --write
```

An initialized project is required. Drafting loads read-only configuration and storage: no init, migration, inference, execution or network access. Stdout is the default. Explicit output creates one new Markdown file exclusively; existing files, agent instructions, symlinks and generated/state paths are refused. Review the proposal under your ordinary source-write workflow before `lore update` imports it. Removing an imported note withdraws current support and preserves historical evidence. No feedback database or second authoritative memory is created.

The input is `lore.task_outcome` schema 1, at most 512 KB, with these **required** fields. Unknown fields are rejected. Unknown values in nullable fields must be explicit `null`.

| Field | Shape |
| --- | --- |
| `contract`, `schema_version` | `"lore.task_outcome"`, `1` |
| `task_id`, `scope` | Opaque ID; 1–64 relative project paths |
| `snapshot` | `{ "project_id": "…", "registry_revision": "blake3:…" }`, as in schema-5 context |
| `source_files` | Complete map of configured `ROOT:relative/path.md` to original file BLAKE3 digest (up to 1024) |
| `evidence_ids` | Up to 128 retained documentary/native evidence IDs |
| `changes` | Up to 128 `{ "path": "src/file.rs", "before": "blake3:…", "after": "blake3:…" }`; one hash may be null for addition/deletion |
| `checks` | Up to 128 `{ "event_id": "attempt-1", "checker_id": "unit-test", "argv_digest": "blake3:…", "exit_status": 0, "log_digest": null }`; exit may be null |
| `started_at`, `finished_at` | RFC3339 timestamps with offsets; finish may be null |
| `usage` | `{ "elapsed_ms": null, "model_calls": null, "input_tokens": null, "output_tokens": null, "cost_usd": null }`; known values are nonnegative |
| `notes` | Up to 32 `{ "kind": "failure", "text": "Private observed failure" }`; kinds: failure, repair, observation; text ≤4000 bytes |
| `actor` | `{ "kind": "agent", "id": "opaque-caller" }`; kinds: agent, user, external_runner |
| `grants` | `{ "inspected_files": [], "network": false, "execution": false }`; historical declaration, never permission |
| `attestations` | Up to 32 `{ "kind": "external_check", "actor_id": "runner", "capture_digest": "sha256:…" }`; also independent_review; empty array if none |

Digests name BLAKE3 or SHA-256 with 64 lowercase hex digits. Lore source/registry digests use BLAKE3. `argv_digest` hashes the producer's exact serialized argv array; the receipt carries only that hash, never an executable command. IDs contain ASCII letters, digits, dot, dash and underscore (≤120 bytes). Events, paths and evidence must be unique; paths cannot escape the project, traverse symlinks or select hidden/private files. Invalid evidence fails closed. Changed registry/configuration/source hashes produce a **stale investigation proposal**, never a current factual assertion.

A hash proves byte identity, not execution. Changes are self-reported unless the invoking caller passes `--allow-file src/file.rs` for each exact file to hash. That verifies current bytes/deletion only, not the historical before state or any check run. Receipt grants cannot authorize these reads. No checker command, log path or capture is opened. Unknown usage and billing remain null.

Submitted external-check and independent-review claims remain unverified: this CLI has no authenticated external verifier or independence trust anchor. Its separate counters for authenticated results and independently reviewed conclusions stay zero. Continue to use the sealed evaluation protocol and its external reviews for actual outcome claims; a feedback proposal cannot satisfy those gates.

For disclosure safety, public output contains receipt/scope/actor hashes, evidence IDs, failure/repair counts and reported outcomes. It **does not copy free-form notes, task text, paths, checker names, commands, logs or external captures**. This deliberately avoids trying to detect arbitrary secrets or answer keys with a denylist. A human or authorized coding agent can add a disclosure-reviewed lesson before committing the note. This bounds automation: the draft is useful as a source-linked investigation entry, but does not synthesize a private repair narrative. Actor identity and supplied results are always labeled self-reported. Content hashes are identifiers, not encryption; retain receipts privately.
