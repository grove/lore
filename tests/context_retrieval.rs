//! Retrieval contracts use a real SQLite registry and immutable evidence,
//! without requiring a model, a generated wiki, or a dataset-specific alias.
use lore::{
    context::{self, ContextOptions, retrieval},
    domain::AssertionProposal,
    sources::{self, Document},
    storage, util,
};
use rusqlite::{Connection, params};

fn database() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    storage::migrate(&conn).unwrap();
    conn.execute(
        "INSERT INTO projects VALUES('p','Example','2026-10-09')",
        [],
    )
    .unwrap();
    for root in ["alpha", "beta"] {
        conn.execute(
            "INSERT INTO source_roots(id,project_id,configured_path) VALUES(?1,'p',?2)",
            params![root, format!("./{root}")],
        )
        .unwrap();
    }
    conn
}

struct Record {
    id: String,
    source: String,
    assertion: String,
    evidence: String,
}

fn record(
    conn: &Connection,
    root: &str,
    path: &str,
    topic: &str,
    subject: &str,
    kind: &str,
    text: &str,
) -> Record {
    let document = Document {
        root_id: root.into(),
        root_path: format!("./{root}").into(),
        material: Default::default(),
        origin: None,
        relative_path: path.into(),
        physical_path: path.into(),
        text: text.into(),
        digest: util::digest(text),
        chunks: sources::split_markdown(text, root, path, 8_000).unwrap(),
    };
    let proposal = AssertionProposal {
        topic: topic.into(),
        topic_title: topic.into(),
        subject: subject.into(),
        statement: text.into(),
        kind: kind.into(),
        lifecycle: if kind == "decision" {
            "accepted"
        } else {
            "active"
        }
        .into(),
        scope: "production".into(),
        effective_at: String::new(),
        quote: text.into(),
    };
    let (source, source_revision) = storage::begin_source(conn, &document, None).unwrap();
    let chunk = &document.chunks[0];
    let (section, section_revision) =
        storage::begin_section(conn, &source, &source_revision, chunk).unwrap();
    let (assertion, evidence) = storage::capture_assertion(
        conn,
        storage::AssertionCapture {
            document: &document,
            chunk,
            proposal: &proposal,
            source: &source,
            source_revision: &source_revision,
            section: &section,
            section_revision: &section_revision,
            model: "fixture",
        },
    )
    .unwrap();
    let id = storage::create_unit(conn, "p", &assertion, &proposal).unwrap();
    Record {
        id,
        source,
        assertion,
        evidence,
    }
}

fn connect(conn: &Connection, from: &Record, to: &Record, kind: &str) {
    storage::add_relation(
        conn,
        &from.id,
        &to.id,
        kind,
        &from.assertion,
        &from.evidence,
    )
    .unwrap();
}

fn ids(conn: &Connection, task: &str, paths: &[&str]) -> Vec<String> {
    retrieval::retrieve(
        conn,
        task,
        &paths.iter().map(|p| p.to_string()).collect::<Vec<_>>(),
    )
    .unwrap()
    .into_iter()
    .map(|hit| hit.knowledge.id)
    .collect()
}

#[test]
fn finds_connected_constraint_with_no_task_words_and_no_model_calls() {
    let conn = database();
    let anchor = record(
        &conn,
        "alpha",
        "transport.md",
        "transport",
        "delivery worker",
        "design",
        "The messenger supports redelivery of failed envelopes.",
    );
    let constraint = record(
        &conn,
        "alpha",
        "identity.md",
        "identity",
        "event identity",
        "constraint",
        "An event key must remain unchanged throughout its lifecycle.",
    );
    let unrelated = record(
        &conn,
        "alpha",
        "storage.md",
        "storage",
        "archival",
        "constraint",
        "The cold store must be sealed before departure.",
    );
    connect(&conn, &anchor, &constraint, "related_to");
    storage::refresh_knowledge(&conn, "p").unwrap();

    let results = retrieval::retrieve(&conn, "Implement redelivery", &[]).unwrap();
    assert!(results.iter().any(|hit| hit.knowledge.id == anchor.id));
    let retrieved = results
        .iter()
        .find(|hit| hit.knowledge.id == constraint.id)
        .unwrap();
    assert!(
        retrieved
            .reasons
            .iter()
            .any(|reason| reason.contains("related_to relationship"))
    );
    assert!(!results.iter().any(|hit| hit.knowledge.id == unrelated.id));
    assert_eq!(
        conn.query_row::<i64, _, _>("SELECT COUNT(*) FROM model_calls", [], |row| row.get(0))
            .unwrap(),
        0
    );
}

#[test]
fn subject_and_small_topic_clusters_bridge_unmentioned_requirements() {
    let conn = database();
    record(
        &conn,
        "alpha",
        "alarms.md",
        "handling",
        "cold-chain",
        "procedure",
        "A thawing alarm pages a handler.",
    );
    let same_subject = record(
        &conn,
        "alpha",
        "limits.md",
        "handling",
        "cold chain",
        "constraint",
        "Payload temperature must stay below six degrees.",
    );
    let same_topic = record(
        &conn,
        "alpha",
        "customs.md",
        "handling",
        "border declaration",
        "constraint",
        "Sealed containers require a customs declaration.",
    );
    storage::refresh_knowledge(&conn, "p").unwrap();
    let results = retrieval::retrieve(&conn, "Handle thawing", &[]).unwrap();
    assert!(
        results
            .iter()
            .find(|hit| hit.knowledge.id == same_subject.id)
            .unwrap()
            .reasons
            .iter()
            .any(|reason| reason.starts_with("same subject"))
    );
    assert!(
        results
            .iter()
            .find(|hit| hit.knowledge.id == same_topic.id)
            .unwrap()
            .reasons
            .iter()
            .any(|reason| reason.starts_with("same topic"))
    );
}

#[test]
fn broad_topic_does_not_become_an_unrelated_constraint_avalanche() {
    let conn = database();
    let anchor = record(
        &conn,
        "alpha",
        "reclamation.md",
        "operations",
        "reclamation",
        "design",
        "Reclamation uses a dedicated worker.",
    );
    for number in 0..80 {
        record(
            &conn,
            "alpha",
            &format!("aux-{number}.md"),
            "operations",
            &format!("separate subject {number}"),
            "constraint",
            &format!("Boundary number {number} must remain isolated."),
        );
    }
    storage::refresh_knowledge(&conn, "p").unwrap();
    assert_eq!(ids(&conn, "Optimize reclamation", &[]), vec![anchor.id]);
}

#[test]
fn exact_paths_preserve_ambiguous_roots_and_allow_explicit_disambiguation() {
    let conn = database();
    let alpha = record(
        &conn,
        "alpha",
        "policy.md",
        "controls",
        "event identity",
        "constraint",
        "An event key must remain stable.",
    );
    let beta = record(
        &conn,
        "beta",
        "policy.md",
        "controls",
        "event identity",
        "constraint",
        "A record label must remain distinct.",
    );
    storage::refresh_knowledge(&conn, "p").unwrap();
    let ambiguous = ids(&conn, "Make a change", &["policy.md"]);
    assert!(ambiguous.contains(&alpha.id));
    assert!(ambiguous.contains(&beta.id));
    assert_eq!(
        ids(&conn, "Make a change", &["alpha:policy.md"]),
        vec![alpha.id]
    );
    assert_eq!(
        ids(&conn, "Make a change", &["beta:./policy.md"]),
        vec![beta.id]
    );
}

#[test]
fn path_evidence_and_path_terminology_can_seed_retrieval() {
    let conn = database();
    let cited = record(
        &conn,
        "alpha",
        "policy.md",
        "controls",
        "instrument identity",
        "constraint",
        "Preserve instrument identifiers in `src/labs/cycle.rs`.",
    );
    let lexical = record(
        &conn,
        "alpha",
        "dispatch.md",
        "dispatch",
        "dispatch worker",
        "design",
        "The dispatch worker batches envelopes.",
    );
    storage::refresh_knowledge(&conn, "p").unwrap();
    let result =
        retrieval::retrieve(&conn, "Make a change", &["src/labs/cycle.rs".into()]).unwrap();
    assert!(
        result
            .iter()
            .find(|hit| hit.knowledge.id == cited.id)
            .unwrap()
            .reasons
            .iter()
            .any(|reason| reason.starts_with("path cited in source evidence"))
    );
    assert!(ids(&conn, "Make a change", &["src/dispatch/worker.rs"]).contains(&lexical.id));
    assert!(ids(&conn, "Make a change", &["dispatch/implementation.rs"]).contains(&lexical.id));
}

#[test]
fn incoming_relationships_preserve_superseded_historical_knowledge() {
    let conn = database();
    let old = record(
        &conn,
        "alpha",
        "old.md",
        "retention",
        "archival schedule",
        "decision",
        "Retention previously used an annual schedule.",
    );
    let new = record(
        &conn,
        "alpha",
        "new.md",
        "refresh",
        "replacement schedule",
        "decision",
        "Quarterly expiry replaces the previous schedule.",
    );
    connect(&conn, &new, &old, "supersedes");
    storage::retire_source(&conn, &old.source).unwrap();
    storage::refresh_knowledge(&conn, "p").unwrap();
    let results = retrieval::retrieve(&conn, "Review retention", &[]).unwrap();
    assert!(results.iter().any(|hit| hit.knowledge.id == new.id));
    let historical = results
        .iter()
        .find(|hit| hit.knowledge.id == old.id)
        .unwrap();
    assert_eq!(historical.knowledge.lifecycle, "superseded");
    assert_eq!(historical.knowledge.support_state, "historical_only");
    assert!(
        historical
            .knowledge
            .evidence
            .iter()
            .all(|evidence| !evidence.active)
    );
}

#[test]
fn graph_expansion_is_bounded_and_repeated_reads_are_identical() {
    let conn = database();
    let records: Vec<_> = (0..5)
        .map(|number| {
            record(
                &conn,
                "alpha",
                &format!("node-{number}.md"),
                &format!("topic-{number}"),
                &format!("subject-{number}"),
                "design",
                if number == 0 {
                    "The zephyr component owns initialization."
                } else {
                    "A secondary component owns a separate mechanism."
                },
            )
        })
        .collect();
    for pair in records.windows(2) {
        connect(&conn, &pair[0], &pair[1], "related_to");
    }
    storage::refresh_knowledge(&conn, "p").unwrap();
    let first = retrieval::retrieve(&conn, "Change zephyr", &[]).unwrap();
    let second = retrieval::retrieve(&conn, "Change zephyr", &[]).unwrap();
    assert_eq!(first.len(), 3);
    assert!(first.iter().any(|hit| hit.knowledge.id == records[2].id));
    assert!(!first.iter().any(|hit| hit.knowledge.id == records[3].id));
    assert!(
        retrieval::retrieve_with_report(&conn, "Change zephyr", &[])
            .unwrap()
            .truncated
    );
    for (left, right) in first.iter().zip(second) {
        assert_eq!(left.knowledge.id, right.knowledge.id);
        assert_eq!(left.score, right.score);
        assert_eq!(left.reasons, right.reasons);
    }
}

#[test]
fn search_reports_term_seed_and_concept_limits_without_claiming_completeness() {
    let conn = database();
    record(
        &conn,
        "alpha",
        "anchor.md",
        "boundary",
        "envelope identity",
        "design",
        "The zephyr mechanism owns initialization.",
    );
    for number in 0..45 {
        record(
            &conn,
            "alpha",
            &format!("constraint-{number}.md"),
            "boundary",
            "envelope identity",
            "constraint",
            &format!("Number {number} must retain an invariant."),
        );
    }
    storage::refresh_knowledge(&conn, "p").unwrap();
    let report = retrieval::retrieve_with_report(&conn, "Change zephyr", &[]).unwrap();
    assert!(report.truncated);
    assert!(report.hits.len() < 46);
    let long_query = (0..40)
        .map(|number| format!("term{number}"))
        .collect::<Vec<_>>()
        .join(" ");
    assert!(
        retrieval::retrieve_with_report(&conn, &long_query, &[])
            .unwrap()
            .truncated
    );
    assert!(
        !retrieval::retrieve_with_report(&conn, &"absent ".repeat(40), &[])
            .unwrap()
            .truncated
    );

    let conn = database();
    for number in 0..55 {
        record(
            &conn,
            "alpha",
            &format!("record-{number}.md"),
            &format!("topic-{number}"),
            &format!("subject-{number}"),
            "design",
            "The zephyr mechanism owns initialization.",
        );
    }
    storage::refresh_knowledge(&conn, "p").unwrap();
    let report = retrieval::retrieve_with_report(&conn, "Change zephyr", &[]).unwrap();
    assert!(report.truncated);
    assert_eq!(report.hits.len(), 48);
}

#[test]
fn weak_topic_constraints_do_not_displace_direct_task_relevance_under_a_budget() {
    let conn = database();
    let direct = record(
        &conn,
        "alpha",
        "engine.md",
        "operations",
        "retry worker",
        "design",
        "The zephyr component processes deferred work using a dedicated worker.",
    );
    storage::refresh_knowledge(&conn, "p").unwrap();
    let mut options = ContextOptions {
        task: "Improve zephyr".into(),
        paths: vec![],
        max_tokens: 10_000,
    };
    let baseline = context::build_context(&conn, &options).unwrap();
    assert_eq!(baseline.sections.items().count(), 1);
    options.max_tokens = baseline.budget.used_tokens + 200;

    record(
        &conn,
        "alpha",
        "garden.md",
        "operations",
        "garden maintenance",
        "constraint",
        "Ornamental shrubs must be watered before sunrise during extended warm periods.",
    );
    storage::refresh_knowledge(&conn, "p").unwrap();
    let result = context::build_context(&conn, &options).unwrap();
    assert_eq!(result.sections.items().count(), 1);
    assert!(
        result
            .sections
            .current_designs
            .iter()
            .any(|item| item.id == direct.id)
    );
    assert_eq!(result.omissions.knowledge_units, 1);
}

#[test]
fn task_punctuation_and_fts_operator_names_are_literal_and_empty_queries_are_empty() {
    let conn = database();
    let anchor = record(
        &conn,
        "alpha",
        "notes.md",
        "notes",
        "delivery",
        "design",
        "Redelivery uses a dedicated worker.",
    );
    storage::refresh_knowledge(&conn, "p").unwrap();
    assert!(ids(&conn, "\"redelivery\" OR NOT (NEAR(foo bar))*", &[]).contains(&anchor.id));
    assert!(ids(&conn, " the and to ", &[]).is_empty());
    assert!(ids(&conn, "???", &[]).is_empty());
}
