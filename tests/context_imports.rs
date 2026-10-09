//! Cross-source task context uses persisted observations and interpretations.
//! Fixtures record relationships explicitly; retrieval must never infer them.
use lore::{
    context::{self, ContextOptions, ContextResult},
    domain::{AssertionProposal, ImportKind, KnowledgeView, SourceMaterial},
    imports::{
        self, ImportedObservation, Inventory, SourceBatch,
        adapters::{
            AdapterRecord, ImportBatch, NativeEvidence, NativeRelationship, ObservationKind,
            ObservationVerification,
        },
        relationships::{CrossSourceEndpoint, CrossSourceRelation},
    },
    sources::{self, Document},
    storage, util,
};
use rusqlite::{Connection, params};
use serde_json::{Value, json};
use std::collections::BTreeSet;

fn database() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    storage::migrate(&conn).unwrap();
    conn.execute(
        "INSERT INTO projects VALUES('p','Context native fixture','2026-10-09')",
        [],
    )
    .unwrap();
    conn.execute("INSERT INTO source_roots(id,project_id,configured_path) VALUES('docs','p','/project/docs')", []).unwrap();
    conn
}

fn native(id: &str, statement: &str, kind: ObservationKind) -> AdapterRecord {
    let mut record = AdapterRecord::new(id, statement, json!({"id":id,"statement":statement}));
    record.kind = kind;
    record.subject = "retry policy".into();
    record.title = statement.into();
    record.evidence = vec![NativeEvidence {
        locator: format!("record://{id}"),
        revision: None,
        field: Some("/statement".into()),
    }];
    record
}

fn source(id: &str, kind: ImportKind, records: Vec<AdapterRecord>) -> SourceBatch {
    let batch = ImportBatch {
        format: "native-context-fixture-v1".into(),
        records,
        warnings: vec![],
    };
    SourceBatch {
        id: id.into(),
        kind,
        path: format!("/project/imports/{id}").into(),
        digest: util::json_digest(&batch).unwrap(),
        batch,
    }
}

fn persist(conn: &Connection, sources: Vec<SourceBatch>) -> Vec<ImportedObservation> {
    imports::storage::persist(
        conn,
        "p",
        &Inventory {
            sources,
            digest: "fixture".into(),
            warnings: vec![],
        },
    )
    .unwrap();
    imports::views(conn).unwrap()
}

fn decision(conn: &Connection) -> KnowledgeView {
    let text = "The accepted dispatch policy permits at most three attempts in production.";
    let document = Document {
        root_id: "docs".into(),
        root_path: "/project/docs".into(),
        material: SourceMaterial::Primary,
        origin: None,
        relative_path: "ADR-017.md".into(),
        physical_path: "/project/docs/ADR-017.md".into(),
        text: text.into(),
        digest: util::digest(text),
        chunks: sources::split_markdown(text, "docs", "ADR-017.md", 8_000).unwrap(),
    };
    let proposal = AssertionProposal {
        topic: "delivery".into(),
        topic_title: "Delivery".into(),
        subject: "dispatch policy".into(),
        statement: text.into(),
        kind: "decision".into(),
        lifecycle: "accepted".into(),
        scope: "production".into(),
        effective_at: String::new(),
        quote: text.into(),
    };
    let (source_id, revision_id) = storage::begin_source(conn, &document, None).unwrap();
    let chunk = &document.chunks[0];
    let (section, section_revision) =
        storage::begin_section(conn, &source_id, &revision_id, chunk).unwrap();
    let (assertion, _) = storage::capture_assertion(
        conn,
        storage::AssertionCapture {
            document: &document,
            chunk,
            proposal: &proposal,
            source: &source_id,
            source_revision: &revision_id,
            section: &section,
            section_revision: &section_revision,
            model: "fixture-no-model",
        },
    )
    .unwrap();
    storage::create_unit(conn, "p", &assertion, &proposal).unwrap();
    storage::refresh_knowledge(conn, "p").unwrap();
    storage::views(conn).unwrap().remove(0)
}

fn endpoint(observation: &ImportedObservation) -> CrossSourceEndpoint {
    CrossSourceEndpoint {
        kind: "observation".into(),
        id: observation.id.clone(),
        revision_id: observation.snapshot_id.clone(),
    }
}

fn relation(
    conn: &Connection,
    from: CrossSourceEndpoint,
    to: Option<CrossSourceEndpoint>,
    kind: &str,
    evidence_ids: Vec<String>,
    review: Option<&str>,
) -> CrossSourceRelation {
    let signature = util::json_digest(&(&from, &to, kind)).unwrap();
    let id = format!("xrel_{}", &signature[7..]);
    let record: CrossSourceRelation = serde_json::from_value(json!({
        "id":id,"input_signature":signature,"from":from,"to":to,"kind":kind,
        "reason":"The cited intent and reported implementation differ; verify applicability before changing behavior.",
        "qualifications":["Potential relationship only; current implementation and policy replacement have not been independently verified."],
        "evidence_ids":evidence_ids,"upstream_kind":null,"review_id":review,"active":true
    })).unwrap();
    conn.execute(
        "INSERT INTO cross_source_evaluations VALUES(?1,?1,?2,?3,?4,?5,?6,?7,?8,'2026-10-09')",
        params![
            signature,
            from.kind,
            from.id,
            from.revision_id,
            to.as_ref().map(|e| &e.kind),
            to.as_ref().map(|e| &e.id),
            to.as_ref().map(|e| &e.revision_id),
            kind
        ],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO cross_source_current VALUES(?1,?1)",
        [&signature],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO cross_source_relations VALUES(?1,?2,?3)",
        params![id, signature, serde_json::to_string(&record).unwrap()],
    )
    .unwrap();
    record
}

fn build(conn: &Connection, task: &str, paths: &[&str], max_tokens: usize) -> ContextResult {
    context::build_context(
        conn,
        &ContextOptions {
            task: task.into(),
            paths: paths.iter().map(|p| p.to_string()).collect(),
            max_tokens,
        },
    )
    .unwrap()
}

fn assert_evidence(conn: &Connection, result: &ContextResult) {
    let evidence_ids: BTreeSet<_> = result
        .evidence
        .iter()
        .map(|e| e.id.clone())
        .chain(result.imported_evidence.iter().map(|e| e.id.clone()))
        .collect();
    let record_ids: BTreeSet<_> = result
        .sections
        .items()
        .map(|i| i.id.clone())
        .chain(result.imported_observations.iter().map(|i| i.id.clone()))
        .collect();
    for observation in &result.imported_observations {
        assert!(!observation.evidence_ids.is_empty());
        assert!(
            observation
                .evidence_ids
                .iter()
                .all(|id| evidence_ids.contains(id))
        );
    }
    for relation in &result.cross_source_relations {
        assert!(
            relation
                .endpoints()
                .all(|endpoint| record_ids.contains(&endpoint.id))
        );
        assert!(!relation.evidence_ids.is_empty());
        assert!(
            relation
                .evidence_ids
                .iter()
                .all(|id| evidence_ids.contains(id))
        );
    }
    for item in &result.recommended_verification {
        assert!(item.record_ids.iter().all(|id| record_ids.contains(id)));
        assert!(item.evidence_ids.iter().all(|id| evidence_ids.contains(id)));
    }
    for citation in &result.imported_evidence {
        let archived = imports::evidence(conn, &citation.id).unwrap();
        assert_eq!(archived.content_hash, citation.content_hash);
        assert_eq!(archived.snapshot_id, citation.snapshot_id);
        for locator in &citation.locators {
            if let Some(field) = &locator.field {
                assert!(archived.record.native_record.pointer(field).is_some());
            }
        }
    }
    assert_eq!(result.model_calls, 0);
    assert_eq!(
        conn.query_row::<u64, _, _>("SELECT count(*) FROM model_calls", [], |row| row.get(0))
            .unwrap(),
        0
    );
}

#[test]
fn native_identifier_retrieves_documented_intent_and_complete_discrepancy() {
    let conn = database();
    let decision = decision(&conn);
    let mut implementation = native(
        "retry-limit",
        "The retry limit is five in the reported implementation.",
        ObservationKind::Implementation,
    );
    implementation.verification = ObservationVerification::UpstreamVerifiedAtRevision;
    implementation.evidence.push(NativeEvidence {
        locator: "repo://src/payments/retry.rs#L42-L48".into(),
        revision: Some("repo-lines-v1:opaque-digest".into()),
        field: None,
    });
    let observation = persist(
        &conn,
        vec![source(
            "implementation",
            ImportKind::Openwiki,
            vec![implementation],
        )],
    )
    .remove(0);
    relation(
        &conn,
        endpoint(&observation),
        Some(CrossSourceEndpoint {
            kind: "knowledge".into(),
            id: decision.id.clone(),
            revision_id: decision.revision_id,
        }),
        "potential_discrepancy",
        vec![
            observation.evidence_id.clone(),
            decision.evidence[0].id.clone(),
        ],
        None,
    );
    conn.pragma_update(None, "query_only", true).unwrap();
    let before: u64 = conn
        .query_row("SELECT total_changes()", [], |row| row.get(0))
        .unwrap();
    let result = build(&conn, "Inspect retry-limit", &[], 8_000);
    assert_eq!(result.schema_version, 2);
    assert_eq!(result.imported_observations.len(), 1);
    assert_eq!(result.sections.decisions.len(), 1);
    assert_eq!(result.sections.decisions[0].id, decision.id);
    assert_eq!(result.discrepancies.len(), 1);
    assert_eq!(result.cross_source_relations.len(), 1);
    assert_eq!(result.recommended_verification.len(), 1);
    assert!(
        result.imported_observations[0]
            .relevance
            .iter()
            .any(|r| r.contains("native record identifier"))
    );
    assert!(
        result
            .warnings
            .iter()
            .any(|warning| warning.contains("freshness") && warning.contains("current checkout"))
    );
    let human = context::render_context(&result);
    for heading in [
        "Documented intent",
        "Observed implementation",
        "Potential discrepancies",
        "Recommended verification",
    ] {
        assert!(human.contains(heading), "missing {heading}: {human}");
    }
    assert_evidence(&conn, &result);
    let repeated = build(&conn, "Inspect retry-limit", &[], 8_000);
    assert_eq!(
        serde_json::to_value(&result).unwrap(),
        serde_json::to_value(repeated).unwrap()
    );
    assert_eq!(
        before,
        conn.query_row::<u64, _, _>("SELECT total_changes()", [], |row| row.get(0))
            .unwrap()
    );
}

#[test]
fn cross_source_groups_are_omitted_whole_and_both_formats_fit_the_budget() {
    let conn = database();
    let decision = decision(&conn);
    let observation = persist(
        &conn,
        vec![source(
            "implementation",
            ImportKind::Openwiki,
            vec![native(
                "limit",
                "Retry handling differs from the documented policy.",
                ObservationKind::Implementation,
            )],
        )],
    )
    .remove(0);
    relation(
        &conn,
        endpoint(&observation),
        Some(CrossSourceEndpoint {
            kind: "knowledge".into(),
            id: decision.id,
            revision_id: decision.revision_id,
        }),
        "potential_discrepancy",
        vec![observation.evidence_id, decision.evidence[0].id.clone()],
        None,
    );
    let full = build(&conn, "Inspect limit", &[], 10_000);
    assert_eq!(full.imported_observations.len(), 1);
    assert_eq!(full.sections.items().count(), 1);
    for budget in [
        512,
        full.budget.used_tokens - 100,
        full.budget.used_tokens,
        10_000,
    ] {
        let result = build(&conn, "Inspect limit", &[], budget);
        let included = result.imported_observations.len();
        assert_eq!(included, result.sections.items().count());
        assert_eq!(included, result.discrepancies.len());
        if included == 0 {
            assert!(result.empty);
            assert_eq!(result.omissions.knowledge_units, 1);
            assert_eq!(result.omissions.imported_observations, 1);
            assert_eq!(result.omissions.critical_groups, 1);
            assert_eq!(result.omissions.discrepancies, 1);
            assert!(result.evidence.is_empty());
            assert!(result.imported_evidence.is_empty());
            assert!(
                result
                    .warnings
                    .iter()
                    .any(|warning| warning.contains("incomplete for a decision"))
            );
        }
        let json = serde_json::to_string(&result).unwrap() + "\n";
        let text = context::render_context(&result);
        assert!(context::count_tokens(&json) <= result.budget.used_tokens);
        assert!(context::count_tokens(&text) <= result.budget.used_tokens);
        assert!(result.budget.used_tokens <= budget);
        assert_evidence(&conn, &result);
    }
}

#[test]
fn native_issue_references_components_and_paths_expand_locally() {
    let conn = database();
    let mut first = native(
        "PAY-17",
        "Use a dedicated handler.",
        ObservationKind::WorkState,
    );
    first.scope.component = Some("payment-processing".into());
    let mut second = native(
        "PAY-22",
        "Inspect upstream behavior.",
        ObservationKind::WorkState,
    );
    second.relationships.push(NativeRelationship {
        native_id: None,
        target_native_id: "PAY-17".into(),
        kind: "blocks".into(),
        upstream_status: None,
        active: true,
        native_record: json!({"blocks":"PAY-17"}),
    });
    second.scope.component = Some("payment-processing".into());
    second.evidence.push(NativeEvidence {
        locator: "repo://src/payments/worker.rs#L2-L8".into(),
        revision: None,
        field: None,
    });
    let mut unrelated = native(
        "OTHER-7",
        "A separate repository uses another handler.",
        ObservationKind::WorkState,
    );
    unrelated.scope.repository = Some("other-project".into());
    unrelated.scope.component = Some("payment-processing".into());
    first.scope.repository = Some("payments".into());
    second.scope.repository = Some("payments".into());
    persist(
        &conn,
        vec![source(
            "work-history",
            ImportKind::Beads,
            vec![first, second, unrelated],
        )],
    );
    let by_id = build(&conn, "Inspect PAY-17", &[], 10_000);
    let ids: BTreeSet<_> = by_id
        .imported_observations
        .iter()
        .map(|o| o.native_id.as_str())
        .collect();
    assert!(ids.contains("PAY-17") && ids.contains("PAY-22"));
    assert!(!ids.contains("OTHER-7"));
    assert!(
        by_id
            .imported_observations
            .iter()
            .find(|o| o.native_id == "PAY-22")
            .unwrap()
            .relevance
            .iter()
            .any(|r| r.contains("upstream issue or record reference"))
    );
    let component = build(&conn, "Change payment-processing", &[], 10_000);
    assert!(component.imported_observations.iter().any(|o| {
        o.relevance
            .iter()
            .any(|r| r.contains("source-reported component"))
    }));
    let path = build(
        &conn,
        "zygomorphic",
        &["work-history:src/payments/worker.rs"],
        10_000,
    );
    assert!(path.imported_observations.iter().any(|o| {
        o.native_id == "PAY-22"
            && o.relevance
                .iter()
                .any(|r| r.contains("upstream evidence path"))
    }));
    let other_root = build(
        &conn,
        "zygomorphic",
        &["another-import:src/payments/worker.rs"],
        10_000,
    );
    assert!(other_root.imported_observations.is_empty());
    let quoted = build(&conn, "Inspect PAY-17 OR \"unterminated", &[], 10_000);
    assert!(!quoted.imported_observations.is_empty());
}

#[test]
fn closed_work_deleted_memories_and_retired_imports_keep_source_authority() {
    let conn = database();
    let mut closed = native(
        "PAY-17",
        "The issue reports the retry rollout completed.",
        ObservationKind::WorkState,
    );
    closed.lifecycle = "closed".into();
    let mut memory = native(
        "memory-2",
        "An agent recalled testing retries successfully.",
        ObservationKind::Recollection,
    );
    memory.lifecycle = "deleted".into();
    let sources = vec![
        source("work", ImportKind::Beads, vec![closed]),
        source("memory", ImportKind::Engram, vec![memory]),
    ];
    persist(&conn, sources);
    let result = build(&conn, "Change retry behavior", &[], 10_000);
    assert!(result.sections.constraints.is_empty());
    assert!(result.sections.decisions.is_empty());
    let closed = result
        .imported_observations
        .iter()
        .find(|o| o.kind == ObservationKind::WorkState)
        .unwrap();
    assert_eq!(closed.lifecycle, "closed");
    assert!(
        closed
            .qualifications
            .iter()
            .any(|q| q.contains("does not verify implementation behavior or approve a decision"))
    );
    let memory = result
        .imported_observations
        .iter()
        .find(|o| o.kind == ObservationKind::Recollection)
        .unwrap();
    assert!(memory.current); // Present in the latest export, but deleted upstream.
    assert_eq!(memory.freshness, "upstream_historical_or_withdrawn");
    assert!(
        memory
            .qualifications
            .iter()
            .any(|q| q.contains("historical or withdrawn"))
    );
    let original_evidence = result.imported_evidence[0].id.clone();
    persist(&conn, vec![]);
    let historical = build(&conn, "Change retry behavior", &[], 10_000);
    assert!(!historical.imported_observations.is_empty());
    assert!(
        historical
            .imported_observations
            .iter()
            .all(|o| !o.current && o.freshness == "historical_import")
    );
    assert!(imports::evidence(&conn, &original_evidence).is_ok());
    assert_evidence(&conn, &historical);
}

#[test]
fn native_retrieval_bounds_and_reconciliation_warnings_are_explicit() {
    let conn = database();
    let records = (0..65)
        .map(|n| {
            native(
                &format!("observation-{n}"),
                "Retry behavior was reported by an agent.",
                ObservationKind::Recollection,
            )
        })
        .collect();
    let mut source = source("memories", ImportKind::Engram, records);
    source
        .batch
        .warnings
        .push("Source export omits deleted history.".into());
    persist(&conn, vec![source]);
    conn.execute(
        "INSERT INTO cross_source_state VALUES('warnings',?1)",
        [serde_json::to_string(&vec![
            "Cross-source reconciliation reached its candidate bound.",
        ])
        .unwrap()],
    )
    .unwrap();
    let result = build(&conn, "Change retry behavior", &[], 10_000);
    assert!(result.retrieval_truncated);
    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.contains("omits deleted history"))
    );
    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.contains("reconciliation reached its candidate bound"))
    );
    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.contains("retrieved candidates only"))
    );
    assert!(result.omissions.imported_observations > 0);
    assert_evidence(&conn, &result);
}

#[test]
fn thousands_of_global_diagnostics_cannot_consume_the_task_context_budget() {
    let conn = database();
    let mut imported = source(
        "implementation",
        ImportKind::Openwiki,
        vec![native(
            "impl-1",
            "The upstream tool reports a retry implementation.",
            ObservationKind::Implementation,
        )],
    );
    imported.batch.warnings = (0..2_000)
        .map(|n| {
            format!(
                "Unverified page {n:04}: {}",
                "Long diagnostic 日本語 \u{1b}[31m ".repeat(30)
            )
        })
        .collect();
    persist(&conn, vec![imported]);
    let result = build(&conn, "Inspect impl-1", &[], context::DEFAULT_MAX_TOKENS);
    assert_eq!(result.imported_observations.len(), 1);
    assert_eq!(result.omissions.source_warnings, 2_000);
    assert!(!result.retrieval_truncated); // Diagnostic bounds are separately counted.
    assert!(
        result
            .warnings
            .iter()
            .any(|warning| warning.contains("2000 of 2000 warnings omitted or shortened"))
    );
    assert!(result.warnings.len() <= 5);
    assert!(!context::render_context(&result).contains('\u{1b}'));
    assert!(result.budget.used_tokens <= context::DEFAULT_MAX_TOKENS);
    assert_evidence(&conn, &result);
}

#[test]
fn review_dispositions_are_visible_without_becoming_implementation_verification() {
    let conn = database();
    let observation = persist(
        &conn,
        vec![source(
            "memory",
            ImportKind::Engram,
            vec![native(
                "memory-2",
                "The retry bug was reportedly fixed.",
                ObservationKind::Recollection,
            )],
        )],
    )
    .remove(0);
    conn.execute(
        "INSERT INTO review_items VALUES('review-native','p','Check reported behavior','pending')",
        [],
    )
    .unwrap();
    relation(
        &conn,
        endpoint(&observation),
        None,
        "verification_question",
        vec![observation.evidence_id],
        Some("review-native"),
    );
    let pending = build(&conn, "Inspect memory-2", &[], 10_000);
    assert_eq!(pending.reviews.len(), 1);
    assert_eq!(pending.discrepancies[0].status, "pending");
    lore::reviews::manual(
        &conn,
        "review-native",
        "dismissed",
        "Reviewed the source report; track the remaining verification separately.",
        "fixture-reviewer",
    )
    .unwrap();
    let dismissed = build(&conn, "Inspect memory-2", &[], 10_000);
    assert!(dismissed.reviews.is_empty());
    assert_eq!(dismissed.discrepancies[0].status, "dismissed");
    assert!(
        dismissed.discrepancies[0]
            .qualifications
            .iter()
            .any(|q| q.contains("does not independently verify"))
    );
    assert_evidence(&conn, &dismissed);
}

#[test]
fn stale_interpretation_cannot_silently_bind_a_new_native_revision() {
    let conn = database();
    let first = native(
        "memory-2",
        "The retry bug was reportedly fixed.",
        ObservationKind::Recollection,
    );
    let observation = persist(
        &conn,
        vec![source("memory", ImportKind::Engram, vec![first])],
    )
    .remove(0);
    relation(
        &conn,
        endpoint(&observation),
        None,
        "verification_question",
        vec![observation.evidence_id],
        None,
    );
    let replacement = native(
        "memory-2",
        "The agent withdrew the earlier retry report.",
        ObservationKind::Recollection,
    );
    persist(
        &conn,
        vec![source("memory", ImportKind::Engram, vec![replacement])],
    );
    let error = context::build_context(
        &conn,
        &ContextOptions {
            task: "Inspect memory-2".into(),
            paths: vec![],
            max_tokens: 10_000,
        },
    )
    .unwrap_err();
    assert_eq!(
        error.downcast_ref::<context::ContextError>().unwrap().code,
        "invalid_registry"
    );
    assert!(error.to_string().contains("revision/evidence validation"));
}

#[test]
fn a_cross_source_citation_cannot_be_substituted_with_an_unrelated_native_record() {
    let conn = database();
    let observations = persist(
        &conn,
        vec![source(
            "memory",
            ImportKind::Engram,
            vec![
                native(
                    "alpha-17",
                    "The rollout was reportedly completed.",
                    ObservationKind::Recollection,
                ),
                native(
                    "beta-92",
                    "A different rollout was inspected.",
                    ObservationKind::Recollection,
                ),
            ],
        )],
    );
    let from = observations
        .iter()
        .find(|observation| observation.record.native_id == "alpha-17")
        .unwrap();
    let unrelated = observations
        .iter()
        .find(|observation| observation.record.native_id == "beta-92")
        .unwrap();
    relation(
        &conn,
        endpoint(from),
        None,
        "verification_question",
        vec![unrelated.evidence_id.clone()],
        None,
    );
    let error = context::build_context(
        &conn,
        &ContextOptions {
            task: "Inspect alpha-17".into(),
            paths: vec![],
            max_tokens: 10_000,
        },
    )
    .unwrap_err();
    assert_eq!(
        error.downcast_ref::<context::ContextError>().unwrap().code,
        "invalid_registry"
    );
    assert!(
        error
            .to_string()
            .contains("exactly the supplied endpoint revisions")
    );
}

#[test]
fn old_document_only_context_remains_readable_without_native_migration() {
    let conn = Connection::open_in_memory().unwrap();
    for migration in [
        storage::SCHEMA_V1,
        storage::SCHEMA_V2,
        storage::SCHEMA_V3,
        storage::SCHEMA_V4,
        storage::SCHEMA_V5,
    ] {
        conn.execute_batch(migration).unwrap();
    }
    conn.pragma_update(None, "query_only", true).unwrap();
    let result = build(&conn, "Inspect dispatch", &[], 3_000);
    assert!(result.empty);
    assert!(result.imported_observations.is_empty());
    assert!(result.discrepancies.is_empty());
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(version, 5);
    let json: Value = serde_json::to_value(result).unwrap();
    for key in [
        "sections",
        "evidence",
        "relations",
        "reviews",
        "budget",
        "warnings",
        "omissions",
    ] {
        assert!(json.get(key).is_some(), "v1 field {key} was removed");
    }
}
