# Configurable reasoning effort

Lore supports **per-task reasoning effort for the OpenAI Responses API**, including compatible enterprise/Foundry Responses endpoints. The setting controls the generative model's requested reasoning budget. It does **not** change OpenAI's Decisions API, Clef-Flash/System One, Jev, or Ollama Chat; those endpoints have separate capability contracts. Local inference never receives a `reasoning` field from these settings.

## Defaults

A new or existing `lore.yml` uses these defaults without requiring an edit:

| Lore work | Config key | Responses effort |
| --- | --- | --- |
| Extraction of source assertions | `extraction` | `low` |
| Knowledge-unit reconciliation | `reconciliation` | `high` |
| Topic-page synthesis | `synthesis` | `medium` |
| Project overview synthesis | `overview` | `medium` |
| Topic semantic verification | `verification` | `high` |
| Overview semantic verification | `overview_verification` | `high` |
| Unknown/new generative task | `default` | `medium` |

These are **benchmark starting points**, not measured optimal settings. In particular, high-effort verification may be slower and consume more reasoning tokens without necessarily improving results. Compare the same corpus and model across effort settings before making general performance claims.

## Example `lore.yml`

```yaml
schema_version: 1
project:
  name: my-project
sources:
  roots:
    - id: docs
      path: ./docs
privacy:
  local_only: false
models:
  generative:
    provider: openai
    model: gpt-6-luna
  # Optional: remove the decision role for generative-only operation.
  decision:
    provider: openai
    model: gpt-6-luna
  reasoning:
    enabled: true
    default: medium
    extraction: low
    reconciliation: high
    synthesis: medium
    overview: medium
    verification: high
    overview_verification: high
providers:
  openai:
    base_url: https://api.openai.com/v1
    api_key_env: OPENAI_API_KEY
```

You can override any one task, for example `models.reasoning.synthesis: high`. Supported configured values are `none`, `low`, `medium`, `high`, `xhigh`, and `max`. GPT-6 Luna supports all six, with `medium` as its native default. **Other models may not support all values**; if a provider rejects an effort, Lore surfaces the provider error. Never silently retry with a different effort or model because that would undermine reproducibility. To preserve a model's default behavior or use an endpoint without the parameter, set `enabled: false`, which omits `reasoning` entirely.

The OpenAI Responses payload includes a task-specific block such as:

```json
{"model": "gpt-6-luna", "reasoning": {"effort": "high"}, "input": "..."}
```

The OpenAI Decisions request has no equivalent setting. The same name `gpt-6-luna` can be configured for distinct roles; a decision-model call is *not* a Responses generation request.

## Repeatable evaluations

Both `evaluation/benchmark.py run` and `evaluation/suite.py run` accept task overrides:

```bash
python3 evaluation/benchmark.py run \\
  --target atlas --provider openai --model gpt-6-luna \\
  --allow-hosted --mutate --lore-binary target/release/lore \\
  --reasoning-extraction low \\
  --reasoning-reconciliation high \\
  --reasoning-synthesis medium \\
  --reasoning-overview medium \\
  --reasoning-verification high \\
  --reasoning-overview-verification high \\
  --output evaluation-results/atlas-luna-reasoning-01
```

Flags also support `--reasoning-default` and `--disable-reasoning`. Each report stores the resolved `reasoning` policy and configuration digest. The run comparator refuses to mark runs strictly comparable if reasoning effort settings differ or were not recorded. The suite acceptance check also requires an explicitly recorded policy shared across its targets.

Changing an effort in `lore.yml` changes Lore's configuration fingerprint, invalidating relevant cached inferences and the successful baseline. An update after changing reasoning effort may call the model again, while a later true no-op is still zero-call. Exact provider model revisions, costs and token usage require independent observation; identical configured model aliases and effort values do not establish identical underlying model deployments.

References: [GPT-6 Luna model](https://developers.openai.com/api/docs/models/gpt-6-luna), [reasoning effort](https://developers.openai.com/api/docs/guides/reasoning), [Responses API](https://developers.openai.com/api/reference/resources/responses/methods/create).
