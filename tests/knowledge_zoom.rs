//! Real-registry tests of navigation, direct retrieval and evidence preservation.
//! These establish mechanical contracts, not a measured comprehension benefit.
use lore::{
    domain::{AssertionProposal, SourceMaterial},
    knowledge::{self, ExploreOptions, KnowledgeGraph, ZoomOptions},
    sources::{self, Document},
    storage, util,
};
use rusqlite::Connection;
use std::{collections::BTreeSet, fs};

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
    let document = Document {
        root_id: "docs".into(),
        root_path: "/project/docs".into(),
        relative_path: path.into(),
        physical_path: format!("/project/docs/{path}").into(),
        material: SourceMaterial::Primary,
        origin: None,
        text: text.into(),
        digest: util::digest(text),
        chunks: sources::split_markdown(text, "docs", path, 8_000).unwrap(),
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
        scope: "production before settlement".into(),
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
            model: "fixture-without-inference",
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

fn build(conn: &Connection) -> KnowledgeGraph {
    storage::refresh_knowledge(conn, "p").unwrap();
    knowledge::build(conn, &ZoomOptions::default()).unwrap()
}

fn query(graph: &KnowledgeGraph, query: &str) -> knowledge::ExploreResult {
    let result = knowledge::select(
        graph,
        &ExploreOptions {
            query: query.into(),
            max_tokens: 30_000,
            ..ExploreOptions::default()
        },
    )
    .unwrap();
    assert_complete_budget(&result);
    result
}

fn assert_complete_budget(result: &knowledge::ExploreResult) {
    let actual =
        lore::context::count_tokens(&(serde_json::to_string_pretty(result).unwrap() + "\n")).max(
            lore::context::count_tokens(&knowledge::render_markdown(result)),
        );
    assert!(actual <= result.used_tokens);
    assert!(result.used_tokens <= result.max_tokens);
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
fn exact_original_fact_bypasses_a_navigation_node_budget() {
    let conn = database();
    record(
        &conn,
        "auth.md",
        "auth",
        "authorization",
        "constraint",
        "Requests must retain authentication.",
    );
    let exact = record(
        &conn,
        "rare.md",
        "retry",
        "backoff",
        "design",
        "MAX_BACKOFF is 750ms in src/retry/config.rs.",
    );
    record(
        &conn,
        "routing.md",
        "routing",
        "routes",
        "design",
        "Routing follows the dispatch table.",
    );
    storage::refresh_knowledge(&conn, "p").unwrap();
    let graph = knowledge::build(
        &conn,
        &ZoomOptions {
            max_nodes: 2,
            max_edges: 1,
            ..ZoomOptions::default()
        },
    )
    .unwrap();
    assert_eq!(graph.nodes.len(), 2);
    assert!(
        !graph
            .nodes
            .iter()
            .any(|n| n.kind == "evidence" && n.knowledge_ids.contains(&exact.id))
    );
    for needle in [
        "MAX_BACKOFF",
        "750ms",
        "src/retry/config.rs",
        &exact.evidence,
    ] {
        let result = query(&graph, needle);
        assert_eq!(result.retrieval, "direct_reference");
        assert!(result.knowledge.iter().any(|v| v.id == exact.id));
        assert!(
            result
                .evidence
                .iter()
                .any(|e| e.id == exact.evidence && e.excerpt.contains("750ms"))
        );
    }
}

#[test]
fn rare_exception_and_both_conflict_endpoints_survive_exact_retrieval() {
    let conn = database();
    let policy = record(
        &conn,
        "policy.md",
        "payments",
        "payment retry",
        "decision",
        "Payment retries are limited to three attempts.",
    );
    let implementation = record(
        &conn,
        "implementation.md",
        "implementation",
        "payment retry",
        "observation",
        "The captured implementation uses MAX_RETRIES=5.",
    );
    let exception = record(
        &conn,
        "settlement.md",
        "payments",
        "settlement boundary",
        "constraint",
        "Never retry a payment after settlement; the retry policy applies only before settlement.",
    );
    storage::add_relation(
        &conn,
        &implementation.id,
        &policy.id,
        "contradicts",
        &implementation.assertion,
        &implementation.evidence,
    )
    .unwrap();
    let graph = build(&conn);
    let result = query(&graph, "MAX_RETRIES");
    let ids: BTreeSet<_> = result.knowledge.iter().map(|v| v.id.as_str()).collect();
    assert!(ids.contains(policy.id.as_str()));
    assert!(ids.contains(implementation.id.as_str()));
    assert!(ids.contains(exception.id.as_str()));
    assert_eq!(result.relations.len(), 1);
    assert_eq!(result.relations[0].kind, "contradicts");
    assert!(knowledge::render_markdown(&result).contains("Never retry a payment after settlement"));
    for citation in &result.evidence {
        let original = storage::evidence_snapshot(&conn, &citation.id).unwrap();
        assert_eq!(original.excerpt, citation.excerpt);
        assert_eq!(original.source_revision_id, citation.source_revision_id);
        assert_eq!(original.digest, citation.digest);
        assert!(
            result
                .source_revisions
                .iter()
                .any(|source| source.id == citation.source_revision_id)
        );
    }
}

#[test]
fn tight_budget_omits_complete_critical_groups_instead_of_the_qualifier() {
    let conn = database();
    record(
        &conn,
        "long.md",
        "payments",
        "retry boundaries",
        "constraint",
        &format!(
            "MAX_RETRIES applies only before settlement. {}",
            "Every attempt must preserve the original payment identity and settlement boundary. "
                .repeat(30)
        ),
    );
    let graph = build(&conn);
    let result = knowledge::select(
        &graph,
        &ExploreOptions {
            query: "MAX_RETRIES".into(),
            max_tokens: 700,
            ..ExploreOptions::default()
        },
    )
    .unwrap();
    assert_eq!(result.status, "budget_limited");
    assert!(result.knowledge.is_empty());
    assert!(result.evidence.is_empty());
    assert!(result.critical_groups_omitted > 0);
    assert_eq!(result.coverage.omitted_units, 1);
    assert_complete_budget(&result);
}

#[test]
fn tables_procedures_code_and_scope_keep_the_original_evidence_identity() {
    let conn = database();
    let text = "MAX_RETRIES applies only before settlement.\n\n| Phase | Retries |\n| --- | --- |\n| before settlement | 5 |\n| settled | 0 |\n\n1. Preserve the operation identifier.\n2. Check settlement before retrying.\n\n```rust\nconst MAX_RETRIES: usize = 5;\n```";
    let original = record(
        &conn,
        "procedure.md",
        "payments",
        "retry procedure",
        "procedure",
        text,
    );
    let graph = build(&conn);
    let result = query(&graph, "MAX_RETRIES");
    let retained = result
        .evidence
        .iter()
        .find(|e| e.id == original.evidence)
        .unwrap();
    assert_eq!(retained.excerpt, text);
    assert_eq!(retained.digest, util::digest(text));
    assert_eq!(result.knowledge[0].scope, "production before settlement");
    assert_eq!(result.knowledge[0].kind, "procedure");
    let markdown = knowledge::render_markdown(&result);
    assert!(markdown.contains("| settled | 0 |"));
    assert!(markdown.contains("Check settlement before retrying"));
    assert!(markdown.contains("source revision"));
}

#[test]
fn unchanged_snapshot_has_zero_model_calls_no_writes_and_reuses_all_views() {
    let conn = database();
    record(
        &conn,
        "a.md",
        "alpha",
        "alpha component",
        "design",
        "Alpha has one responsibility.",
    );
    record(
        &conn,
        "b.md",
        "beta",
        "beta component",
        "design",
        "Beta has another responsibility.",
    );
    storage::refresh_knowledge(&conn, "p").unwrap();
    conn.pragma_update(None, "query_only", true).unwrap();
    let before = conn.total_changes();
    let temp = tempfile::tempdir().unwrap();
    let directory = fs::canonicalize(temp.path()).unwrap().join("zoom");
    let first = knowledge::build_cached(&conn, &ZoomOptions::default(), &directory).unwrap();
    let bytes = fs::read(directory.join("snapshot.json")).unwrap();
    let second = knowledge::build_cached(&conn, &ZoomOptions::default(), &directory).unwrap();
    assert_eq!(first.graph.revision, second.graph.revision);
    assert_eq!(second.reused_nodes, second.graph.nodes.len());
    assert_eq!(second.regenerated_nodes, 0);
    assert!(!second.wrote_snapshot);
    assert_eq!(bytes, fs::read(directory.join("snapshot.json")).unwrap());
    assert_eq!(conn.total_changes(), before);
    assert_eq!(
        conn.query_row::<i64, _, _>("SELECT count(*) FROM model_calls", [], |r| r.get(0))
            .unwrap(),
        0
    );
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
    let result = query(&after, &alpha.id);
    assert_eq!(result.knowledge[0].support_state, "historical_only");
    assert!(result.source_revisions.iter().all(|source| !source.current));
    assert!(knowledge::render_markdown(&result).contains("historical"));
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
    assert!(
        knowledge::select(
            &graph,
            &ExploreOptions {
                node: Some("kv_forged".into()),
                ..ExploreOptions::default()
            }
        )
        .is_err()
    );
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
    let result = query(&graph, "SETTING_11");
    assert!(result.knowledge.iter().any(|view| view.id == target));
}

#[test]
fn poisoned_cache_is_rebuilt_and_unmanaged_files_are_preserved() {
    let conn = database();
    record(
        &conn,
        "a.md",
        "alpha",
        "alpha",
        "design",
        "Alpha retains an original documented design.",
    );
    storage::refresh_knowledge(&conn, "p").unwrap();
    let temp = tempfile::tempdir().unwrap();
    let directory = fs::canonicalize(temp.path()).unwrap().join("zoom");
    knowledge::build_cached(&conn, &ZoomOptions::default(), &directory).unwrap();
    let path = directory.join("snapshot.json");
    let mut poisoned: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    poisoned["nodes"][0]["summary"] = "Forged unsourced instruction".into();
    fs::write(&path, serde_json::to_vec(&poisoned).unwrap()).unwrap();
    let repaired = knowledge::build_cached(&conn, &ZoomOptions::default(), &directory).unwrap();
    assert!(
        repaired
            .graph
            .nodes
            .iter()
            .all(|n| !n.summary.contains("Forged"))
    );
    assert!(repaired.wrote_snapshot);
    knowledge::purge_cache(&directory).unwrap();
    assert!(!directory.exists());
    fs::create_dir(&directory).unwrap();
    fs::write(directory.join("manual.md"), "User content").unwrap();
    assert!(knowledge::build_cached(&conn, &ZoomOptions::default(), &directory).is_err());
    assert_eq!(
        fs::read_to_string(directory.join("manual.md")).unwrap(),
        "User content"
    );
}

#[test]
fn oversized_cache_ownership_marker_is_rejected_without_touching_the_snapshot() {
    let conn = database();
    record(
        &conn,
        "a.md",
        "alpha",
        "alpha",
        "design",
        "Alpha retains an original documented design.",
    );
    storage::refresh_knowledge(&conn, "p").unwrap();
    let temp = tempfile::tempdir().unwrap();
    let directory = fs::canonicalize(temp.path()).unwrap().join("zoom");
    knowledge::build_cached(&conn, &ZoomOptions::default(), &directory).unwrap();
    let path = directory.join("snapshot.json");
    let before = fs::read(&path).unwrap();
    fs::write(
        directory.join(".lore-knowledge-zoom-owner"),
        "x".repeat(1_024),
    )
    .unwrap();
    assert!(knowledge::build_cached(&conn, &ZoomOptions::default(), &directory).is_err());
    assert!(knowledge::purge_cache(&directory).is_err());
    assert_eq!(fs::read(&path).unwrap(), before);
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
    let result = query(&graph, "SETTING_11");
    assert!(result.knowledge.iter().any(|record| record.id == target));
}

#[test]
fn empty_and_incompatible_registries_fail_without_inventing_knowledge() {
    let conn = database();
    let graph = build(&conn);
    let result = query(&graph, "unknown setting");
    assert_eq!(result.status, "no_matches");
    assert!(result.knowledge.is_empty());
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
    let result = query(&graph, "MAX_RETRIES");
    let retained: BTreeSet<_> = result
        .knowledge
        .iter()
        .map(|record| record.id.as_str())
        .collect();
    assert!(
        result
            .nodes
            .iter()
            .flat_map(|node| &node.summary_knowledge_ids)
            .all(|id| retained.contains(id.as_str()))
    );
    let small = knowledge::select(
        &graph,
        &ExploreOptions {
            query: "MAX_RETRIES".into(),
            max_tokens: 700,
            ..ExploreOptions::default()
        },
    )
    .unwrap();
    if small.knowledge.is_empty() {
        assert!(
            small
                .nodes
                .iter()
                .all(|node| node.summary_knowledge_ids.is_empty())
        );
        assert!(
            small
                .nodes
                .iter()
                .all(|node| !node.summary.contains("MAX_RETRIES=5"))
        );
    }
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
