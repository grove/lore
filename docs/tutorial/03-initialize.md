# Lesson 3 — Compile durable project intelligence

**Goal:** Understand initialization, configuration, model costs and `update`.

**Optional model-backed lesson.** Complete lesson 2 first. If you have no configured inference provider, read this lesson and skip execution; lessons 5's *integration preview* and the offline example tests still work.

Work in your copied classroom directory, not in the original repository:

```sh
lore init --configure-only --source ./docs
```

This writes editable `lore.yml` deliberately and **does not call a model**. Inspect the file before continuing. Sources should point at `./docs`; this is a small, intentional corpus. Keep `.lore/` private and ignored by Git because its registry and caches can contain source material.

## Choose your provider explicitly

The default configuration uses local-only Ollama and a generative model such as `gemma4:12b`. For that option, install/start Ollama independently and obtain the configured model:

```sh
ollama pull gemma4:12b
lore doctor --inference
lore init
```

`doctor --inference` sends small synthetic model probes; `init` can perform **many model calls** to extract and reconcile project evidence. Running a local model consumes compute; hosted inference can incur charges and requires explicit provider credentials, project privacy changes and host grants. Do not run inference without the relevant permissions and resource budget.

If you're not using Ollama, adapt `lore.yml` using the [local](../../examples/lore.local.yml), [intelligent](../../examples/lore.intelligent.yml) or [hosted](../../examples/lore.openai.yml) sample and the [provider documentation](../IMPLEMENTATION.md). Avoid pasting real keys into documentation.

## Inspect and maintain

After successful initialization:

```sh
lore status
lore --json context "Change production charge attempt policy"
lore search "charge"
lore read index
```

Lore now has a durable SQLite registry in `.lore/` and generated Markdown in `lore/`. These are *derived* material, not a replacement for original source files.

After deliberately editing a classroom Markdown source, run:

```sh
lore status
lore update
lore status
```

`status` reports changes without inference. `update` can invoke configured models and should be explicitly authorized. Do **not** change ADR-001's accepted wording for this lesson; add a harmless note to a separate classroom document if you want to experiment.

**Exercise:** Why does `lore context` not silently run `lore update` when a document changes? Think about source consistency, provider expense and user permissions.

**Next:** [Find answers and evidence](04-explore-and-decide.md).
