# Adaptive coding-task comparison

`adaptive_tasks.py` connects the explicit schema-5 experience to the existing
actual coding-task runner. It invokes the operator's coding-agent command,
applies its proposed changes to a separate source copy, and runs the selected
independent correctness and constraint checks. It also creates bound, blinded
review packets for two reviewers per attempted implementation.

This is executable evaluation infrastructure. The checked-in tests use labeled
offline protocol doubles and actual agent/checker subprocesses; they establish
neither real model quality nor better coding productivity. No real adaptive
coding-agent outcome has been measured by adding this protocol.

## Comparison arms

| Arm | Context supplied to the coding agent |
| --- | --- |
| `baseline` | The original source files and task, with no Lore context. |
| `fast` | Explicit `lore context --fast`, schema 2. |
| `lore05` | Explicit `lore context --schema-version 3`. |
| `lore06` | Explicit schema 4; inspection and investigation are requested when the study grants inspection. |
| `adaptive_no_reuse` | Explicit schema 5 with `--no-cache` on both context requests. |
| `adaptive_reuse` | Explicit schema 5 with its normal disposable cache policy. |

The existing `coding_tasks.py` and `decision_tasks.py` commands keep their
original arms and behavior. The new module reuses their source preparation,
agent input format, executable verification, evidence resolver and review
validation. It reuses the shared experience's schema-5, source-relationship and
exact static-observation validators.

**The public no-reuse control is `--no-cache`.** It disables combined adaptive
response and investigation reuse, and can affect other optional cache paths.
This experiment cannot attribute a difference solely to investigative memory.
A separate memory-only ablation would need a narrower production control.

### Matched warm-up and served request

Each case is initialized once. Every Lore arm receives a separate byte-identical
copy of that initialized project, including the same retained registry and
model configuration. No arm reads another arm's disposable cache.

Within each arm, the runner first requests context for the unsolved task, then
requests it again with exactly the same arguments. The second response is the
one supplied to the coding agent. The first response is retained as the
`warmup_response`; it is never an earlier coding answer or a solved patch. Both
requests occur before any coding agent or checker runs for that case. Context
arm order and coding-agent arm order are separately shuffled, with the context
collection order recorded in each sample.

This measures **same-task reuse**. It does not establish reuse on a different
question, reuse after source mutation, or reduced investigation across an
iterative coding session. The existing Rust source-mutation and permission
tests cover freshness contracts separately.

The independent checker and answer keys should be outside the selected source
tree, as in the bundled manifests. The runner never adds checker programs,
checker results or candidate patches to warm-up requests. Original project
files remain inputs to all arms; an operator who puts answer keys or the
independent checker inside that input tree has compromised the study. Source
fingerprints and the retained agent request make that input discipline
reviewable; this is not a filesystem sandbox.

## Commands

Preparation uses no model or Lore subprocess:

```bash
python3 evaluation/adaptive_tasks.py prepare \
  --output evaluation-results/adaptive-prepared-01
```

For an actual local-model run, build Lore and provide an available Lore model
and a coding-agent model. Replace the absolute adapter path and model
placeholders below. The coding-agent command must be a JSON argument array;
it is executed from a separate, initially empty working directory.

```bash
cargo build --release --locked

python3 evaluation/adaptive_tasks.py run \
  --lore-binary target/release/lore \
  --provider ollama --model YOUR_LORE_MODEL \
  --allow-inspection \
  --agent-command '["python3", "/ABSOLUTE/PATH/TO/lore/evaluation/coding_agent.py", "--provider", "ollama", "--model", "YOUR_CODING_MODEL"]' \
  --agent-id 'YOUR_CODING_MODEL; pinned settings; file-proposal adapter' \
  --agent-location local \
  --max-tokens 6000 --timeout 3600 \
  --output evaluation-results/adaptive-local-01
```

Use `--cases /absolute/path/cases.json` for independently reviewed projects.
The schema-1 manifest and executable check format are the same as the
[existing coding-task protocol](INTELLIGENCE.md). A case identifies the source
snapshot, task, allowed editable files, critical conditions, and independent
test command. The source-copy limits and path protections are inherited from
that protocol. Public bundled fixtures remain recognized by source content
even if their labels claim that they are held out.

Provider, decision model, optional embedding model, role-specific endpoints and
reasoning options are passed through the existing configuration builder. These
settings stay identical across the copied Lore arms. The coding-agent command
and declared identity remain identical across all six arms; reported provider
model differences are rejected when those identities are available. A generic
external adapter's model, tools and actual inference are still operator
attestations, not authenticated facts.

Assessment performs no inference and does not rerun a checker:

```bash
python3 evaluation/adaptive_tasks.py assess evaluation-results/adaptive-local-01
```

`prepare` exits successfully after creating a fresh preparation directory.
`run` and `assess` exit with code 0 when the mechanical comparison is complete,
code 2 when its mechanical gates fail, and code 1 for invalid input or failed
execution. A passing mechanical exit code does not mean that a coding task
passed, a source interpretation was correct, or a productivity benefit exists.

## Permissions and comparable inputs

Every Lore, agent and checker subprocess receives its own environment
dictionary. Ambient `LORE_INSPECTION_ROOT`, `LORE_ALLOW_HOSTED_EGRESS` and
`LORE_ALLOW_CHECKOUT_EGRESS` are removed before applying this run's options.
The parent process's environment is never changed. Credentials can be
inherited privately; reports contain only the declared Lore grants, not the
complete environment.

`--allow-inspection` sets a root for the particular copied project or candidate
workspace. Without it, schema 4 and schema 5 receive `--no-inspect` and the
assessor rejects ungranted static observations. `--allow-hosted` supplies the
hosted Lore grant; a hosted coding-agent adapter must also receive its own
documented opt-in. `--allow-checkout-egress` requires both study options and
adds the same checkout consent to both adaptive arms. Repository configuration
cannot broaden these grants.

These are **declared Lore permissions**, which the Lore implementation consumes.
Environment variables do not sandbox the external coding agent or independent
checker. Their selected commands execute with the operator's privileges.
Filesystem, network, tool restrictions and actual model execution require
independent enforcement or accountable capture review outside this harness.
The generated `INTEGRITY_AUDIT.json` starts unmeasured and requires bound capture
files before an audit can pass.

The assessor checks source content hashes, copied configurations, cold registry
identity, distinct context copies, equal output budgets, exact declared grants,
and equal effective schema-5 capabilities. Both adaptive responses must name
the same project and registry revision. The agent receives the complete served
context and original files through the existing bound request contract. The
maximum token option limits Lore output, not the total original-source payload
or an arbitrary external agent's tools. Keep those external budgets fixed in
the pinned agent command.

Existing OpenWiki, Engram and Beads imports remain available through the same
case preparation. This six-arm protocol does not create a separate OpenWiki-only
agent arm or isolate the cost of producing upstream snapshots. The
[cross-source evaluation](CROSS_SOURCE.md) retains its separate source/tool
comparison; a study that claims an OpenWiki-specific agent benefit needs matched
upstream-only task attempts too.

## Recorded measurements and interpretation

Each context record keeps the complete warm-up and served responses, hashes,
CLI arguments, declared grants, exact evidence resolutions, original source
and registry state, and disposable-cache fingerprints. Generated static
observation references are checked against exact source bytes and line ranges.
The additive source-relationship manifest is checked against retained resolver
responses, preserving native qualifications and documentary revision bindings.

Each task retains its exact proposed files, implementation manifest, agent
request hash, checker executable/helper hashes, independent correctness and
constraint results, and reviewer bindings. Assessment derives those checks
again from the saved artifacts; editing a recorded `checks` boolean is
insufficient. It does not authenticate a researcher's wholesale replacement of
an entire report and its source artifacts.

| Measurement | Meaning and limit |
| --- | --- |
| Passed tasks and failed constraint checks | Actual selected checker results against the submitted implementation. A failed task can coexist with a mechanically valid comparison. |
| Material mistakes and missed conditions | Concrete annotations averaged across at least two independently identified blinded reviewers. Review identity, independence and judgment remain accountable attestations. |
| Warm-up and served model calls | Calls reported by the real CLI for each request; the schema-5 envelope is unwrapped correctly. Unknown provider tokens and actual billing stay null. |
| Complete context size | Full Python-serialized response byte size plus the CLI's reported whole-output token budget. The byte metric is not a token estimate or a second independent tokenizer. |
| Served context latency | Wall time for the second CLI request. It includes the selected command's retrieval, inspection and revalidation work; no latency threshold establishes a win. |
| Warm-up-inclusive recorded cost | Initialization is charged in full to each Lore arm; context-copy time, both context requests, agent time and verification time are included in their recorded phases. The baseline has no Lore initialization charge. |
| Guidance cache hits | A hit must match the unsolved warm-up revision and non-prunable decision premises, retain the same trace, have saved cache data, and report zero served model calls. Complete optional items may pack differently; both responses undergo full source validation. Schema 3 retains its older public preferred-approach comparison. A cache hit is not a correctness result. |
| Retained investigation reuse | A served hit with `cache_reused` and a retained nonempty investigative trace. Merely enabling cache, returning fallback, or retaining no trace does not count. |

The summary reports signed reuse-minus-no-reuse differences in served context
calls, served context time, warm-up-inclusive recorded task time and passed
tasks. Negative or inconclusive findings remain visible. A missing cost
component remains unknown; calls do not imply billed dollars or provider token
counts. Review completeness and independently reviewed task attempts are
separate from fixture results.

This remains one coding attempt per arm. It does not measure correction loops,
time to a correct final contribution, mentor effort, human learning, provider
server-cache effects, or avoided investigation across different tasks. It
does not automatically announce a productivity win. Three distinct held-out
projects, genuine intelligent responses, actual coding calls and complete
independent review are prerequisites for its reviewed-task status; broader
productivity claims need a declared study and the missing longitudinal outcomes.

## Offline contract gate

```bash
python3 -m unittest discover -s evaluation/tests -p test_adaptive_tasks.py -v
python3 -m unittest discover -s evaluation/tests -v
```

The focused fixtures exercise all six arms, positive and negative cache
bookkeeping, per-subprocess environments, exact observations, source-relationship
qualification, tampered arguments/grants/snapshots/requests/costs, actual failed
independent checks, and refusal to promote public fixtures to model-quality
evidence. No model credential is required, and these tests create no live-model
outcome report.
