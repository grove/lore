# Provider usage and cost accounting

Lore 0.8 records provider attempts and keeps unknown token usage and billing
unknown. A returned answer, an unsuccessful request and a local cache hit have
different accounting. Offline response-envelope tokens used by `--max-tokens`
are never substituted for provider tokens.

## Values and aggregation

Generation, embedding and decision responses have optional `ProviderUsage`.
Each recorded attempt includes these fields:

```json
{
  "provider_request_count": 1,
  "input_tokens": 1200,
  "output_tokens": 250,
  "total_tokens": 1450,
  "billed_cost_usd": null,
  "billing_source": null,
  "status": "completed",
  "cache_hit": false
}
```

`provider_request_count` counts dispatched HTTP attempts, including retries. A
transport failure can occur before a provider receives the request, so it is
not proof of billable execution. `model_calls` counts distinct logical calls;
retries share a call ID. A custom Rust adapter without optional usage still
contributes a logical call, with unknown physical requests and token usage.

Token counts are nonnegative integers or `null`. OpenAI Responses uses
`usage.input_tokens`, `output_tokens` and `total_tokens`; OpenAI embeddings
uses `usage.prompt_tokens` and `total_tokens`. Ollama uses `prompt_eval_count`
and `eval_count`, with no generated tokens for embeddings. An omitted total is
derived only when both component counts are valid. Invalid types, inconsistent
totals and overflow remain unknown. TypeSafe has no assumed accounting mapping.
See the [OpenAI Responses reference](https://developers.openai.com/api/reference/resources/responses/methods/create)
and [Ollama API documentation](https://github.com/ollama/ollama/blob/main/docs/api.md).

These direct API mappings do not provide an invoice. `billed_cost_usd` remains
`null`, including for local models, unless an embedding application supplies an
explicit measured amount with its `billing_source`. No price table or guessed
USD estimate is included. A summary adds every event; one unknown component
keeps that aggregate unknown. A measured no-inference invocation and an actual
cache hit have zero new requests, zero new tokens and zero new provider USD.
Cache events never copy an earlier response's token charge.

Events distinguish `started`, `completed`, `http_error`, `transport_error`,
`timed_out`, `refused`, `incomplete`, `validation_failed`, `cancelled`, `cached`
and `rejected`. An HTTP error body can contain usable token counts even when
the call fails. Such counts survive retry and refusal. Local rejection and
repair update the original attempt outcome and charge each new repair call
once. Preflight denial or invalid HTTP metadata dispatches zero requests.

## Memory, explicit output and SQLite

The default invocation sink is in memory. Context queries do not create an
accounting directory or file. Existing source-only and `--no-cache` behavior
therefore keeps its file-write boundary. Schema 5 adds a compact `usage`
summary; the complete JSON and Markdown, including metadata, remain inside
the requested response budget. Explicit schemas 2, 3 and 4 keep their public
output contract. `init` and `update` reports also include usage.

An invoking host can explicitly request a separate ledger:

```sh
LORE_USAGE_LEDGER=/absolute/existing/audit/lore-attempt-001.json \
  lore context "Implement the requested change" --schema-version 5 --json
```

The parent directory must already exist and the file must be new. Existing
files and symlink paths are rejected. The ledger is caller-owned output, not
a cache; explicitly requesting it also requests its writes with `--no-cache`.
Unix output files are private to their owner. The sink saves a started event
before dispatch and updates it as the attempt completes. A dropped pending
request is cancelled; abrupt process termination can leave a started event
with unknown usage and a running invocation. A captured failed invocation
must never be interpreted as a successful zero-cost request.

The version-1 `lore.provider_usage` envelope contains `invocation_status`,
`summary` and `events`. Events retain bounded provider/model identifiers,
operation, logical call and attempt IDs, status, HTTP status, duration and
optional accounting. They exclude prompts, source text, response text,
provider error bodies, endpoint URLs and credentials. The summary is
independently recomputable from these events.

A schema-5 summary can reference the selected path and a SHA-256 binding to
its event prefix. Later phases may append events without invalidating that
earlier summary. The event hash uses sorted JSON object keys, UTF-8 and no
whitespace; for hashing only, each non-null billing number is replaced with
the lowercase 16-digit hexadecimal IEEE-754 binary64 representation. This
avoids JSON number-format differences between Rust and Python. The ledger's
public billing value remains a nullable JSON number. Consumers must read
only their own selected output path, never follow a path supplied in an
untrusted response.

Successful compiler runs save the same non-sensitive events in SQLite schema
8 (`provider_usage_runs` and `provider_usage_events`) within the run's existing
publication transaction. Migration from schema 7 is transactional and leaves
historic `model_calls` rows intact. Missing historic usage is `None`/unknown;
a measured run with no events is explicitly distinguishable. No historical
token counts or prices are backfilled. Failed compiler work remains available
in the invocation sink or explicitly requested ledger, not an incomplete
published database generation. Older binaries cannot write schema 8; retain
the pre-upgrade project backup when a downgrade is required.

Library callers can use `UsageSession::memory().scope(future)` and then read
`events()` or `summary()`. `HttpModel::usage_events()` also exposes the client's
attempts without a surrounding CLI scope. `usage::generate`, `embed` and
`decide` include custom trait adapters without double-counting an instrumented
HTTP client. Trait methods remain object safe. Custom response struct literals
can use `usage: None` until their adapter provides measured accounting.

## Evaluation and failure retention

The coding and adaptive evaluators select a fresh ledger path for each Lore
invocation and coding subprocess. They capture exact-file and canonical-content
hashes, recheck the event sum and bind any public usage summary to its captured
events. The coding adapter discards usage-looking fields inside generated
answers and derives accounting from the provider response envelope. Its
`--retries` option defaults to zero; enabling retries retains every attempt.

Preparation, context warm-up, served context and every coding repair attempt
retain separate accounting. Checker time, retry waiting and other observed
invocation wall time stay in the existing cost phases. Unknown token or USD
components make the all-in total unknown. Repeated same-task cache requests
demonstrate no new inference; they do not demonstrate correctness or reuse
across different tasks.

An initial `metrics.json`, `sample-starts` identity records and per-attempt
invocation journals survive interrupted studies. Failed or cancelled coding
subprocesses retain their exact captured usage before the exception is
re-raised. Such records support an explicit incomplete-study denominator;
they are never converted into successful checked implementations. Historical
binaries or external adapters that omit the optional ledger remain visibly
unmeasured rather than receiving invented usage.

Offline tests cover local HTTP success/error/retry/refusal/timeout/cancellation,
malformed counts, cache charges, extraction rejection and repair, migration,
canonical cross-language hashes and falsified aggregate rejection. They make
no real-model quality, throughput or billing claim.
