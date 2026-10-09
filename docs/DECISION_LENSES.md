# Decision lenses and documented cases

`lore decisions QUERY` explains the relevant retained choices through their
original records and exact documentary passages. `lore cases QUERY` presents
documented procedures, boundary cases, rejected approaches, risks, reported
outcomes, and source-owned work history. Both commands are deterministic,
read-only views of the shared registry. They require no model provider.

```sh
lore decisions "dispatch queue capacity"
lore decisions "dispatch queue capacity" --max-tokens 12000 --json
lore cases "dispatch ordering during failover" --json
```

The library entry points are:

```rust
pub fn decisions(
    conn: &rusqlite::Connection,
    query: &str,
    max_tokens: usize,
) -> anyhow::Result<DecisionLenses>;

pub fn cases(
    conn: &rusqlite::Connection,
    query: &str,
    max_tokens: usize,
) -> anyhow::Result<CaseResult>;

pub fn render_decisions(result: &DecisionLenses) -> String;
pub fn render_cases(result: &CaseResult) -> String;
```

## Source ownership and lifecycle

Each response uses schema version 1 and contains the unchanged schema-2
`context` selected by the established context retrieval and group assembly.
That context keeps the original record IDs, source evidence, relationship
endpoints, scope, classifications, disagreements, qualifications, and imported
native evidence. A `registry_revision` identifies the shared documentary and
imported state used by the entire response. The query runs in one SQLite read
snapshot and writes no knowledge or cache records.

A decision lens retains its complete `ContextItem`. Its `original_lifecycle`
also preserves the original accepted/proposed/rejected state when an explicit
supersession has changed the effective lifecycle. Its `history` copies the
selected source-backed supersession, reaffirmation, contradiction, and suspected
conflict relationships, with their original witnesses. The command does not
infer replacement from a newer date or a closed issue.

`negative_knowledge` identifies selected rejected records and documented risks.
These preserve their recorded scope and evidence. A risk remains a risk; it
does not become a report of an actual failure. A lens links negative records to
its decision only when they are the same record or are joined by a selected
explicit documentary relationship. Relevant negative records also remain
available together with the shared context.

## How explanatory passages are attached

The registry contains immutable evidence snapshots and captured Markdown
heading paths. The command uses those existing records to organize documented
starting conditions, rationale, alternatives, trade-offs, assumptions,
constraints, applicability conditions, exceptions, expected behavior, reported
outcomes, consequences, reconsideration triggers, rejected approaches, and
failure modes.

A supplemental passage must belong to the same captured source and the same
structural decision or case family. For example, `Dispatch / Decision` can use
`Dispatch / Rationale`; it cannot use `Database / Rationale` from another section
of the same meeting minutes. Current support can join other current retained
passages from that same family. Historical support only joins its own captured
source revision. An explicitly labeled line such as `Rationale:` is another
eligible organization hint. Generic phrases such as “because” do not manufacture
a rationale field.

A synthetic `Document` or `Preamble` family does not establish a boundary
between choices in an unstructured file. In such a family, an explicit marker
can organize the decision's own complete quote; it cannot attach a different
record's rationale simply because the records share a file.

Every `DocumentedFacet` contains:

- The category, its `classification_basis`, and the captured `heading_path`.
- Exact source-owned record classifications, lifecycle, original lifecycle,
  scope, and support state. A proposed alternative keeps its proposed status
  when displayed beside an accepted decision.
- An `evidence_id` resolving to the same original `EvidenceSnapshot.id`.
- The complete retained `EvidenceSnapshot`: exact excerpt, digest, original
  source and source revision, path, line range, captured provenance, and bounded
  surrounding context.
- A `current` flag describing retained documentary support.

The excerpt digest is checked before a supplemental passage is returned. The
command never cuts a quote at a convenient sentence or truncates a procedure or
table to hide a later condition. A source passage can contain multiple concerns:
the organizing label does not change or remove any of that passage's text.

Heading labels are structural organization hints, not independent proof that
the source's explanation is correct. Missing fields remain missing. They do not
establish that the original source has no rationale or exceptions: extraction
may not have retained such a passage. Full source documents are not stored or
reconstructed by these commands. The response therefore always reports
`coverage.complete_source_documents: false`.

## Cases retain their evidentiary meaning

Cases have stable IDs derived from the original record and its evidence IDs.
Each documentary case retains its source record and any structurally associated
facets. Imported cases retain their unchanged `ContextObservation`; their native
source payload and provenance remain in `context.imported_evidence`.

| Case kind | Meaning |
| --- | --- |
| `documented_procedure` | A source describes an operational sequence. The exact sequence and its retained conditions remain available. |
| `source_reported_outcome` | A source reports an outcome. Lore did not reproduce it. |
| `source_reported_observation` | A source describes an observation. It is not a checkout observation made by this command. |
| `rejected_approach` | The retained record explicitly has a rejected lifecycle. |
| `documented_risk` | A documented risk, without an invented failure history. |
| `boundary_case` | A constraint has structurally associated exceptions or failure modes. |
| `work_history` | A documentary issue-state record or imported work-state observation. Native closure retains its native meaning. |
| `imported_report` | Another source-owned imported observation, with its recorded verification qualification. |

Starting conditions, expected behavior, reported actual behavior, and reported
failure rationale appear as separate categories when the source retained them.
An `Actual behavior` heading becomes a **source-reported outcome**. It never
becomes independent runtime evidence. There is no generated execution trace.

Both commands report `model_calls: 0`. Cases additionally report
`runtime_execution: false` and `tests_run: 0`. These APIs accept a registry
connection, not a checkout or execution capability. They perform no checkout
inspection, command execution, model inference, or network request. A real
runtime result must come from an independently authorized execution path and
must not be inferred from a case's title, lifecycle, or upstream verification.

## Budgets and completeness

`--max-tokens` must be between 512 and 100000. The shared retrieval receives half
the output allowance, with its existing minimum budget. The rest is available
for explanatory views and their original source context. The full result's
`budget.used_tokens` conservatively accounts for the larger of its serialized
JSON (including the final newline) and Markdown rendering using `cl100k_base`,
including source evidence, nested context, qualifications, metadata, and the
budget itself. Accounting grows monotonically so a one-token oscillation in the
printed count cannot understate the final output.

At most 12 decision lenses or 16 cases are returned. A source family containing
more than 64 distinct eligible facet evidence snapshots is omitted as a complete
view. When the full response exceeds its token allowance, complete lenses or
cases are removed; facets inside a retained lens are never selectively removed.
The unchanged shared context remains available. `coverage` discloses whole-view
omissions, candidate-limit exhaustion, selected record count, and inherited
retrieval truncation. If even the complete nested context and minimal response
envelope cannot fit, the command returns the typed `invalid_budget` error.

These bounds control response construction and presentation. Registry retrieval
still uses the existing registry-wide retrieval implementation; this feature
does not introduce a new storage-scale performance guarantee.

## Validation and remaining outcome gates

`tests/decision_lenses.rs` exercises real retained SQLite records and source
snapshots. Its contracts cover exact rationale and rare table conditions;
same-document unrelated-decision isolation; absent-rationale abstention;
proposed, rejected, and scoped alternatives; explicit supersession history;
distinct expected and source-reported actual outcomes; risk versus observed
failure; native closed-work evidence; whole-view budget pruning; deterministic
read-only behavior; and Unicode/CRLF quote preservation.

These are source-integrity and behavior contracts. They do not show that a
person understands the project better, completes a first task sooner, or
transfers the lesson to a distinct task. The independent source review and real
human outcome gates in `evaluation/SHARED_INTELLIGENCE.md` remain separate and
unmeasured until the corresponding reviews and participant results are recorded.
