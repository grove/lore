# Lesson 1 — What Lore does (and doesn't)

**Goal:** Explain why Lore is more than a generated wiki, and decide when to invoke it.

A project has scattered knowledge: current decisions, outdated plans, incidents, work reports and implementation. Lore reads project Markdown and optional imported snapshots (including supported OpenWiki, Engram and Beads formats), preserves quotations and their histories, and connects the evidence into an explanation or a recommendation for a task. It also publishes ordinary Markdown wiki pages when initialized.

Consider the [HarborPay practice project](../../examples/tutorial-project/README.md):

- [ADR-001](../../examples/tutorial-project/docs/ADR-001.md) is the **accepted** production charge rule: at most three attempts with a stable idempotency key.
- [RFC-002](../../examples/tutorial-project/docs/RFC-002.md) is a **proposal** for five attempts, not a policy change.
- [INC-014](../../examples/tutorial-project/docs/INC-014.md) is a **reported** incident, not independent proof about today's deployed software.
- [src/policy.py](../../examples/tutorial-project/src/policy.py) is current example code; even inspecting it does not prove production behavior.

Lore should help you choose a responsible next step, not merely list those four files. An agent can still make a code change only with its separate permission, and an actual checker must run before declaring tests passed.

## Four things Lore distinguishes

| Kind | What it establishes | What it does not establish |
| --- | --- | --- |
| Accepted decision | Documented intent and scope | Current production behavior |
| Proposal | A suggested future alternative | Approval |
| Incident/work report | What a source reports happened | Independent verification |
| Code observation | Bounded static checkout evidence | A live execution or successful test |

**Exercise:** Without running any model, read ADR-001 and RFC-002. Should an agent raise the production limit to five merely because the RFC is newer? What about retrying an ambiguous refund outcome? Check [answers](answers.md) after answering.

**Next:** [First contact](02-first-contact.md).
