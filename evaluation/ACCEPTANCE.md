# v0.2 acceptance rules — fixed before the next live runs

The implementation is ready for testing when deterministic contracts pass; the **quality candidate is validated only by real inference and explicit human assessment**. Neither CI fixtures nor an edited gold rubric are substitutes for that assessment. The suite defaults to Atlas, Lore-self and pinned OpenWiki, with independent output/state directories for every run. Use `--repeats 3` to inspect variability; comparison reports describe lexical stability, not correctness.

## Automated gates

Every run must complete an initial compilation with synthesis verification enabled, valid SQLite integrity, nonempty checked source evidence and no excerpt failures. The unchanged update must report no-op, zero generative/decision calls and identical wiki page hashes. Atlas must also complete its mutation and pass both initial relationship rules and all three post-mutation relationship rules. The binary identity must be recorded, and the three candidate runs must use the same binary and configured model roles. Exact type/lifecycle scores remain diagnostics because multiple classifications can be defensible; they must not conceal the failure of a semantic criterion.

## Human gates

The seven criteria are knowledge coverage, documented design versus future intent, decision chronology, uncertainty handling, citation entailment, organization, and usefulness. A reviewer reads the generated pages and sources, supplies their own attribution and date, and records 0–3 scores with explanations and actual evidence IDs. Every score must be at least 2, with no critical errors. Marking a review complete is an attestation by its author, not identity verification by Lore. An incomplete or mismatched worksheet cannot pass. Modifying the wiki, source corpus or metrics invalidates the binding, requiring a fresh assessment rather than reusing obsolete approval.

Particular cases to inspect are architecture falsely labeled as planned work, issue reports overstated as verified deployments, historical reaffirmations confused with contradictions, an original decision still presented as current after explicit replacement, an unsupported effective date, a source citation that exists but does not support the sentence, and an already-settled relationship question still misleadingly marked pending. Check the project overview as well as the detailed topic pages.

## What is and is not measured

The initial user-supplied Atlas run at `9479727` passed relationship 3/3, provenance and no-op checks; classification was 5/11 strict and 9/11 under the then-current acceptable-label rubric. It used 77 generative calls and 18 decision calls. That is a pre-v0.2 baseline, not a score for the new implementation. The implementation session has no configured live Foundry/Ollama execution environment and does not supply any new live scores or human scores. Run the suite in the model-enabled environment, inspect the outputs, and record the measurements before labeling this quality milestone validated.
