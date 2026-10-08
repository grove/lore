# Real-world validation baseline — 2026-10-08

This report deliberately separates implemented tooling from measured model quality. It must not be interpreted as a passed semantic benchmark.

| Area | Current evidence | Status |
| --- | --- | --- |
| Evaluation harness | Six offline standard-library unit tests | Run in GitHub CI; see Actions for the measured result |
| Atlas sources | Nine initial Markdown files plus two explicit later changes | Prepared |
| Atlas gold labels | Eleven source checkpoints and three relationship checks | Source text verified, model output **not scored** |
| Lore-self | Current README and implementation documentation | Target defined; real inference not run |
| OpenWiki | Public commit pinned in targets.json | Target defined; real inference not run |
| LLM Wiki | Public commit pinned in targets.json | Target defined; real inference not run |
| Ollama | No Ollama installation available in the evaluation environment | Live model quality unmeasured |
| OpenAI | No hosted API credential available in the evaluation environment | Live model quality unmeasured |
| True factual precision and wiki usefulness | Requires real inference plus human reviewers | **Unknown** |
| Provider token usage and actual cost | Existing Lore runtime does not persist token usage | **Not measured** |

The offline work only verifies corpus preparation, privacy boundaries, and conservative evaluation bookkeeping. First live runs should start with Atlas, then Lore-self, followed by the two pinned public projects. Store the resulting metrics and reviewed pages alongside model/prompt configuration; do not substitute deterministic fixture-test scores for empirical model quality.
