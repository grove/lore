//! These are executable provenance and presentation contracts. They do not
//! measure a real learner or imply that documented cases ran in a checkout.
use lore::{
    context,
    domain::{AssertionProposal, ImportKind},
    imports::{
        self, Inventory, SourceBatch,
        adapters::{AdapterRecord, ImportBatch, NativeEvidence, ObservationKind, ObservationScope},
    },
    insights::{self, CaseKind, FacetKind},
    sources::{self, Document},
    storage, util,
};
use rusqlite::Connection;
use serde_json::json;
use std::collections::BTreeSet;

fn database() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    storage::migrate(&conn).unwrap();
    conn.execute(
        "INSERT INTO projects VALUES('p','Dispatch fixture','2026-10-09')",
        [],
    )
    .unwrap();
    conn.execute("INSERT INTO source_roots(id,project_id,configured_path) VALUES('docs','p','/project/docs')", []).unwrap();
    conn
}

struct Spec<'a> {
    quote: &'a str,
    kind: &'a str,
    lifecycle: &'a str,
    scope: &'a str,
    topic: &'a str,
    subject: &'a str,
}

fn spec<'a>(quote: &'a str, kind: &'a str, lifecycle: &'a str) -> Spec<'a> {
    Spec {
        quote,
        kind,
        lifecycle,
        scope: "production",
        topic: "dispatch",
        subject: "dispatch queuing",
    }
}

struct Record {
    id: String,
    assertion: String,
    evidence: String,
}

fn capture(conn: &Connection, path: &str, text: &str, specs: &[Spec<'_>]) -> Vec<Record> {
    let document = Document {
        root_id: "docs".into(),
        root_path: "/project/docs".into(),
        material: Default::default(),
        origin: None,
        relative_path: path.into(),
        physical_path: format!("/project/docs/{path}").into(),
        text: text.into(),
        digest: util::digest(text),
        chunks: sources::split_markdown(text, "docs", path, 8_000).unwrap(),
    };
    let (source, revision) = storage::begin_source(conn, &document, None).unwrap();
    let mut records = Vec::new();
    for item in specs {
        let chunk = document
            .chunks
            .iter()
            .find(|chunk| chunk.text.contains(item.quote))
            .unwrap();
        let (section, section_revision) =
            storage::begin_section(conn, &source, &revision, chunk).unwrap();
        let proposal = AssertionProposal {
            topic: item.topic.into(),
            topic_title: item.topic.into(),
            subject: item.subject.into(),
            statement: item.quote.into(),
            quote: item.quote.into(),
            kind: item.kind.into(),
            lifecycle: item.lifecycle.into(),
            scope: item.scope.into(),
            effective_at: String::new(),
        };
        let (assertion, evidence) = storage::capture_assertion(
            conn,
            storage::AssertionCapture {
                document: &document,
                chunk,
                proposal: &proposal,
                source: &source,
                source_revision: &revision,
                section: &section,
                section_revision: &section_revision,
                model: "fixture-no-inference",
            },
        )
        .unwrap();
        let id = storage::create_unit(conn, "p", &assertion, &proposal).unwrap();
        records.push(Record {
            id,
            assertion,
            evidence,
        });
    }
    storage::refresh_knowledge(conn, "p").unwrap();
    records
}

const DECISION: &str = "Dispatch uses a bounded queue with per-key ordering.";
const RATIONALE: &str = "Dispatch producers outrun consumers during regional failover; a finite queue preserves the memory bound.";
const EXCEPTION: &str = "| Environment | Limit | Exception |\n| --- | --- | --- |\n| production | 128 | Never bypass ordering. |\n| isolated replay | 1 | Only after draining every tenant key; preserve the original request token. |";

fn rich_decision(conn: &Connection) -> Vec<Record> {
    let alternatives =
        "Dispatch alternatives considered: an unbounded queue and synchronous delivery.";
    let tradeoff = "Dispatch bounds memory at the cost of explicit backpressure.";
    let assumptions =
        "Dispatch consumers eventually recover; permanent outage requires operator intervention.";
    let constraint = "Dispatch must never discard an accepted item when the queue is full.";
    let consequence =
        "Dispatch callers must handle a queue-full result without changing the request token.";
    let trigger =
        "Revisit dispatch capacity when failover bursts exceed 128 pending items for ten minutes.";
    let text = format!(
        "# ADR-004: Dispatch\n\n## Decision\n{DECISION}\n\n## Rationale\n{RATIONALE}\n\n## Alternatives\n{alternatives}\n\n## Trade-offs\n{tradeoff}\n\n## Assumptions\n{assumptions}\n\n## Constraints\n{constraint}\n\n## Exceptions\n{EXCEPTION}\n\n## Consequences\n{consequence}\n\n## Reconsideration triggers\n{trigger}\n"
    );
    capture(
        conn,
        "ADR-004.md",
        &text,
        &[
            spec(DECISION, "decision", "accepted"),
            spec(RATIONALE, "design", "active"),
            spec(alternatives, "design", "active"),
            spec(tradeoff, "design", "active"),
            spec(assumptions, "design", "active"),
            spec(constraint, "constraint", "accepted"),
            Spec {
                scope: "production and isolated replay",
                ..spec(EXCEPTION, "constraint", "accepted")
            },
            spec(consequence, "design", "active"),
            spec(trigger, "constraint", "active"),
        ],
    )
}

#[test]
fn decision_lens_keeps_exact_rationale_tradeoffs_and_rare_table_exception() {
    let conn = database();
    let records = rich_decision(&conn);
    let result = insights::decisions(&conn, "dispatch queuing", 32_000).unwrap();
    assert_eq!(result.schema_version, 1);
    assert_eq!(result.model_calls, 0);
    assert_eq!(result.context.schema_version, 2);
    let lens = result
        .lenses
        .iter()
        .find(|lens| lens.record.id == records[0].id)
        .unwrap();
    assert_eq!(lens.record.statement, DECISION);
    assert_eq!(lens.record.lifecycle, "accepted");
    assert_eq!(lens.original_lifecycle, "accepted");
    assert_eq!(lens.record.scope, "production");
    assert!(
        lens.record.effective_at.is_empty(),
        "capture time is not effective time"
    );
    let categories: BTreeSet<_> = lens.facets.iter().map(|facet| facet.kind).collect();
    assert_eq!(
        categories,
        BTreeSet::from([
            FacetKind::Rationale,
            FacetKind::Alternatives,
            FacetKind::TradeOffs,
            FacetKind::Assumptions,
            FacetKind::Constraints,
            FacetKind::Exceptions,
            FacetKind::Consequences,
            FacetKind::ReconsiderationTriggers,
        ])
    );
    let exception = lens
        .facets
        .iter()
        .find(|facet| facet.kind == FacetKind::Exceptions)
        .unwrap();
    assert_eq!(exception.evidence.excerpt, EXCEPTION);
    assert_eq!(exception.evidence_id, records[6].evidence);
    assert_eq!(exception.evidence_id, exception.evidence.id);
    assert_eq!(exception.records[0].scope, "production and isolated replay");
    assert_eq!(exception.evidence.digest, util::digest(EXCEPTION));
    assert_eq!(
        exception.heading_path,
        vec!["ADR-004: Dispatch", "Exceptions"]
    );
    for facet in &lens.facets {
        let original = storage::evidence_snapshot(&conn, &facet.evidence_id).unwrap();
        assert_eq!(
            serde_json::to_value(&facet.evidence).unwrap(),
            serde_json::to_value(original).unwrap()
        );
        assert_eq!(facet.classification_basis, "captured_source_heading");
    }
    assert!(!result.coverage.complete_source_documents);
    assert!(insights::render_decisions(&result).contains("original request token"));
}

#[test]
fn a_rationale_from_another_decision_in_the_same_minutes_is_never_attached() {
    let conn = database();
    let wrong = "The database choice reduces storage licensing costs.";
    let text = format!(
        "# Architecture minutes\n## Dispatch\n### Decision\n{DECISION}\n### Rationale\n{RATIONALE}\n## Database\n### Rationale\n{wrong}\n"
    );
    let records = capture(
        &conn,
        "minutes.md",
        &text,
        &[
            spec(DECISION, "decision", "accepted"),
            spec(RATIONALE, "design", "active"),
            Spec {
                topic: "database",
                subject: "database storage",
                ..spec(wrong, "design", "active")
            },
        ],
    );
    let result = insights::decisions(&conn, "dispatch queuing", 20_000).unwrap();
    let lens = result
        .lenses
        .iter()
        .find(|lens| lens.record.id == records[0].id)
        .unwrap();
    assert_eq!(lens.facets.len(), 1);
    assert_eq!(lens.facets[0].evidence.excerpt, RATIONALE);
    assert!(
        lens.facets
            .iter()
            .all(|facet| facet.evidence_id != records[2].evidence)
    );
    assert!(
        lens.facets
            .iter()
            .flat_map(|facet| &facet.records)
            .all(|record| record.knowledge_id != records[2].id)
    );
    // Original bounded surrounding context may include the next heading. It
    // remains source context and is never attached as a rationale witness.
}

#[test]
fn absent_rationale_is_not_filled_from_general_engineering_expectations() {
    let conn = database();
    let text = format!("# Dispatch\n## Decision\n{DECISION}\n");
    capture(
        &conn,
        "bare-choice.md",
        &text,
        &[spec(DECISION, "decision", "accepted")],
    );
    let result = insights::decisions(&conn, "dispatch", 6_000).unwrap();
    assert_eq!(result.lenses.len(), 1);
    assert!(result.lenses[0].facets.is_empty());
    assert!(
        result
            .coverage
            .qualification
            .contains("Missing fields do not establish")
    );
    assert!(!insights::render_decisions(&result).contains("Documented rationale"));
}

#[test]
fn unstructured_source_markers_cannot_attach_another_choices_rationale() {
    let conn = database();
    let unrelated = "Rationale: the database licensing constraint favors the existing SQL service.";
    let text = format!("{DECISION}\n\n{unrelated}\n");
    capture(
        &conn,
        "plain-notes.md",
        &text,
        &[
            spec(DECISION, "decision", "accepted"),
            Spec {
                topic: "database",
                subject: "database licensing",
                ..spec(unrelated, "design", "active")
            },
        ],
    );
    let result = insights::decisions(&conn, "dispatch", 8_000).unwrap();
    assert_eq!(result.lenses.len(), 1);
    assert!(
        result.lenses[0].facets.is_empty(),
        "a shared unstructured file is not a source-defined decision family"
    );
}

#[test]
fn proposed_facets_rejected_approaches_and_unrelated_risks_keep_source_roles() {
    let conn = database();
    let proposal = "Dispatch could add a dedicated staging queue; this remains a proposal.";
    let rejected =
        "Dispatch rejected an unbounded queue because a regional outage exhausted memory.";
    let unrelated = "The image renderer can exceed its bitmap memory allocation.";
    let text = format!(
        "# Dispatch\n## Decision\n{DECISION}\n## Alternatives\n{proposal}\n## Rejected approaches\n{rejected}\n"
    );
    let records = capture(
        &conn,
        "choices.md",
        &text,
        &[
            spec(DECISION, "decision", "accepted"),
            Spec {
                scope: "staging",
                ..spec(proposal, "proposal", "proposed")
            },
            spec(rejected, "proposal", "rejected"),
        ],
    );
    capture(
        &conn,
        "bitmap.md",
        unrelated,
        &[Spec {
            topic: "graphics",
            subject: "image renderer",
            ..spec(unrelated, "risk", "active")
        }],
    );
    let result = insights::decisions(&conn, "dispatch queuing", 20_000).unwrap();
    let accepted = result
        .lenses
        .iter()
        .find(|lens| lens.record.id == records[0].id)
        .unwrap();
    let alternative = accepted
        .facets
        .iter()
        .find(|facet| facet.kind == FacetKind::Alternatives)
        .unwrap();
    assert_eq!(alternative.records[0].kind, "proposal");
    assert_eq!(alternative.records[0].lifecycle, "proposed");
    assert_eq!(alternative.records[0].scope, "staging");
    assert!(
        result
            .lenses
            .iter()
            .any(|lens| lens.record.id == records[1].id && lens.record.lifecycle == "proposed")
    );
    assert_eq!(result.negative_knowledge.len(), 1);
    assert_eq!(result.negative_knowledge[0].knowledge_id, records[2].id);
    assert_eq!(
        result.negative_knowledge[0].basis,
        "retained_rejected_lifecycle"
    );
    assert!(!serde_json::to_string(&result).unwrap().contains(unrelated));
}

#[test]
fn explicit_replacement_preserves_original_acceptance_and_the_relationship_witness() {
    let conn = database();
    let old = "Dispatch originally allowed 64 queued items.";
    let new =
        "Dispatch now allows 128 queued items; this explicitly replaces the 64-item decision.";
    let old_records = capture(
        &conn,
        "ADR-old.md",
        old,
        &[spec(old, "decision", "accepted")],
    );
    let new_records = capture(
        &conn,
        "ADR-new.md",
        new,
        &[spec(new, "decision", "accepted")],
    );
    storage::add_relation(
        &conn,
        &new_records[0].id,
        &old_records[0].id,
        "supersedes",
        &new_records[0].assertion,
        &new_records[0].evidence,
    )
    .unwrap();
    storage::refresh_knowledge(&conn, "p").unwrap();
    let result = insights::decisions(&conn, "dispatch queuing", 16_000).unwrap();
    let old_lens = result
        .lenses
        .iter()
        .find(|lens| lens.record.id == old_records[0].id)
        .unwrap();
    let new_lens = result
        .lenses
        .iter()
        .find(|lens| lens.record.id == new_records[0].id)
        .unwrap();
    assert_eq!(old_lens.record.lifecycle, "superseded");
    assert_eq!(old_lens.original_lifecycle, "accepted");
    assert_eq!(new_lens.record.lifecycle, "accepted");
    for lens in [old_lens, new_lens] {
        assert_eq!(lens.history.len(), 1);
        let relation = &lens.history[0];
        assert_eq!(relation.from, new_records[0].id);
        assert_eq!(relation.to, old_records[0].id);
        assert_eq!(relation.kind, "supersedes");
        assert_eq!(relation.evidence_id, new_records[0].evidence);
        assert!(
            result
                .context
                .evidence
                .iter()
                .any(|e| e.id == relation.evidence_id)
        );
    }
}

#[test]
fn documented_case_keeps_start_expected_and_reported_actual_distinct_without_execution() {
    let conn = database();
    let start = "Dispatch begins with a drained queue and one tenant key.";
    let procedure = "Dispatch replay procedure: enqueue two requests with the same key, then release the consumer.";
    let expected = "Dispatch should deliver the requests in submission order.";
    let reported =
        "The dispatch incident report records the second request arriving first in staging.";
    let reason = "The dispatch incident report attributes the reversal to parallel consumers.";
    let text = format!(
        "# Dispatch replay\n## Starting conditions\n{start}\n## Procedure\n{procedure}\n## Expected behavior\n{expected}\n## Actual behavior\n{reported}\n## Why this failed\n{reason}\n"
    );
    let records = capture(
        &conn,
        "incident.md",
        &text,
        &[
            spec(start, "design", "active"),
            spec(procedure, "procedure", "active"),
            spec(expected, "design", "active"),
            Spec {
                scope: "staging",
                ..spec(reported, "reported_outcome", "completed")
            },
            Spec {
                scope: "staging",
                ..spec(reason, "observation", "active")
            },
        ],
    );
    let result = insights::cases(&conn, "dispatch replay", 32_000).unwrap();
    let case = result
        .cases
        .iter()
        .find(|case| case.record.as_ref().is_some_and(|r| r.id == records[1].id))
        .unwrap();
    assert_eq!(case.kind, CaseKind::DocumentedProcedure);
    let facet = |kind| {
        &case
            .facets
            .iter()
            .find(|facet| facet.kind == kind)
            .unwrap()
            .evidence
            .excerpt
    };
    assert_eq!(facet(FacetKind::StartingConditions), start);
    assert_eq!(facet(FacetKind::ExpectedBehavior), expected);
    assert_eq!(facet(FacetKind::ReportedOutcome), reported);
    assert_eq!(facet(FacetKind::Rationale), reason);
    assert!(!result.runtime_execution);
    assert_eq!(result.tests_run, 0);
    assert_eq!(result.model_calls, 0);
    assert!(case.qualification.contains("has not executed or replayed"));
    assert!(insights::render_cases(&result).contains("Source-reported outcome"));
}

#[test]
fn rejected_approach_is_a_case_and_documented_risk_is_not_an_observed_failure() {
    let conn = database();
    let rejected = "Dispatch rejected retrying every timeout without preserving the request token.";
    let risk = "Dispatch risks duplicate work if the caller changes the request token.";
    let text =
        format!("# Dispatch\n## Rejected approaches\n{rejected}\n## Failure modes\n{risk}\n");
    capture(
        &conn,
        "negative.md",
        &text,
        &[
            spec(rejected, "proposal", "rejected"),
            spec(risk, "risk", "active"),
        ],
    );
    let result = insights::cases(&conn, "dispatch request token", 16_000).unwrap();
    assert!(
        result
            .cases
            .iter()
            .any(|case| case.kind == CaseKind::RejectedApproach)
    );
    assert!(
        result
            .cases
            .iter()
            .any(|case| case.kind == CaseKind::DocumentedRisk)
    );
    assert!(
        !result
            .cases
            .iter()
            .any(|case| case.kind == CaseKind::SourceReportedOutcome)
    );
    assert!(
        result
            .cases
            .iter()
            .all(|case| case.record.as_ref().unwrap().lifecycle != "completed")
    );
}

#[test]
fn closed_imported_issue_is_source_owned_history_and_carries_native_evidence() {
    let conn = database();
    let statement = "Dispatch ordering fix was marked closed after a staging replay.";
    let mut record = AdapterRecord::new(
        "dispatch-42",
        statement,
        json!({"id":"dispatch-42","status":"closed","statement":statement}),
    );
    record.kind = ObservationKind::WorkState;
    record.title = "Dispatch ordering fix".into();
    record.subject = "dispatch queuing".into();
    record.lifecycle = "closed".into();
    record.scope = ObservationScope {
        environment: Some("staging".into()),
        ..Default::default()
    };
    record.evidence = vec![NativeEvidence {
        locator: "record://dispatch-42".into(),
        revision: Some("issue-revision-8".into()),
        field: Some("/statement".into()),
    }];
    let batch = ImportBatch {
        format: "fixture-v1".into(),
        records: vec![record],
        warnings: vec![],
    };
    let source = SourceBatch {
        id: "work".into(),
        kind: ImportKind::Beads,
        path: "/project/issues.jsonl".into(),
        digest: util::json_digest(&batch).unwrap(),
        batch,
    };
    imports::storage::persist(
        &conn,
        "p",
        &Inventory {
            sources: vec![source],
            digest: "fixture".into(),
            warnings: vec![],
        },
    )
    .unwrap();
    let result = insights::cases(&conn, "dispatch queuing", 8_000).unwrap();
    assert_eq!(result.cases.len(), 1);
    let case = &result.cases[0];
    assert_eq!(case.kind, CaseKind::WorkHistory);
    assert!(case.record.is_none());
    let imported = case.imported_record.as_ref().unwrap();
    assert_eq!(imported.lifecycle, "closed");
    assert_eq!(imported.scope.environment.as_deref(), Some("staging"));
    assert_eq!(case.evidence_ids, imported.evidence_ids);
    assert!(!case.evidence_ids.is_empty());
    assert!(
        case.evidence_ids.iter().all(|id| result
            .context
            .imported_evidence
            .iter()
            .any(|e| e.id == *id))
    );
    assert!(
        case.qualification
            .contains("do not prove current code behavior")
    );
    assert!(!result.runtime_execution);
}

#[test]
fn every_returned_view_fits_its_full_json_and_markdown_budget_without_slicing_facets() {
    let conn = database();
    rich_decision(&conn);
    let mut retained = false;
    let mut bounded_omission = false;
    for limit in [512, 1_000, 2_000, 6_000, 12_000, 32_000] {
        match insights::decisions(&conn, "dispatch", limit) {
            Ok(result) => {
                let used = context::count_tokens(&(serde_json::to_string(&result).unwrap() + "\n"))
                    .max(context::count_tokens(&insights::render_decisions(&result)));
                assert!(result.budget.used_tokens >= used);
                assert!(result.budget.used_tokens <= limit);
                assert!(used <= limit);
                bounded_omission |= result.coverage.omitted_complete_items > 0;
                for lens in result.lenses {
                    retained = true;
                    let facet = lens
                        .facets
                        .iter()
                        .find(|facet| facet.kind == FacetKind::Exceptions)
                        .unwrap();
                    assert_eq!(
                        facet.evidence.excerpt, EXCEPTION,
                        "a retained lens keeps the complete rare exception"
                    );
                }
            }
            Err(error) => assert_eq!(
                error.downcast_ref::<context::ContextError>().unwrap().code,
                "invalid_budget"
            ),
        }
        if let Ok(result) = insights::cases(&conn, "dispatch", limit) {
            let used = context::count_tokens(&(serde_json::to_string(&result).unwrap() + "\n"))
                .max(context::count_tokens(&insights::render_cases(&result)));
            assert!(result.budget.used_tokens >= used);
            assert!(result.budget.used_tokens <= limit);
            assert!(used <= limit);
        }
    }
    assert!(retained);
    assert!(
        bounded_omission,
        "bounded mode must disclose omitted whole lenses"
    );
}

#[test]
fn views_are_deterministic_and_can_run_with_database_writes_disabled() {
    let conn = database();
    rich_decision(&conn);
    let revision = storage::registry_revision(&conn).unwrap();
    let changes = conn.total_changes();
    conn.execute_batch("PRAGMA query_only=ON").unwrap();
    let first = insights::decisions(&conn, "dispatch", 24_000).unwrap();
    let second = insights::decisions(&conn, "dispatch", 24_000).unwrap();
    assert_eq!(
        serde_json::to_value(first).unwrap(),
        serde_json::to_value(second).unwrap()
    );
    let first_case = insights::cases(&conn, "dispatch", 24_000).unwrap();
    let second_case = insights::cases(&conn, "dispatch", 24_000).unwrap();
    assert_eq!(
        serde_json::to_value(first_case).unwrap(),
        serde_json::to_value(second_case).unwrap()
    );
    assert_eq!(revision, storage::registry_revision(&conn).unwrap());
    assert_eq!(changes, conn.total_changes());
}

#[test]
fn explicit_marker_uses_whole_original_quote_and_unicode_bytes_are_preserved() {
    let conn = database();
    let quote = "Rationale: dispatch preserves café tenant order.\r\nException: isolated replay still preserves the token.\r\n";
    let text = format!("# Dispatch\r\n{DECISION}\r\n{quote}");
    capture(
        &conn,
        "unicode.md",
        &text,
        &[
            spec(DECISION, "decision", "accepted"),
            spec(quote, "design", "active"),
        ],
    );
    let result = insights::decisions(&conn, "dispatch", 12_000).unwrap();
    let facet = &result.lenses[0].facets[0];
    assert_eq!(
        facet.classification_basis,
        "explicit_retained_source_marker"
    );
    assert_eq!(facet.evidence.excerpt.as_bytes(), quote.as_bytes());
    assert_eq!(facet.evidence.digest, util::digest(quote));
    assert!(
        facet.evidence.excerpt.contains("Exception:"),
        "marker categorization never slices later conditions"
    );
}
