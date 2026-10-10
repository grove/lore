# Evaluating Lore 0.7

Lore 0.7 adds source-reviewed retrieval regressions and reproducible protocols
for coding attempts, human contributions, and longitudinal guardian behavior.
The [0.7 product guide](../docs/V07.md) explains the implemented commands. The
[release scorecard](PRODUCT_QUALITY_07.md) records the current gates, captured
results, and outcomes that remain unmeasured. A prepared corpus, passing checker
control, or synthetic regression is not a completed real-model or human study.

| Evaluation area | Available 0.7 evidence and protocol | Outcome boundary |
| --- | --- | --- |
| Knowledge retrieval | [Knowledge Zoom methods and results](KNOWLEDGE_ZOOM.md), legacy constrained-budget regressions, and 24 pinned ripgrep/fd/jq source cases with a separate AI-agent source-gold review | This checks source bindings, complete conditions, scope, and retrieval regressions. General graph benefit and broad semantic accuracy remain unestablished. |
| Coding-agent work | [Thirty public fault-repair candidates](REAL_CODING_TASKS.md), [six matched configurations](ADAPTIVE_TASKS.md), independent checker controls, bounded repair attempts, and charged investigation/reuse | Candidate tasks are constructed debugging cases, not held-out productivity evidence. Real-model success, time, and cost gains require completed bound attempts and independent review. |
| Intelligence changes | [Failure triage and change validation](FAILURE_TRIAGE.md), including an explicit pilot evidence requirement | An unrun or synthetic study does not justify a claim of an improved adaptive controller. |
| Human onboarding | [Twelve-participant pilot protocol](HUMAN_ONBOARDING.md), opt-in pseudonymous records, first contribution, and distinct transfer tasks | Prepared exercises do not create participants, consent, competence, or learning-transfer results. |
| Project guardian | [Sixty successive debug transitions across three projects](GUARDIAN_LONGITUDINAL.md), original baseline/current source captures, current-support regression, and independent review forms | Both full implementation-session captures failed source integrity; targeted reversion checks passed. Independently reviewed model alert precision and burden remain unmeasured. |

The [recorded guardian evidence](results/guardian-debug-2026-10-10/README.md)
retains all failed checks: the baseline and candidate full captures had 32 and
15 source-integrity failures respectively, when deleted paths reappeared during
read-command intervals. One capture includes an intermediate `.rsync-tmp` path;
the writer was not independently identified. These are failed full integrity
captures. No unavailable provider is scored as a successful quiet guardian.

The existing default context schema 4, deterministic schema 2, and optional
schema 3 remain compatibility controls. Shared context schema 5 is explicitly
selected, and compact exploration schema 2 requires `--compact`. Evaluation
does not silently promote either experimental interface into the default.

## Existing shared, coding, and synthesis protocols

For the explicit schema-5 shared agent experience and schema-1 human orientation,
see [Shared intelligence and learning-transfer evaluation](SHARED_INTELLIGENCE.md).
It collects both actual CLI outputs on the same source snapshot, rechecks
legacy and permission contracts, and provides an external same-project transfer
task whose non-idempotent exception defeats a copied retry solution. Human
source review, actual learning and coding-agent outcomes remain separate.

For matched actual coding tasks with schema-5 reuse enabled and disabled, see
[Adaptive coding-task comparison](ADAPTIVE_TASKS.md). It keeps the legacy arms,
isolates context copies and subprocess grant environments, charges both warm-up
and served context, and reuses independent executable checks and bound review.
Its `--no-cache` arm tests combined cache/investigation reuse, not a memory-only
ablation; offline fixtures do not establish real coding productivity.

For Lore 0.5's executable baseline / fast / intelligent coding-task comparison,
see [Intelligence evaluation](INTELLIGENCE.md). It runs a supplied real coding
agent, applies allowed implementation changes to isolated copies, executes
correctness and constraint checks, and retains bound human-review packets.
Fallbacks and synthetic fixtures do not establish intelligent benefit; unknown
inference charges remain unknown.

For Lore 0.4's native-import scenarios and four-setup engineering-task comparison, see [Cross-source evaluation](CROSS_SOURCE.md). Its mechanical tests and preparation command run without inference. The semantic release gates require actual model runs and blind human review; bundled fixtures cannot establish measured benefit.

The original document-synthesis benchmark defines four collections: an evolving,
human-curated payments project called Atlas; Lore's own documentation; and
pinned revisions of the public OpenWiki and LLM Wiki repositories. External
repositories are fetched only when explicitly selected and at the exact commit
recorded in [targets.json](targets.json). That benchmark copies permitted
Markdown documents into an isolated workspace and never executes source-repository
code or edits the original files. Coding-task protocols separately execute an
explicitly selected agent and independent checker in a deliberately isolated
study environment; this is not a Lore runtime capability.

The critical research question is whether real inference models turn scattered, changing project artifacts into coherent, accurate knowledge. The benchmark checks SQLite integrity, current source excerpt validity, source-specific assertion extraction, expected types/lifecycles, and known equivalence or supersession relationships. It also performs an unchanged update and verifies **zero model calls and byte-identical wiki pages**. The Atlas mutation adds an explicit replacement ADR and a reported deployment, allowing us to observe whether Lore updates the documented architectural decision without wrongly upgrading a report to independently verified production reality.

## Limits of automatic scoring

The original source-assertion checkpoint matcher performs **lexical substring
matching** against retained evidence excerpts, not semantic entailment. A correct
quotation or foreign key does not make a model's interpretation true. Two
assertions can share wording while having different scope or dates, and two
differently worded assertions can convey the same proposition. Each synthesis
run therefore produces a human-review worksheet. Reviewers should inspect
generated prose, cited evidence IDs, historical applicability, open
contradictions, and the information architecture, recording counterexamples.
The original OpenWiki/LLM Wiki synthesis collections have structural checks
without reviewed gold interpretations; they must not be used to advertise
semantic recall. The separate 24-case 0.7 source-gold review covers its pinned
documentary queries and conditions only, and does not establish these wider
synthesis or human outcomes.

Lore 0.8 records non-sensitive per-attempt provider usage, including retries and unsuccessful calls, and persists successful compiler-run ledgers in SQLite schema 8. The coding and adaptive evaluators capture explicit invocation ledgers and independently recompute their totals. Historical usage and missing token counts remain unknown. Direct provider APIs do not supply an invoice, so billed USD remains null; the original single-project benchmark also permits an operator-supplied measured amount using `--billed-cost-usd`. Never infer actual USD from request counts or offline envelope tokens. See [provider usage and cost accounting](../docs/PROVIDER_USAGE.md) for the optional output contract and failure retention.

## What is needed

Build Lore from a current stable Rust toolchain with cargo build --release --locked. The benchmark uses Python 3.10+ with no third-party dependencies. The two external repositories additionally require Git. The selected model must be available in Ollama or through an API account with the appropriate permissions. The command invokes **the real Lore binary** and doctor --inference; it does not pretend to run a language model. Failed preflight or failed inference ends the benchmark without fabricated quality results.

Each run creates a new output directory with project/docs, project/wiki, a project/.lore state database and model cache, metrics.json, and REVIEW.md. Treat the entire directory as potentially sensitive. The benchmark does not upload those files anywhere. For hosted inference, supplying --provider openai is insufficient: --allow-hosted is also required, explicitly authorizing source passages to leave your machine. Credentials remain environment variables. Local-only mode uses the literal loopback Ollama endpoint by default.

## Run a real local-model benchmark

Use a unique output path each time so that old model caches do not hide model errors or distort inference costs. Begin with Atlas before testing the larger real repositories.

~~~bash
cargo build --release --locked
ollama pull gemma4:12b
ollama pull clef-flash

python3 evaluation/benchmark.py run \
  --target atlas --provider ollama --model gemma4:12b \
  --decision-provider ollama --decision-model clef-flash \
  --lore-binary target/release/lore --mutate \
  --output evaluation-results/atlas-ollama-01
~~~

For hosted OpenAI, select a Responses-compatible generative model that **your particular API account supports**. Set OPENAI_API_KEY in the environment. This example sends the public bundled Atlas corpus to OpenAI. Start without a decision model to isolate extraction and synthesis quality, then run a separately labeled OpenAI Decisions configuration once access is verified.

~~~bash
export OPENAI_API_KEY='your-api-key'

python3 evaluation/benchmark.py run \
  --target atlas --provider openai --model YOUR_RESPONSES_MODEL_ID \
  --allow-hosted --mutate --lore-binary target/release/lore \
  --output evaluation-results/atlas-openai-01
~~~

The same command can target lore-self, openwiki, and llm-wiki with their respective --target values. OpenWiki includes sixteen pinned Markdown files; LLM Wiki includes three files; Lore uses its README and two implementation-related documents. As with Atlas, all inputs are copied to an isolated workspace. Running OpenWiki is a larger and potentially expensive inference workload; set model limits and inspect available API budgets before executing it. You can include an optional decision role with --decision-provider and --decision-model and should report the model combination explicitly.

## Compare reasoning-effort settings

The benchmark and three-project suite now record a per-task `reasoning` policy. By default, OpenAI Responses runs use `low` for extraction, `high` for reconciliation/verification, and `medium` for page and overview synthesis. Lore 0.5 adds `context_synthesis: medium` and `context_verification: high`. Use flags such as `--reasoning-reconciliation medium`, `--reasoning-verification xhigh`, `--reasoning-context-synthesis high` or `--disable-reasoning` to test other settings. Do not interpret different reasoning policies as identical-run repeatability: the comparator explicitly flags different or missing policies, and suite acceptance requires the same configured policy for each target. Keep the model, corpus, binary and verification settings fixed while varying one effort. See [Reasoning configuration](../docs/REASONING.md) for task settings and supported values.

## Inspect without using any models

Preparing a corpus is separate from inference and therefore does not require Ollama or API credentials. Scoring a completed run also does not call a model.

~~~bash
python3 evaluation/benchmark.py prepare \
  --target atlas --output /tmp/lore-atlas-eval

python3 evaluation/benchmark.py score \
  --project evaluation-results/atlas-ollama-01/project \
  --gold evaluation/gold/atlas.json
~~~

The Atlas corpus contains nine initial files, two added in the evolution phase, eleven manually selected source checkpoints, and three relation checks. These identify plausible expected behavior, but reviewers should revise gold labels that prove ambiguous and document those revisions. See [BASELINE.md](BASELINE.md) for results that have actually been measured.

## Document-synthesis quality gate

Before describing Lore as validated, require no fabricated source citations in the reviewed pages, no confirmed promotion of unapproved plans into deployed facts, explicit historical preservation of superseded decisions, and correct no-op and interrupted-publication behavior. Investigate Atlas lexical and typed checkpoint coverage below roughly 90 percent, while recognizing these figures are provisional targets rather than observed results. Have at least two independent people review each real generated wiki, including whether the page hierarchy provides genuine value beyond the source files. Compare the same source revision under different models, prompts, and optional decision-model strategies, recording wall-clock time, model calls, measured billed charges, omissions, and wrong conclusions.

Only after this baseline exists should we choose whether to invest next in more reliable extraction, global topic planning, human review resolution, provider routing, or scalable candidate retrieval. Fast decisions must not silently exclude potentially material new knowledge just to reduce cost.

## Compare independent runs and review historical knowledge

Use the new `compare_runs.py` command to compare separate evaluation directories without any model calls. It hashes the original Markdown paths and bytes, checks provider/model settings and Lore binary fingerprints, and reports overlap in topic names and normalized knowledge statements. These are **lexical stability measurements, not factual accuracy**. Comparisons with different source corpora, model configurations, or unknown build identities are flagged rather than presented as proof of repeatability.

```bash
python3 evaluation/compare_runs.py \
  evaluation-results/atlas-run-a evaluation-results/atlas-run-b \
  --output evaluation-results/repeatability.json
```

The benchmark now records the executed Lore binary SHA-256 and source-file SHA-256 manifest for newly created runs. Reuse the same pinned model version and repeat the *same initial source corpus* several times to separate model-output variation from intentional incremental changes. Earlier results that did not record build hashes can still be compared descriptively, but are not considered strictly comparable. The stricter source-checkpoint gold matching remains visible; separately labelled acceptable kinds/lifecycles avoid incorrectly treating a documented outcome as categorically false because of one narrowly specified metadata label.

Read [ATLAS_FINDINGS.md](ATLAS_FINDINGS.md) for the source-backed defects found in the first real Atlas run, the reason a reaffirmation is distinct from equivalence, and the criteria for a validated follow-up. Never commit the raw `.lore` evidence database or exported evaluation results from a confidential project.

## v0.2 cross-project suite

Run `python3 evaluation/suite.py prepare --output evaluation-results/prepared-v02` to prepare Atlas, Lore-self and pinned OpenWiki without inference. Run the suite with real models to produce fresh, separate projects and reports. Every Atlas run includes its mutation. `--repeats 3` runs three independent copies per target and writes lexical repeatability reports; it does not make the suite a human assessment. Keep previous result directories and use a new output directory for each suite invocation.

```bash
python3 evaluation/suite.py run \
  --provider openai --model gpt-6-luna \
  --generative-base-url "$FOUNDRY_OPENAI_BASE_URL" \
  --decision-provider ollama --decision-model clef-flash \
  --decision-base-url http://127.0.0.1:11434 \
  --allow-hosted --lore-binary target/release/lore \
  --repeats 1 --output evaluation-results/v02-suite-01
```

Set `FOUNDRY_OPENAI_BASE_URL` to your actual already-tested compatible endpoint and provide the configured API-key environment variable. The same role-specific flags are available on `benchmark.py run`. They resolve the previous ambiguity when a hosted generative endpoint and a local decision endpoint differ. A single provider still has a single endpoint in Lore configuration; incompatible per-role endpoints for that same provider are rejected. Hosted opt-in is required even for hosted decisions alone. Normal provider charging applies; the larger OpenWiki corpus and exhaustive reconciliation can be expensive, so start with `--targets atlas` before running the whole suite.

A successful suite run creates an unscored `HUMAN_REVIEW.json` for each project. Read the wiki and sources, then fill in reviewer/date, all seven 0–3 scores, notes, actual evidence IDs and any critical errors before setting `complete` to true. The scores are your assessment, not values generated by Lore. For an existing completed run, `python3 evaluation/suite.py init-review PATH_TO_RUN` creates the template without inference. The template is bound to exact report, wiki and source hashes; edits invalidate prior approval.

```bash
python3 evaluation/suite.py assess \
  evaluation-results/v02-suite-01/atlas-01 \
  evaluation-results/v02-suite-01/lore-self-01 \
  evaluation-results/v02-suite-01/openwiki-01 \
  --output evaluation-results/v02-suite-01/acceptance.json
```

Assessment exits with code 2 when the quality gate is incomplete or fails. It requires the three target projects, matching binary/model roles, passing automated evidence/no-op checks, Atlas relationships and synthesis verification, plus human scores at least 2 and no critical errors. A fixture can test these rules, but only model-enabled runs and real human readings can satisfy them in practice. See [ACCEPTANCE.md](ACCEPTANCE.md). This implementation does not claim a new live-model quality score. The Atlas rubric is versioned and its previous definition archived; scores across changed rubrics must not be presented as model improvements.

## Source-bound verifier context and rejection diagnostics

Lore's topic verifier can inspect a bounded set of active evidence from **the same source document** even when the assertions appear on other topic pages. Those context rows are verifier-only, not extra citation privileges for the topic writer. The active source document's heading path may include a date; this dates the documentation, not automatically a decision's effective time or a reported deployment. If the verifier's context budget omits some sibling rows, it is explicitly marked incomplete and must not support claims that nothing else exists.

Evaluation phase reports retain bounded `quality_diagnostics` entries (topic, attempt, validator, issue description) for rejected drafts, even when a repair succeeds. These are diagnostic data, not proof of factual reliability; review the underlying source documents and published pages. Atlas relationship scoring now requires a *unique source-backed knowledge-unit assignment* instead of an exact kind/lifecycle match, with the scorer version recorded in metrics. Compare versions separately and preserve the strict type/lifecycle classification score.


## Shared-passage relationship scoring

Relationship scoring uses `source-witness-sets-v3`. Distinct assertions can legitimately share one original passage. The scorer examines the full candidate sets rather than selecting whichever unit agrees with an expected label. It records the active source-bound edge witnesses for positive relations and tests all candidate pairs for prohibited relations. Missing assignments, an assertion assigned to several units, invalid excerpts, and mismatched witnesses remain unassessable. Exact type/lifecycle labels remain separate diagnostics. The comparator flags changed scorer versions; a corrected score on an existing database must not be advertised as a new model-quality result.
