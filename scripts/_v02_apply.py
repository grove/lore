from pathlib import Path
import json

def edit(name,old,new):
 p=Path(name);s=p.read_text();assert s.count(old)==1,(name,old[:80],s.count(old));p.write_text(s.replace(old,new))
edit('Cargo.toml','version = "0.1.0"','version = "0.2.0"')
edit('Cargo.lock','name = "lore"\nversion = "0.1.0"','name = "lore"\nversion = "0.2.0"')
p=Path('migrations/0004_review_history.sql');s=p.read_text();a='PRAGMA user_version=4;';assert s.count(a)==1;s=s.replace(a,'''CREATE TRIGGER review_state_requires_event BEFORE UPDATE OF status ON review_items
WHEN NEW.status <> OLD.status AND NOT EXISTS(
 SELECT 1 FROM review_events e WHERE e.review_id=OLD.id
 AND e.sequence=(SELECT max(sequence) FROM review_events WHERE review_id=OLD.id)
 AND e.from_status=OLD.status AND e.to_status=NEW.status
)
BEGIN SELECT RAISE(ABORT,'review disposition requires a history event'); END;
'''+a);p.write_text(s)
p=Path('README.md');s=p.read_text();lines=s.splitlines();
for i,line in enumerate(lines):
 if line.startswith('Lore now has a working initial implementation:'):
  lines[i]='Lore 0.2 adds clearer knowledge classification, an evidence-bound review workflow, and a cited project overview to the working incremental compiler. Documented architecture is distinguished from future plans and source-reported delivery; review questions can be resolved without erasing their history. The offline suite exercises the compiler, HTTP contracts, actual CLI and failure recovery, while the cross-project evaluation suite makes real-model and human validation reproducible. This is early software: implemented safeguards and passing fixture tests are not a claim of universal model accuracy. See [the v0.2 guide](docs/V02.md) for the changes and upgrade behavior.'
 if line.startswith('The CLI also provides `lore evidence'):
  lines[i]='The CLI also provides `lore evidence <evidence-id>` for an exact archived passage and `lore review` for unresolved questions. `lore review show <review-id>` displays the history and evidence binding; `resolve`, `dismiss` and `reopen` accept an explicit `--reason` and optional `--actor`. These actions record a disposition, not a new project fact, and make no model calls. Specific relationship questions can also close automatically when active evidence from the same source revision establishes the named replacement; withdrawn evidence reopens them. `--config path/to/lore.yml` selects another configuration, and `--json` produces machine-readable results. Operational errors exit with code 1, argument errors with code 2, and audit findings with code 3.'
s='\n'.join(lines)+'\n';anchor='## Local, hosted, or a combination';assert s.count(anchor)==1
s=s.replace(anchor,'''## An overview you can follow back to evidence

The generated index now explains the project's major systems, documented design, decisions, future initiatives and open questions in cited paragraphs. It uses a bounded, representative selection from every topic; the detailed pages preserve the full extracted knowledge. An extra generative check reviews the overview for unsupported claims when synthesis verification is enabled. Decision links name documents rather than repeating topic titles, so two decisions on the same page appear as “ADR-027 explicitly supersedes ADR-001.” The review-history page distinguishes pending questions from retained resolutions and makes the reason for each transition inspectable.

Documentary status and factual verification stay separate. An architecture specification can describe the selected design without proving what runs in production, and that lack of verification does not make it a future plan. Proposals remain future intent, reported deployments remain reports, and mandatory release rules are not confused with implementation completion. These distinctions are represented in the extraction schema and generation prompts; their accuracy should still be checked on real project documents.

'''+anchor);p.write_text(s)
p=Path('docs/IMPLEMENTATION.md');s=p.read_text();lines=s.splitlines()
for i,line in enumerate(lines):
 if line.startswith('Review records are append-preserving diagnostics.'):
  lines[i]='Review records now have immutable source/target bindings and append-only disposition events. `lore review` lists pending questions; `review list --all`, `review show`, `review resolve`, `review dismiss` and `review reopen` expose the retained history and human actions. Automatic resolution requires an active explicit relation from the same source revision to the same predecessor for an accepted decision question. Generic ambiguity, conflicts, proposals and unrelated questions are not automatically approved. Withdrawal of resolution evidence reopens the question. Human dispositions leave knowledge and source assertions unchanged, and their actor field is user-supplied attribution rather than authentication.'
 if line.startswith('The current supersession guard is intentionally narrow:'):
  lines[i]='The current supersession guard is intentionally narrow: the source must document an accepted decision and an exact passage must explicitly replace an identifiable predecessor. A unique named document reference takes precedence over differing model paraphrases of scope; the full-statement fallback still requires matching scope. Conditional, speculative, negated or ambiguous references remain reviewable. An issue closure is insufficient, and these guards do not independently verify deployment.'
 if line.startswith('Current limitations include exhaustive candidate cost,'):
  lines[i]='Current limitations include exhaustive candidate cost, sequential inference, conservative identity across ambiguous renames, bounded context rather than autonomous research, no independent code/runtime verification, and no graph-editing UI. Topic pages retain stable identities. The project overview now synthesizes representative evidence from every topic, subject to explicit context limits, while review-only edits update the queue without regenerating that narrative. Citation and semantic verification remain fallible; consult docs/V02.md for implementation boundaries and evaluation/ACCEPTANCE.md for the uncompleted live/human validation gates.'
s='\n'.join(lines)+'\n';s+='''
## v0.2 operation

The extraction schema includes `design` for selected documented architecture, separate from proposals/plans, observed behavior and reported outcomes. Mandatory invariants can be constraints while operating checklists can be procedures. The synthesis receives an explicit documentary-basis field and preserves those distinctions. Version 4 of the SQLite schema adds review contexts and append-only events; ordinary status updates must pass through a matching event. Existing records are bound only by exact reconstruction of their original hashed keys from retained reconciliation logs. Unresolvable legacy questions are not automatically closed.

The overview is now a generated and optionally verified narrative with known-ID citations, not just an index. Selection is breadth-first across topics, up to eight representative records per topic, and refuses a budget that cannot include any record from every topic. Exact relation snapshots support decision-document labels; connected replacement/reaffirmation chains are supplied to affected pages. Overview synthesis adds inference work on changed knowledge, but no-op updates and manual review-only actions do not call models. Manual review publication is staged and recoverable; it requires clean sources/configuration/output and refuses to overwrite human page edits.

Use `evaluation/suite.py` for separate Atlas, Lore-self and pinned OpenWiki runs. Its explicit endpoint flags support hosted generative inference and local decisions without an ambiguous shared base URL. `HUMAN_REVIEW.json` starts unscored and is bound to metrics, wiki/source hashes and binary identity. The assessment gate requires completed real results and seven evidence-supported human scores; it does not infer human approval from CI or lexical gold-label matches. The old Atlas rubric remains archived, and rubric hashes prevent relabeling earlier scores as new model performance.
''';p.write_text(s)
p=Path('DESIGN.md');s=p.read_text();assert 'Draft v0.6' in s;s=s.replace('Draft v0.6','Draft v0.7');anchor='## 1. Overview';assert s.count(anchor)==1;s=s.replace(anchor,'''**v0.2 implementation update.** The classification model now includes documented `design` independently of future intent and runtime verification. Review contexts bind exact source-assertion revisions and predecessor units; immutable events project pending/resolved/dismissed queue state. Specific active documentary evidence can resolve matching accepted-decision questions, withdrawal reopens them, and explicit human dispositions preserve the graph and publication invariants. Generated decision navigation uses source-document identities and exact relation evidence. The index is a bounded, cited and verified project synthesis. A three-project evaluation suite binds human assessments to report/wiki/source fingerprints and refuses to equate fixture tests or changed rubrics with empirical model quality. [The v0.2 guide](docs/V02.md) and [acceptance rules](evaluation/ACCEPTANCE.md) define the shipped behavior and remaining real-model validation.

'''+anchor);p.write_text(s)
p=Path('evaluation/README.md');s=p.read_text();s+='''
## v0.2 cross-project suite

Run `python3 evaluation/suite.py prepare --output evaluation-results/prepared-v02` to prepare Atlas, Lore-self and pinned OpenWiki without inference. Run the suite with real models to produce fresh, separate projects and reports. Every Atlas run includes its mutation. `--repeats 3` runs three independent copies per target and writes lexical repeatability reports; it does not make the suite a human assessment. Keep previous result directories and use a new output directory for each suite invocation.

```bash
python3 evaluation/suite.py run \\
  --provider openai --model gpt-6-luna \\
  --generative-base-url "$FOUNDRY_OPENAI_BASE_URL" \\
  --decision-provider ollama --decision-model clef-flash \\
  --decision-base-url http://127.0.0.1:11434 \\
  --allow-hosted --lore-binary target/release/lore \\
  --repeats 1 --output evaluation-results/v02-suite-01
```

Set `FOUNDRY_OPENAI_BASE_URL` to your actual already-tested compatible endpoint and provide the configured API-key environment variable. The same role-specific flags are available on `benchmark.py run`. They resolve the previous ambiguity when a hosted generative endpoint and a local decision endpoint differ. A single provider still has a single endpoint in Lore configuration; incompatible per-role endpoints for that same provider are rejected. Hosted opt-in is required even for hosted decisions alone. Normal provider charging applies; the larger OpenWiki corpus and exhaustive reconciliation can be expensive, so start with `--targets atlas` before running the whole suite.

A successful suite run creates an unscored `HUMAN_REVIEW.json` for each project. Read the wiki and sources, then fill in reviewer/date, all seven 0–3 scores, notes, actual evidence IDs and any critical errors before setting `complete` to true. The scores are your assessment, not values generated by Lore. For an existing completed run, `python3 evaluation/suite.py init-review PATH_TO_RUN` creates the template without inference. The template is bound to exact report, wiki and source hashes; edits invalidate prior approval.

```bash
python3 evaluation/suite.py assess \\
  evaluation-results/v02-suite-01/atlas-01 \\
  evaluation-results/v02-suite-01/lore-self-01 \\
  evaluation-results/v02-suite-01/openwiki-01 \\
  --output evaluation-results/v02-suite-01/acceptance.json
```

Assessment exits with code 2 when the quality gate is incomplete or fails. It requires the three target projects, matching binary/model roles, passing automated evidence/no-op checks, Atlas relationships and synthesis verification, plus human scores at least 2 and no critical errors. A fixture can test these rules, but only model-enabled runs and real human readings can satisfy them in practice. See [ACCEPTANCE.md](ACCEPTANCE.md). This implementation does not claim a new live-model quality score. The Atlas rubric is versioned and its previous definition archived; scores across changed rubrics must not be presented as model improvements.
''';p.write_text(s)
p=Path('evaluation/benchmark.py');s=p.read_text();a='"configuration_sha256": hashlib.sha256(json.dumps(checked_config, sort_keys=True).encode()).hexdigest(),';assert s.count(a)==1;s=s.replace(a,a+'\n              "synthesis_verification": checked_config["processing"]["verify_synthesis"],');p.write_text(s)
p=Path('evaluation/suite.py');s=p.read_text();s=s.replace('failures=[];noop=report.get("no_op",{})','failures=[];noop=report.get("no_op",{})\n    if report.get("synthesis_verification") is not True:failures.append("Synthesis verification was disabled or not recorded")\n    if not report.get("lore_binary_sha256"):failures.append("Binary identity missing")');s=s.replace('"human_failures":[],"ready":False}', '"human_failures":[],"ready":False,"binary":report.get("lore_binary_sha256"),\n        "model_roles":[report.get(k) for k in ("provider","model","decision_provider","decision_model")]}')
s=s.replace('"candidate_validated":set(TARGETS)<=targets and all(x["ready"] for x in entries),','"candidate_validated":set(TARGETS)<=targets and all(x["ready"] for x in entries) and len({x["binary"] for x in entries})==1 and len({tuple(x["model_roles"]) for x in entries})==1,')
p.write_text(s)
p=Path('evaluation/tests/test_suite.py');s=p.read_text();s=s.replace("'schema_version':1,'target':target,'lore_binary_sha256':'fixture-only'", "'schema_version':1,'target':target,'lore_binary_sha256':'fixture-only','synthesis_verification':True,'provider':'fixture','model':'not-real'");p.write_text(s)
p=Path('evaluation/targets.json');data=json.loads(p.read_text());data['lore-self']['include'].append('docs/V02.md');p.write_text(json.dumps(data,indent=2)+'\n')
p=Path('.github/workflows/ci.yml');s=p.read_text();a='          python evaluation/benchmark.py prepare --target openwiki --output /tmp/lore-eval-openwiki';assert s.count(a)==1;s=s.replace(a,'          python evaluation/suite.py prepare --output /tmp/lore-v02-corpora');p.write_text(s)
