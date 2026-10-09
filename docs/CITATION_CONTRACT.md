# Scoped synthesis citations

## Failure observed in the v0.2 live suite

The uploaded 2026-10-09 failure record describes two fresh Foundry/Ollama Atlas attempts at `af2b936`. The first failed initialization without a retained CLI diagnostic. The second published its initial wiki and completed a zero-call no-op, then failed mutation with `overview output failed validation after repair: overview cited unknown knowledge`. The retained database is the initial publication, not a completed mutation. The rejected overview was not retained, so the specific offending ID and the first attempt's cause are unknown. Lore-self, OpenWiki and human assessment were not reached; the record is not a validated-beta result.

Two independently reproducible contract defects were found in the implementation. The page output schema allowed any string in `knowledge_ids`, although the validator accepted only IDs from the knowledge rows supplied in that request. Separately, overview selection took up to eight representative records per topic while providing all explicit decision relationships; a historically superseded predecessor could therefore be shown in relationship context without its actual knowledge record in the permitted citation set. These defects are addressed without claiming which particular ID triggered the unavailable rejected response.

## Generation and validation use the same permitted IDs

Both overview and topic synthesis now use a request-specific JSON Schema enum for citation IDs. The enum contains exactly the supplied `knowledge[].id` values. Evidence snapshot IDs, assertion/revision IDs, source document names, IDs outside the selected rows and invented IDs are not accepted as knowledge citations. These restrictions are sent through both OpenAI Responses and Ollama Chat adapters. Runtime validation remains necessary for endpoints that do not enforce the schema.

Overview selection first reserves a representative record for every topic and the records for both endpoints of every included decision relationship. It then fills remaining budget with additional representative knowledge. An endpoint is not merely added to the validator's allowlist: its actual record and evidence must be supplied. Missing endpoints or an insufficient explicit context budget produce an actionable error rather than silent omission. Topic-page batches remain restricted to their own knowledge rows; cross-topic relationships are explanatory context, not permission to cite arbitrary outside IDs.

Unknown/duplicate citations still fail validation and trigger bounded repair. Lore does not delete an invalid citation, relabel it as a nearby ID, widen validation to the entire registry, disable semantic verification, or publish an uncited paragraph. Even correctly formed citations can support incorrect prose; semantic verification and human assessment remain separate requirements. Failed generation leaves the previous database and wiki publication intact.

## Diagnostics and upgrade behavior

Rejected citation attempts write bounded metadata to `.lore/citation-diagnostics/` (or the configured state directory): task/run, permitted internal knowledge IDs, section/paragraph/citation positions, rejection category, ID length and hash, and a hash of the rejected draft. Raw rejected IDs, generated prose, source excerpts, credentials and provider response bodies are not stored. A record describes a rejected attempt; a later repair can succeed. These diagnostics are local project state and must not be routinely committed to Git.

A separate presentation-contract marker invalidates old rendered output without changing the extraction/reconciliation fingerprint. On an otherwise unchanged pre-fix project, the next `lore update` regenerates presentation using existing assertions and provenance instead of re-extracting all sources. It can still make synthesis/verification calls. Source changes pending from a failed mutation are processed normally. A successful subsequent unchanged run makes zero model calls. Cache validation and publication checks remain enabled.

## Validation scope

Regression tests check exact citation enums in both provider request formats, repair when a mock endpoint ignores the schema, historical endpoints below the old sampling cutoff, budget failures, unknown-ID rejection, semantic-verification failure, prior-publication preservation, private diagnostics, presentation-only upgrades, mutation and no-op behavior. These tests establish contract behavior, not model quality. A fresh Atlas run using the configured Foundry endpoint is still needed before proceeding to Lore-self, OpenWiki and human acceptance review.


## Chronology-safe topic synthesis

Topic-page writing can cite only knowledge units included in that exact synthesis batch. Cross-topic decision relationships remain visible to the independent verifier, and source-backed decision history is rendered separately with its exact relationship evidence. A writer must not turn an undated PostgreSQL proposal into a claim that the proposal chronologically preceded a newer ADR, or attach supersession to old design citations that do not support the replacement.

When semantic verification rejects the generated prose, Lore requests two targeted rewrites that specifically remove unsupported before/after, current-state or supersession claims while retaining all local knowledge coverage. Unsupported generated text is never published. If all three semantic checks fail, only that topic batch degrades to **verbatim source excerpts with an explicit “synthesis requires review” marker**. This is documentary evidence, not verified synthesis, and must not be treated as a passing quality result. The CLI's JSON report identifies the affected topic under `degraded_topics`, and the evaluation suite automatically fails the beta gate when the list is nonempty. The documentation and evidence still remain accessible, and a subsequent unchanged update remains a no-op.

The fallback is deliberately limited to topic-level semantic verification failures. Citation-schema errors, invented or missing evidence, extraction failures, provider failures, and semantically unsupported generated *project overviews* still fail closed without publication. Rust's deterministic documentary-decision section continues to cite exact recorded relationship evidence; a source title or publication date is not proof of an event chronology.


## Project-overview verifier refusals

A live Foundry/Ollama Atlas run at `5171f86` reached its project overview during initialization but failed semantic verification before publication. The retained verifier responses identified unsupported interpretation of a generic ADR `Date:` field as a publication date, chronology of an undated architecture review, and scope expansion from a migration-feasibility investigation to an approved or project-wide ledger migration. These were **valid semantic objections**; a correct repair must remove the unsupported claims, not override the verifier.

Overview writing is now explicitly restricted against inferred publication dates, relative chronology from unanchored plans or reviews, and scope expansion from a narrowly documented investigation. It gets up to three attempts with concrete verification feedback. A draft can only be published as a verified narrative when the verifier accepts it.

After persistent semantic rejection, Lore publishes an **evidence-only project index** containing original source excerpts and their supported knowledge IDs. The index has the visible `lore:degraded-overview-synthesis` marker and an explanation that semantic narrative synthesis did not pass. Rejected text is not used or recorded as a replacement explanation. The JSON update report contains `degraded_overview: true` and a warning. On no-op updates that status remains true, rather than silently disappearing from the report. The existing publisher's atomicity, excerpt integrity, and other failures remain fail-closed.

Unlike a verified project overview, the fallback is a navigable source index rather than consolidated historical interpretation. The acceptance suite fails when `degraded_overview` is true, unreported, or present as a page marker. It cannot be mistaken for a passing validated-beta result. Errors in source extraction, invalid citations, provider calls, missing evidence, and non-semantic overview validation still abort normally instead of invoking the fallback.
