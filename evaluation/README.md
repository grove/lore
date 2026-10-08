# Evaluating Lore on real projects

This directory is a **reproducible evaluation harness**, not a claim that model quality has already been established. It defines four document collections: an evolving, human-curated payments project called Atlas; Lore's own documentation; and pinned revisions of the public OpenWiki and LLM Wiki repositories. External repositories are fetched only when explicitly selected and at the exact commit recorded in [targets.json](targets.json). The script copies permitted Markdown documents into an isolated workspace and never executes source-repository code or edits the original files.

The critical research question is whether real inference models turn scattered, changing project artifacts into coherent, accurate knowledge. The benchmark checks SQLite integrity, current source excerpt validity, source-specific assertion extraction, expected types/lifecycles, and known equivalence or supersession relationships. It also performs an unchanged update and verifies **zero model calls and byte-identical wiki pages**. The Atlas mutation adds an explicit replacement ADR and a reported deployment, allowing us to observe whether Lore updates the documented architectural decision without wrongly upgrading a report to independently verified production reality.

## Limits of automatic scoring

The source-assertion checkpoint matcher performs **lexical substring matching** against retained evidence excerpts, not semantic entailment. A correct quotation or foreign key does not make the model's interpretation true. Two assertions can share wording while having different scope or dates, and two differently worded assertions can convey the same proposition. For this reason, every run produces a human-review worksheet. Reviewers should inspect generated prose, cited evidence IDs, historical applicability, open contradictions, and the usefulness of the information architecture, recording concrete counterexamples. The external public projects currently have structural checks but do not have reviewed gold interpretations; they must not be used to advertise semantic recall.

The current Lore database logs model calls, cached calls, provider/model identifiers, task types and durations but **does not persist provider input/output tokens or actual currency charges**. The report therefore leaves billing cost as null unless you explicitly supply a measured billed amount using the --billed-cost-usd argument. Do not infer actual USD expense from the number of requests. Provider usage instrumentation should be added before drawing conclusions about relative pricing.

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

## A proposed gate for the next release

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
