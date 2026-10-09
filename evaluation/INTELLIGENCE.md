# Lore 0.5 coding-task evaluation

The 0.5 extension compares **completed implementations**, not just retrieved
text. It reuses `benchmark.py` for model configuration and real Lore execution,
and `cross_source.py` for copied snapshots, byte fingerprints, and exact
evidence resolution. The original four-setup cross-source study remains
available; its Lore calls now explicitly use `--fast` to preserve the 0.4
model-free contract.

| Setup | Coding-agent input |
| --- | --- |
| `baseline` | The task and original project sources, including the same supplied upstream snapshots |
| `fast` | Identical task and sources plus `lore --json context TASK --fast` |
| `intelligent` | Identical task and sources plus default intelligent `lore --json context TASK` |

The agent model, execution settings, allowed edits, test command, and task
remain fixed across the three setups. Setup execution order is shuffled.
Each implementation starts from a fresh copy of the same source bytes.
The benchmark does not give the baseline fewer underlying source files.

## What is included

Three small Python tasks extend the existing cross-source snapshots:

| Task | Executable checks | Human review |
| --- | --- | --- |
| Payment retries | Success, reuse of the idempotency token, no retries for declines, preservation of the existing five-retry count and final exception | Does the explanation preserve the unresolved three-versus-five policy distinction and recommend a useful follow-up? |
| Ledger transfer | Successful atomic transfer, rollback on failed writes or commit, rejection of invalid amounts, current database architecture | Does the approach apply the replacement ADR and qualify old memories? |
| Release admission | Signed artifact admission, production signature requirement, fixture-only staging exception, unknown environments rejected | Does the reasoning preserve scope instead of treating staging history as production policy? |

The payment task explicitly preserves the observed behavior for this limited
code change. It does not mark the accepted three-retry ADR as replaced or
authorize another increase. The separate human review checks that distinction.

These are **synthetic development fixtures**. Their source manifests force
fixture-only status when prepared with the bundled cases. They cannot establish
independent product benefit. The executable checks cover their stated
behaviors; a passing implementation is not proof of general code quality.
Known merged fixture fingerprints are also recognized when someone supplies
their bytes through an external manifest; relabeling them does not make them
independent projects.

## Prepare without models

```bash
python evaluation/coding_tasks.py prepare \
  --output evaluation-results/coding-prepared
```

This copies snapshots and records exact source fingerprints. No model, coding
agent, or candidate program is executed. The hidden fixture checks stay outside
the agent's input snapshot. A nonempty or existing output directory is refused.

## Run a real comparison

Build Lore, make your chosen local models available, and supply a coding-agent
command. The included `coding_agent.py` is a minimal real Ollama/OpenAI adapter;
it requires no third-party Python packages. Replace the absolute adapter path
and model identifiers in this example:

```bash
cargo build --release --locked

python evaluation/coding_tasks.py run \
  --lore-binary target/release/lore \
  --provider ollama --model YOUR_LOCAL_LORE_MODEL \
  --agent-location local \
  --agent-id 'ollama/YOUR_PINNED_CODING_MODEL; default settings' \
  --agent-command '["python3", "/absolute/path/to/lore/evaluation/coding_agent.py", "--provider", "ollama", "--model", "YOUR_PINNED_CODING_MODEL"]' \
  --output evaluation-results/coding-local-01
```

Use `--embedding-model YOUR_EMBEDDING_MODEL` to include semantic retrieval.
Omit it to measure reasoning over lexical retrieval and existing relationships.
Run separate, freshly labeled output directories for the two configurations;
their difference isolates retrieval from synthesis. The embedding role uses
the same provider and endpoint as the selected generative role in this runner.

The existing generative/decision endpoint and reasoning options are supported,
including `--reasoning-context-synthesis` and
`--reasoning-context-verification`. A generative or decision OpenAI provider
requires `--allow-hosted`. A hosted coding agent additionally requires
`--agent-location hosted` and `--allow-hosted` on the runner. The bundled
coding adapter itself also requires its own `--allow-hosted` flag when its
provider or endpoint is hosted. Credentials stay in environment variables.

The adapter accepts `--provider openai --model YOUR_RESPONSES_MODEL`, optional
`--base-url`, `--api-key-env`, and `--reasoning-effort`. Pin and record the same
coding model and settings across setups. It extracts provider-reported token
usage without turning that usage into invented USD charges. Its local-only
endpoint must use a literal loopback address; redirects and ambient HTTP proxies
are disabled so a local request cannot silently forward source text elsewhere.
Non-loopback endpoints require HTTPS even with hosted opt-in, and recognized
Ollama cloud tags require hosted opt-in even on loopback. OpenAI requests set
`store: false`. Incomplete, refused, truncated, or tool-call responses are rejected
before they can be counted as coding completions.

**Execution boundary:** the chosen agent command and verification command run
as ordinary subprocesses with the invoking user's privileges. Candidate Python
code is executed by the fixture checks. Input snapshots and allowlisted output
files provide reproducibility, not an OS sandbox. Run untrusted candidate code
inside an independently isolated execution environment. The harness never
silently installs an agent or executes a command merely because source text
contains it.

## Coding-agent protocol

`--agent-command` is a JSON array of executable and arguments, with no shell
interpolation. Use an absolute adapter path: the agent starts in a separate
empty working directory. Its standard input contains one JSON object with:

| Field | Meaning |
| --- | --- |
| `schema_version` | Integer `1` |
| `task` | Identical requested change for every setup |
| `files` | Relative path to original UTF-8 content, with identical source bytes across setups |
| `editable_files` | Exact paths the agent may replace |
| `context` | `null` for baseline, the complete actual Lore response otherwise |
| `response_contract` | Explanation of the required response format |

The agent writes one JSON object to stdout and exits successfully:

```json
{
  "schema_version": 1,
  "files": {"src/implementation.py": "complete proposed file contents\n"},
  "summary": "Explain the implementation and material assumptions.",
  "usage": {
    "model_calls": 1,
    "input_tokens": 1200,
    "output_tokens": 240,
    "billed_cost_usd": null,
    "billing_source": null
  }
}
```

These numbers illustrate the protocol, not measured results. Unknown usage
fields must be `null` or omitted. Measured billing requires a nonnegative
finite amount and a nonempty `billing_source`. Nonzero request counts are
never converted to dollar estimates. The runner validates file paths, allowed
edits, output limits, JSON, and usage fields before applying proposed contents.
It rejects attempts to overwrite source documents or the evaluator's tests.

The agent may instead be an existing coding-agent wrapper following this
protocol. Agent-reported model identity, calls, token usage, billed charges,
and genuine model execution remain operator attestations; running a subprocess
does not prove the wrapper actually called the advertised model. The bundled
unit tests explicitly use fixtures, never present their fake provider records
as live inference, and cannot pass independent validation.

## Measurements and integrity

For each task, the collector invokes both fast and intelligent context twice.
It records first-call and repeat latency separately, the full responses and
their hashes, cache status, call counts, evidence resolver results, and the
registry before and after retrieval. Fast context must be deterministic,
model-free, and leave the complete database and wiki bytes unchanged.
Intelligent context must preserve those source-registry bytes too; its
disposable caches live separately. An intelligent repeat must report a cache
hit and the same preferred approach without another model call. Other displayed
items can vary at a tight token boundary because cache metadata takes space.

Every cited evidence ID in both context responses is resolved through the real
`lore evidence` command. Explicit `ev_...` and `ne_...` references in the coding
answer are also checked. Assessment derives citation coverage again from the
full responses and complete per-ID resolver records. A boolean saying that
citations passed cannot replace those records. Registry byte hashes detect
payload changes even when identities and row counts stay unchanged.

`mode: fast_fallback` is retained and reported as a fallback. It may be useful
for the coding task, but it never contributes to the count of tasks that
received intelligent guidance. A fallback cannot establish intelligent benefit.

Execution checks are stored individually as correctness or constraint checks.
Failures count as failed task outcomes; the evaluator does not convert them
into a successful implementation because the agent summary sounds plausible.
Source-locator coverage is recorded separately and is only a retrieval proxy.
Human reviewers assess the meaning, usefulness, unsupported authority, and
implementation quality that fixture checks cannot establish.

Costs include full Lore preparation, initial context, coding execution, and
verification latency. The two Lore setups are each charged the same full
preparation cost for the comparison; the collector physically prepares Lore
once per project. This is a cold-setup comparison without amortization.
Repeat-context probes are retained separately and excluded from task totals.
Existing OpenWiki, Engram, and Beads snapshots are identical supplied inputs;
their original creation cost is **unmeasured**. No claim about total upstream
tool economics follows from this experiment.

Totals remain `null` if any inference-cost component is unknown. Lore's current
context interface reports call counts but not billed charges or provider token
usage, so full USD and token totals will ordinarily be unknown even when the
coding adapter reports its own tokens. Wall time is measured directly. Treat
latency as environment-specific and repeat runs before drawing speed claims.

## Blind human review and assessment

Each run creates `answers/`, anonymous `reviews/`, `metrics.json`, and
`assessment.json`. Give reviewers only `answers/` and `reviews/`, plus a common
copy of the original project sources. Keep metrics and setup assignments out of
the review packet. The answer itself can reveal a setup; reviewers must leave
`blind_confirmed` false if they recognize it.

Reviewers record missed constraints, whether the recommendation was useful,
implementation quality from 0 to 3, high-severity unsupported claims, their
identity/date, and concrete notes. Use at least two reviewers across the study.
The template begins incomplete and contains no generated scores. Its hashes
bind the complete comparison and exact answer bytes. Changing an answer,
source snapshot, tested implementation, setup, model identity, costs, or
context after review invalidates that review.
The selected verification executable, script arguments that identify files,
and optional `verification_files` are fingerprinted too; changes invalidate the
execution binding. Include imported test helpers in `verification_files` when
they affect the checks.

```bash
python evaluation/coding_tasks.py assess evaluation-results/coding-local-01
```

`comparison_complete` describes mechanically complete execution records.
`human_review_complete` is separate. The 20% missed-constraint reduction against
`fast` is a directional measurement, not a fabricated success gate; when fast
misses no constraints the ratio is `null`. Independent validation additionally
requires at least three independently selected held-out projects, distinct
source snapshots, complete reviews, and actual intelligent responses.

For independent work, supply `--cases /path/to/cases.json`. Follow the bundled
manifest schema, replace `base_project`/`overlay` with `source_root`, and specify
the project's `imports` when needed. The corpus must contain a `docs/` source
root. Keep verification scripts outside source inputs. `test_command` is an
argv array; `{python}`, `{manifest}`, and `{workspace}` expand to the Python
interpreter, manifest directory, and isolated implementation directory.
The command must return schema 1 and nonempty `checks`, each with unique `id`,
boolean `passed`, and `kind` equal to `correctness` or `constraint`. These commands
are explicitly selected by the operator, not discovered in imported documents.

Keep synthetic data labeled as such. Project independence, held-out selection,
honest blinding, and test adequacy require researcher judgment. Fingerprints
prevent unnoticed changes; they cannot authenticate those judgments.

## What has and has not been established

Offline tests exercise real subprocess execution of the bundled checks,
initial failing implementations, scoped-policy counterexamples, all three
setup paths, citation and source integrity, anonymous unscored reviews, fallback
classification, protected output paths, unknown costs, and review binding.
Run them with:

```bash
python -m unittest discover -s evaluation/tests -v
```

**No live-model coding score, missed-constraint improvement, independent
comparison, or inference-cost saving is claimed by the checked-in fixtures or
documentation.** Record actual model-enabled results and blind reviews before
making those claims. The 0.4 evaluation history remains in
[BASELINE.md](BASELINE.md); its measurements must not be relabeled as 0.5 results.
