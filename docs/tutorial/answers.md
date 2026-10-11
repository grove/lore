# Answer key and self-check

These are **conceptual invariants**, not promised exact text or stable output JSON IDs.

1. **Accepted vs proposed:** ADR-001 caps production charge submissions at three attempts with the same key; RFC-002 only proposes five. Newer publication alone does not override accepted scope.
2. **Legacy refunds:** An ambiguous outcome calls for read-only reconciliation, not another write. INC-014 is an incident *report*, not current deployed verification.
3. **Source-only bootstrap:** Without initialization, `lore onboard` and unpinned `lore context` can return cited local documentary navigation, schema 1 `bootstrap_source_only`, zero model calls and no source writes. Explicit `--fast` and pinned schemas require a compiled registry.
4. **Why initialization is explicit:** It can take substantial model calls and persist a knowledge snapshot; context queries do not implicitly authorize that cost or silently refresh accepted knowledge.
5. **Which command:** Use `explore` for navigation, `decisions` for why, `cases` for reported experience and `context` for what to do next. Source links remain the authority, not the generated presentation.
6. **Code vs execution:** Reading `src/policy.py` proves neither tests nor production ran. The learner must separately execute `python3 -m unittest discover -s tests -v` in the copied project and examine its result.
7. **Agent integration:** Preview/status are read-only; `--apply` edits only an exact managed instruction block. Instructions do not supply extra model, code execution or network grants, nor do they guarantee real agents consult Lore.
8. **Review and feedback:** Review decisions are source-bound, provenance-retaining and can become stale; a task receipt is a reported observation and draft, not independent proof. Only a fresh authorized update compiles an approved documentation change.
9. **Guardian:** A clean source-integrity replay does not prove alerts are useful. A report with no warnings is not a runtime safety certification.

## Graduation task

Explain an authorized change to the classroom retry behavior without violating accepted decisions. Your explanation should identify the original rule, rejected or proposed alternatives, reported negative case, relevant code, required separately executed check and a revalidation step after documents change.

If you can do that without confusing **evidence**, **interpretation**, **reported outcomes** and **actual execution**, you know the core mental model for effective Lore use.
