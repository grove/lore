# Source-pinned real-project decision controls

Seven public **development regression cases** use nine annotated passages from
two real projects: Python's PEP repository and Go's published compatibility
policy. These are source-attributed documentary cases, with implementation-agent
labels. They are not synthetic upstream documents, independently reviewed gold,
held-out tasks, or evidence of a coding-agent productivity gain. The independent
controlled study in [#2](https://github.com/grove/lore/issues/2) remains separate.

## Reproduce offline

From the repository root, with Python 3.10 or newer:

```sh
python3 -m unittest discover -s evaluation/tests -p test_real_decision_corpus.py -v
cargo test --locked --test real_decision_corpus
```

The Python tests verify complete file inventories, SHA-256 pins, exact cited
line ranges and excerpts, documentary status witnesses, case completeness, and
exclusion from independent evaluations even after relabeling a task. Both the
coding-task and cross-source evaluators recognize these public source snapshots
as development fixtures. Recognition is an exact-snapshot guard; modified or
partial copies still require honest development/holdout declarations.

The Rust test initializes isolated registries, then captures the annotated
passages through the existing source/evidence storage APIs. RST and HTML source
files are copied **byte for byte**, with a `.md` suffix only in the temporary
workspace, so ordinary status/audit can inspect their content identity. Original
upstream paths, hashes and line numbers remain in `manifest.json`. No RST/HTML
parser, extraction-quality claim, provider simulation, or benchmark runner is
added. The authored source windows and labels supply extraction; the explicit
PEP replacement annotation supplies the grounded relation. Registry IDs are
created normally and resolved against each manifest record during the test.

For every task, the test calls the real `context --fast`, `decisions`, and `cases`
CLI commands. Required records and evidence survive selection at the declared
24,000-token context budget (48,000 for the larger lens/case envelopes). A
512-token check requires disclosed omissions, complete retained excerpts and an
all-or-none supersession pair. Rejected/superseded lifecycles and source scopes
remain distinct from current documentary support. Two unrelated-query controls
select no records. Every original excerpt resolves through `lore evidence`, both
projects pass `lore audit`, the registry bytes remain unchanged during reads,
and model-call counters remain zero.

This intentionally generous positive budget tests evidence mechanics, not
default-budget recall. The adjacent critical retrieval controls are described
by their [manifest](../../../tests/fixtures/critical-retrieval/manifest.json)
and `tests/critical_retrieval.rs`; decision applicability and default rendering
remain covered by the existing tests. Run the complete offline suite with
`cargo test --all-targets --locked` and
`python3 -m unittest discover -s evaluation/tests -v`.

## Upstream pins and licenses

| Project | Immutable revision | Retained originals | Redistribution |
| --- | --- | --- | --- |
| [Python PEPs](https://github.com/python/peps/tree/145bee803f08ed68c225c185dc40d4f40a089e1c) | `145bee803f08ed68c225c185dc40d4f40a089e1c` | `peps/pep-0622.rst`, `peps/pep-0634.rst`, `peps/pep-0640.rst` | Each complete PEP retains its Copyright section: public domain or CC0-1.0, whichever is more permissive. |
| [Go website](https://github.com/golang/website/tree/e27b2f73fde2565bc4a18dcb6a5a71db27769757) | `e27b2f73fde2565bc4a18dcb6a5a71db27769757` | `_content/doc/go1compat.html`, `LICENSE` | BSD-3-Clause; complete upstream copyright, conditions and disclaimer retained. |

`manifest.json` binds every retained byte, source/status passage, scope, expected
lifecycle, task, required record, included condition, negative case, and unknown.
Paths are relative to each project's `source_root`. Citations use one-based,
inclusive upstream file lines; excerpt hashes cover the exact quoted UTF-8 text
without its final newline. To independently refresh provenance, fetch these
repositories and compare the listed files at these exact commits, rather than
reading their moving default branches. `.gitattributes` preserves original bytes
on every platform. This corpus does not automatically update upstream pins.

`Final` in PEP 634 maps to documentary acceptance, without implying runtime
verification. PEP 622 is explicitly superseded; its earlier acceptance is not
asserted. Go's policy is labeled active, without inventing a formal acceptance
vote. Creation dates and capture times are not effective deployment dates.

## Case reports

The completed replay passed on the hardening base
`1f2fe93717ca3d5c7a01af9c34159b8fb52be412` plus this corpus/test change. No
production correction was made for these cases. **Before #81–#84: not replayed**
for every row; the following after observations establish regression contracts,
not a measured before/after improvement or independent semantic assessment.

| Case | Pinned documentary evidence and expected scope | After observation | Must remain unknown |
| --- | --- | --- | --- |
| `explicit-supersession` | PEP 622 header and abstract; PEP 634 header/abstract explicitly replace 622. Python pattern matching only. | Both records selected; old lifecycle stays superseded with current source support; tight budget cannot split the replacement group. | Interpreter compliance, deployment; whether 622 was previously accepted. |
| `rejected-unused-syntax` | PEP 640 header/rejection and proposal abstract; PEP 634 capture/wildcard rules. | Rejection and accepted wildcard conditions remain inspectable together. | Any fork's implementation; a prototype does not prove acceptance. |
| `unspecified-partial-bindings` | PEP 634 match semantics explicitly leave failed partial bindings unspecified. | The entire two-sided warning survives: assume neither bindings nor unchanged variables. | Actual bindings in any particular interpreter execution. |
| `source-versus-binary` | Go policy introduction distinguishes intended source compatibility from binary compatibility. | Source scope, no binary guarantee, and required recompilation remain one complete excerpt. | Whether a concrete binary links or runs. |
| `security-exception` | Go expectations and security exception, alongside the source-compatibility passage. | General intent and conditional permission to break compatibility are selected together. | Necessity or deployment of a particular fix; unrelated breaking changes are not authorized. |
| `keyed-struct-exception` | Go expectations for fields added to exported structs in another package. | Keyed preservation and unkeyed failure remain together. | Whether any specific struct changed or application compiles. |
| `no-performance-promise` | Go expectations explicitly disclaim performance guarantees between releases. | Complete limitation is selected; case views claim zero runtime execution/tests. | Latency, measurements, causation, or any actual regression. |

The initial fixture validation found a cited end line that included a trailing
blank line outside the trimmed excerpt. All ranges now terminate at their exact
last quoted line; both Python integrity and Rust resolver checks enforce that.
There were no retained selection misses at the declared positive budgets.
Tiny-budget omissions are expected and asserted, rather than scored as success
at supplying the omitted evidence.

## Review, exclusions, and hardening boundaries

Separate human source-label review is **pending**; no reviewer or agreement has
been fabricated. The manifest retains author identity, label qualifications,
empty reviewer notes and an empty disagreements list. A later reviewer should
record identity, cited source, proposed label and any disagreement there instead
of silently replacing the author's annotation. Until then these remain
author-labeled controls, not independently established ground truth.

The manifest also records excluded uncertain cases: an accepted rule versus a
later unaccepted proposal, documentation/implementation disagreements, and
independent review. No matching source-code or authentic OpenWiki snapshot was
curated, so **#81 freshness and implementation truth are not measured here**.
Existing OpenWiki import/freshness tests remain the relevant offline contracts.
Selection/omissions relate to #82; lifecycle/scope preservation relates to #83.
For #84, these cases exercise deterministic views only; default schema-5
recommendation/rendering behavior remains covered by the existing golden tests,
not a new real-model measurement.

No live Codex or hosted-provider exploration was run (`explorations: []`). Any
future exploration must be recorded separately with exact model identity,
settings, failures and source pins; it must not overwrite these deterministic
controls or enter the independent held-out benchmark pool. The
[decision-work controls](../../DECISION_WORK_09.md) and
[cross-source methods](../../CROSS_SOURCE.md) retain their original study gates.
