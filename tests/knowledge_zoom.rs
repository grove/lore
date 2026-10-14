//! Real-registry tests of the retained containment graph and evidence preservation.
//! These establish mechanical contracts, not a measured comprehension benefit.
use lore::{
    domain::{AssertionProposal, SourceMaterial},
    knowledge::{self, KnowledgeGraph, ZoomOptions},
    sources::{self, Document},
    storage, util,
};
use rusqlite::Connection;

struct Record {
    id: String,
    source: String,
    assertion: String,
    evidence: String,
}

fn database() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    storage::migrate(&conn).unwrap();
    conn.execute(
        "INSERT INTO projects VALUES('p','Zoom fixture','2026-10-10')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO source_roots VALUES('docs','p','/project/docs')",
        [],
    )
    .unwrap();
    conn
}

fn record(
    conn: &Connection,
    path: &str,
    topic: &str,
    subject: &str,
    kind: &str,
    text: &str,
) -> Record {
    let (proposal, source, assertion, evidence) = capture(conn, path, topic, subject, kind, text);
    let id = storage::create_unit(conn, "p", &assertion, &proposal).unwrap();
    Record {
        id,
        source,
        assertion,
        evidence,
    }
}

fn capture(
    conn: &Connection,
    path: &str,
    topic: &str,
    subject: &str,
    kind: &str,
    text: &str,
) -> (AssertionProposal, String, String, String) {
    capture_with_excerpt(conn, path, topic, subject, kind, text, text)
}

fn capture_with_excerpt(
    conn: &Connection,
    path: &str,
    topic: &str,
    subject: &str,
    kind: &str,
    text: &str,
    excerpt: &str,
) -> (AssertionProposal, String, String, String) {
    let document = Document {
        root_id: "docs".into(),
        root_path: "/project/docs".into(),
        relative_path: path.into(),
        physical_path: format!("/project/docs/{path}").into(),
        material: SourceMaterial::Primary,
        origin: None,
        text: excerpt.into(),
        digest: util::digest(excerpt),
        chunks: sources::split_markdown(excerpt, "docs", path, 8_000).unwrap(),
    };
    let proposal = AssertionProposal {
        topic: topic.into(),
        topic_title: topic.into(),
        subject: subject.into(),
        statement: text.into(),
        kind: kind.into(),
        lifecycle: if kind == "decision" {
            "accepted"
        } else if kind == "proposal" {
            "proposed"
        } else {
            "active"
        }
        .into(),
        scope: "production before settlement".into(),
        effective_at: String::new(),
        quote: excerpt.into(),
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
            model: "fixture-without-inference",
        },
    )
    .unwrap();
    (proposal, source, assertion, evidence)
}

fn build(conn: &Connection) -> KnowledgeGraph {
    storage::refresh_knowledge(conn, "p").unwrap();
    knowledge::build(conn, &ZoomOptions::default()).unwrap()
}

fn depth(graph: &KnowledgeGraph, id: &str) -> usize {
    1 + graph
        .edges
        .iter()
        .filter(|edge| edge.parent == id)
        .map(|edge| depth(graph, &edge.child))
        .max()
        .unwrap_or(0)
}

#[test]
fn semantic_membership_can_exceed_five_levels_without_a_depth_parameter() {
    let conn = database();
    let concepts = [
        "commerce",
        "payments",
        "authorization",
        "retry",
        "backoff",
        "jitter",
        "circuit",
        "outage",
        "recovery",
    ];
    for index in 0..concepts.len() {
        record(
            &conn,
            &format!("concept-{index}.md"),
            "architecture",
            &concepts[..=index].join(" "),
            "design",
            &format!("The component has responsibility number {index}."),
        );
    }
    let graph = build(&conn);
    assert!(depth(&graph, &graph.root_id) > 5);
    assert!(graph.nodes.iter().any(|node| node.kind == "concept"));
    knowledge::validate(&graph).unwrap();
}

#[test]
fn cross_cutting_concepts_and_records_have_multiple_semantic_parents() {
    let conn = database();
    record(
        &conn,
        "a.md",
        "payments",
        "shared authentication",
        "design",
        "Shared authentication applies to payments.",
    );
    record(
        &conn,
        "b.md",
        "payments",
        "payment routing",
        "design",
        "Payment routing has its own responsibility.",
    );
    record(
        &conn,
        "c.md",
        "delivery",
        "shared authentication",
        "design",
        "Shared authentication applies to delivery.",
    );
    record(
        &conn,
        "d.md",
        "delivery",
        "delivery routing",
        "design",
        "Delivery routing has its own responsibility.",
    );
    let graph = build(&conn);
    assert!(
        graph
            .nodes
            .iter()
            .any(|node| node.parent_view_ids.len() >= 2)
    );
    assert_eq!(graph.model_calls, 0);
    knowledge::validate(&graph).unwrap();
}

#[test]
fn source_retirement_invalidates_affected_views_and_keeps_unrelated_revisions() {
    let conn = database();
    let alpha = record(
        &conn,
        "alpha.md",
        "alpha",
        "alpha component",
        "design",
        "Alpha retains the documented design.",
    );
    let beta = record(
        &conn,
        "beta.md",
        "beta",
        "beta component",
        "design",
        "Beta retains another design.",
    );
    let before = build(&conn);
    storage::retire_source(&conn, &alpha.source).unwrap();
    let after = build(&conn);
    let leaf = |graph: &KnowledgeGraph, id: &str| {
        graph
            .nodes
            .iter()
            .find(|n| n.kind == "evidence" && n.knowledge_ids == [id])
            .unwrap()
            .revision
            .clone()
    };
    assert_ne!(before.revision, after.revision);
    assert_ne!(leaf(&before, &alpha.id), leaf(&after, &alpha.id));
    assert_eq!(leaf(&before, &beta.id), leaf(&after, &beta.id));
    assert_eq!(
        after
            .knowledge
            .iter()
            .find(|record| record.id == alpha.id)
            .unwrap()
            .support_state,
        "historical_only"
    );
}

#[test]
fn relationship_changes_invalidate_both_topic_views() {
    let conn = database();
    let alpha = record(
        &conn,
        "alpha.md",
        "alpha",
        "alpha interface",
        "decision",
        "Alpha selects the documented interface.",
    );
    let beta = record(
        &conn,
        "beta.md",
        "beta",
        "beta interface",
        "decision",
        "Beta selects a different interface.",
    );
    record(
        &conn,
        "gamma.md",
        "gamma",
        "gamma topic",
        "design",
        "Gamma is independent.",
    );
    let before = build(&conn);
    storage::add_relation(
        &conn,
        &alpha.id,
        &beta.id,
        "contradicts",
        &alpha.assertion,
        &alpha.evidence,
    )
    .unwrap();
    let after = build(&conn);
    for id in [&alpha.id, &beta.id] {
        let old = before
            .nodes
            .iter()
            .find(|n| n.kind == "evidence" && n.knowledge_ids.contains(id))
            .unwrap();
        let new = after.nodes.iter().find(|n| n.id == old.id).unwrap();
        assert_ne!(old.revision, new.revision);
    }
}

#[test]
fn invalid_refs_and_containment_cycles_are_rejected() {
    let conn = database();
    record(
        &conn,
        "a.md",
        "alpha",
        "alpha",
        "design",
        "Alpha has a documented boundary.",
    );
    let graph = build(&conn);
    let mut bad = graph.clone();
    let leaf = bad
        .nodes
        .iter()
        .find(|node| node.kind == "evidence")
        .unwrap()
        .id
        .clone();
    bad.edges.push(knowledge::ViewEdge {
        parent: leaf,
        child: bad.root_id.clone(),
        kind: "contains".into(),
    });
    assert!(knowledge::validate(&bad).is_err());
    let mut bad = graph;
    bad.knowledge[0].evidence[0].excerpt = "A forged source statement".into();
    assert!(knowledge::validate(&bad).is_err());
}

#[test]
fn tiny_grouping_work_budget_falls_back_to_a_valid_direct_index() {
    let conn = database();
    let mut target = String::new();
    for index in 0..12 {
        target = record(
            &conn,
            &format!("value-{index}.md"),
            "configuration",
            &format!("configuration value {index}"),
            "design",
            &format!("SETTING_{index} has a recorded value."),
        )
        .id;
    }
    storage::refresh_knowledge(&conn, "p").unwrap();
    let graph = knowledge::build(
        &conn,
        &ZoomOptions {
            max_work: 3,
            ..ZoomOptions::default()
        },
    )
    .unwrap();
    assert!(graph.report.truncated);
    assert!(graph.report.work_used <= 3);
    knowledge::validate(&graph).unwrap();
    assert!(graph.knowledge.iter().any(|record| record.id == target));
}

#[test]
fn large_corpus_keeps_semantic_navigation_and_direct_facts_within_a_small_node_budget() {
    let conn = database();
    let mut target = String::new();
    for index in 0..12 {
        target = record(
            &conn,
            &format!("record-{index}.md"),
            if index % 2 == 0 {
                "payments"
            } else {
                "routing"
            },
            &format!("component-{index}"),
            "design",
            &format!("SETTING_{index} has documented value {index}."),
        )
        .id;
    }
    storage::refresh_knowledge(&conn, "p").unwrap();
    let graph = knowledge::build(
        &conn,
        &ZoomOptions {
            max_nodes: 9,
            max_edges: 12,
            ..ZoomOptions::default()
        },
    )
    .unwrap();
    assert!(graph.nodes.len() <= 9);
    assert!(graph.nodes.iter().any(|node| node.kind == "concept"));
    assert!(graph.report.omitted_navigation_units > 0);
    assert_eq!(graph.knowledge.len(), 12);
    assert!(graph.knowledge.iter().any(|record| record.id == target));
}

#[test]
fn empty_and_incompatible_registries_fail_without_inventing_knowledge() {
    let conn = database();
    let graph = build(&conn);
    assert!(graph.knowledge.is_empty());
    let empty = Connection::open_in_memory().unwrap();
    assert!(knowledge::build(&empty, &ZoomOptions::default()).is_err());
}

#[test]
fn concept_summaries_quote_original_records_and_keep_complete_critical_scope() {
    let conn = database();
    let policy = record(
        &conn,
        "policy.md",
        "payments",
        "payment retry",
        "decision",
        "MAX_RETRIES=5 applies before settlement.",
    );
    let exception = record(
        &conn,
        "exception.md",
        "payments",
        "payment settlement",
        "constraint",
        "Never retry a payment after settlement.",
    );
    record(
        &conn,
        "routing.md",
        "routing",
        "routing",
        "design",
        "Routing has a separate documented responsibility.",
    );
    let graph = build(&conn);
    let concept = graph
        .nodes
        .iter()
        .find(|node| {
            node.kind == "concept"
                && node.knowledge_ids.contains(&policy.id)
                && node.knowledge_ids.contains(&exception.id)
        })
        .unwrap();
    assert!(
        concept
            .summary
            .contains("MAX_RETRIES=5 applies before settlement")
    );
    assert!(
        concept
            .summary
            .contains("Never retry a payment after settlement")
    );
    assert!(
        concept
            .critical_knowledge_ids
            .iter()
            .all(|id| concept.summary_knowledge_ids.contains(id))
    );
}

#[test]
fn excessive_critical_summary_is_disclosed_without_losing_original_records() {
    let conn = database();
    for index in 0..2 {
        record(&conn, &format!("boundary-{index}.md"), "payments", "retry boundaries", "constraint", &format!("Payment boundary {index} must be preserved. {}", "Every attempt must preserve operation identity, settlement status, and the specified exception. ".repeat(40)));
    }
    let graph = build(&conn);
    let root = graph
        .nodes
        .iter()
        .find(|node| node.id == graph.root_id)
        .unwrap();
    assert!(root.summary_knowledge_ids.is_empty());
    assert!(
        root.summary
            .contains("qualifications exceed the summary budget")
    );
    assert_eq!(root.critical_knowledge_ids.len(), 2);
    assert_eq!(graph.knowledge.len(), 2);
    assert_eq!(root.summary_coverage.omitted_units, 2);
}
