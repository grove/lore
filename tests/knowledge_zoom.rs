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
    assert!(!first.refresh.topology_reused);
    assert!(first.refresh.layout_generation_work > 0);
    assert_eq!(first.refresh.summaries_generated, first.graph.nodes.len());
    assert!(second.refresh.topology_reused);
    assert_eq!(second.refresh.layout_generation_work, 0);
    assert_eq!(second.refresh.summaries_generated, 0);
    assert_eq!(second.refresh.dependency_revisions_generated, 0);
    assert_eq!(second.refresh.summaries_validated, second.graph.nodes.len());
    assert_eq!(
        second.refresh.dependency_revisions_validated,
        second.graph.nodes.len()
    );
    assert_eq!(
        second.refresh.records_revalidated,
        second.graph.knowledge.len()
    );
    assert_eq!(
        second.refresh.evidence_revalidated,
        second.graph.evidence.len()
    );
    assert!(second.refresh.topology_validation_work > 0);
    assert_eq!(
        second.refresh.layout_generation_work + second.refresh.topology_validation_work,
        second.graph.report.work_used
    );
    assert!(second.refresh.fallback_reason.is_none());
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

fn assert_matches_fresh(graph: &KnowledgeGraph, conn: &Connection) {
    let fresh = knowledge::build(conn, &graph.options).unwrap();
    assert_eq!(
        serde_json::to_value(graph).unwrap(),
        serde_json::to_value(fresh).unwrap()
    );
}

#[test]
fn stable_topology_refreshes_only_nodes_with_changed_source_status() {
    let conn = database();
    let alpha = record(
        &conn,
        "alpha.md",
        "alpha",
        "alpha component",
        "design",
        "Alpha retains its documented design.",
    );
    let beta = record(
        &conn,
        "beta.md",
        "beta",
        "beta component",
        "design",
        "Beta retains its independent design.",
    );
    storage::refresh_knowledge(&conn, "p").unwrap();
    let temp = tempfile::tempdir().unwrap();
    let directory = fs::canonicalize(temp.path()).unwrap().join("zoom");
    let first = knowledge::build_cached(&conn, &ZoomOptions::default(), &directory).unwrap();
    storage::retire_source(&conn, &alpha.source).unwrap();
    storage::refresh_knowledge(&conn, "p").unwrap();
    let changed = knowledge::build_cached(&conn, &ZoomOptions::default(), &directory).unwrap();
    let affected = changed
        .graph
        .nodes
        .iter()
        .filter(|node| node.knowledge_ids.contains(&alpha.id))
        .count();
    assert!(changed.refresh.topology_reused);
    assert_eq!(changed.refresh.layout_generation_work, 0);
    assert_eq!(changed.refresh.summaries_generated, affected);
    assert_eq!(changed.refresh.dependency_revisions_generated, affected);
    assert!(affected > 0 && affected < changed.graph.nodes.len());
    assert_eq!(changed.reused_nodes, changed.graph.nodes.len() - affected);
    let beta_before = first
        .graph
        .nodes
        .iter()
        .find(|node| node.kind == "evidence" && node.knowledge_ids == [beta.id.clone()])
        .unwrap();
    assert_eq!(
        changed
            .graph
            .nodes
            .iter()
            .find(|node| node.id == beta_before.id)
            .unwrap(),
        beta_before
    );
    let result = query(&changed.graph, &alpha.id);
    assert_eq!(result.knowledge[0].support_state, "historical_only");
    assert!(result.source_revisions.iter().all(|source| !source.current));
    assert_matches_fresh(&changed.graph, &conn);
    let no_op = knowledge::build_cached(&conn, &ZoomOptions::default(), &directory).unwrap();
    assert_eq!(no_op.refresh.summaries_generated, 0);
    assert_eq!(no_op.refresh.layout_generation_work, 0);
    assert!(!no_op.wrote_snapshot);
}

#[test]
fn changed_source_support_refreshes_opposite_relationship_endpoints_in_a_validated_layout() {
    let conn = database();
    let alpha = record(
        &conn,
        "alpha.md",
        "alpha",
        "alpha interface",
        "decision",
        "Alpha selects one interface.",
    );
    let beta = record(
        &conn,
        "beta.md",
        "beta",
        "beta interface",
        "decision",
        "Beta selects another interface.",
    );
    let gamma = record(
        &conn,
        "gamma.md",
        "gamma",
        "gamma component",
        "design",
        "Gamma has an independent responsibility.",
    );
    storage::add_relation(
        &conn,
        &alpha.id,
        &beta.id,
        "contradicts",
        &alpha.assertion,
        &alpha.evidence,
    )
    .unwrap();
    storage::refresh_knowledge(&conn, "p").unwrap();
    let temp = tempfile::tempdir().unwrap();
    let directory = fs::canonicalize(temp.path()).unwrap().join("zoom");
    let first = knowledge::build_cached(&conn, &ZoomOptions::default(), &directory).unwrap();
    let (_, _, assertion, evidence) = capture(
        &conn,
        "alpha-scope.md",
        "alpha",
        "alpha interface",
        "decision",
        "Alpha selects one interface only if settlement is still pending; never repeat it after settlement.",
    );
    storage::assign(&conn, &assertion, &alpha.id).unwrap();
    storage::refresh_knowledge(&conn, "p").unwrap();
    let changed = knowledge::build_cached(&conn, &ZoomOptions::default(), &directory).unwrap();
    assert!(changed.refresh.topology_reused);
    assert_eq!(changed.refresh.layout_generation_work, 0);
    let expected = changed
        .graph
        .nodes
        .iter()
        .filter(|node| {
            node.knowledge_ids.contains(&alpha.id) || node.knowledge_ids.contains(&beta.id)
        })
        .count();
    assert_eq!(changed.refresh.summaries_generated, expected);
    assert_eq!(changed.refresh.dependency_revisions_generated, expected);
    assert!(expected < changed.graph.nodes.len());
    // Beta's original record did not change, but a referenced opposite endpoint
    // gained source evidence. Its view dependency must still advance.
    let beta_before = first
        .graph
        .knowledge
        .iter()
        .find(|record| record.id == beta.id)
        .unwrap();
    let beta_after = changed
        .graph
        .knowledge
        .iter()
        .find(|record| record.id == beta.id)
        .unwrap();
    assert_eq!(beta_before.revision_id, beta_after.revision_id);
    for record in [&alpha, &beta] {
        let old = first
            .graph
            .nodes
            .iter()
            .find(|node| node.kind == "evidence" && node.knowledge_ids.contains(&record.id))
            .unwrap();
        let new = changed
            .graph
            .nodes
            .iter()
            .find(|node| node.id == old.id)
            .unwrap();
        assert_ne!(old.revision, new.revision);
    }
    let gamma_before = first
        .graph
        .nodes
        .iter()
        .find(|node| node.kind == "evidence" && node.knowledge_ids.contains(&gamma.id))
        .unwrap();
    assert_eq!(
        changed
            .graph
            .nodes
            .iter()
            .find(|node| node.id == gamma_before.id)
            .unwrap(),
        gamma_before
    );
    let result = query(&changed.graph, &beta.id);
    assert!(result.evidence.iter().any(
        |item| item.id == evidence && item.excerpt.contains("never repeat it after settlement")
    ));
    assert_matches_fresh(&changed.graph, &conn);
}

#[test]
fn grouping_labels_and_resource_limits_invalidate_even_unchanged_memberships() {
    let conn = database();
    record(
        &conn,
        "alpha-a.md",
        "alpha",
        "first component",
        "design",
        "First component has one role.",
    );
    record(
        &conn,
        "alpha-b.md",
        "alpha",
        "second component",
        "design",
        "Second component has another role.",
    );
    record(
        &conn,
        "beta.md",
        "beta",
        "separate component",
        "design",
        "Separate component has another role.",
    );
    storage::refresh_knowledge(&conn, "p").unwrap();
    let temp = tempfile::tempdir().unwrap();
    let directory = fs::canonicalize(temp.path()).unwrap().join("zoom");
    let first = knowledge::build_cached(&conn, &ZoomOptions::default(), &directory).unwrap();
    let original = first
        .graph
        .nodes
        .iter()
        .find(|node| node.kind == "concept" && node.title == "alpha")
        .unwrap();
    conn.execute(
        "UPDATE topics SET title='Renamed alpha' WHERE slug='alpha'",
        [],
    )
    .unwrap();
    let renamed = knowledge::build_cached(&conn, &ZoomOptions::default(), &directory).unwrap();
    assert!(!renamed.refresh.topology_reused);
    assert!(renamed.refresh.layout_generation_work > 0);
    let updated = renamed
        .graph
        .nodes
        .iter()
        .find(|node| node.id == original.id)
        .unwrap();
    assert_eq!(updated.knowledge_ids, original.knowledge_ids);
    assert_eq!(updated.title, "Renamed alpha");
    assert_ne!(updated.grouping_basis, original.grouping_basis);
    assert_matches_fresh(&renamed.graph, &conn);
    let options = ZoomOptions {
        max_work: ZoomOptions::default().max_work - 1,
        ..ZoomOptions::default()
    };
    let limited = knowledge::build_cached(&conn, &options, &directory).unwrap();
    assert!(!limited.refresh.topology_reused);
    assert_eq!(
        limited.refresh.summaries_generated,
        limited.graph.nodes.len()
    );
    assert_matches_fresh(&limited.graph, &conn);
}

#[test]
fn a_new_critical_exception_changes_leaf_priority_and_forces_a_full_refresh() {
    let conn = database();
    for index in 0..3 {
        record(
            &conn,
            &format!("component-{index}.md"),
            "components",
            &format!("component {index}"),
            "design",
            &format!("Component {index} has a documented responsibility."),
        );
    }
    storage::refresh_knowledge(&conn, "p").unwrap();
    let temp = tempfile::tempdir().unwrap();
    let directory = fs::canonicalize(temp.path()).unwrap().join("zoom");
    let options = ZoomOptions {
        max_nodes: 2,
        max_edges: 1,
        ..ZoomOptions::default()
    };
    let first = knowledge::build_cached(&conn, &options, &directory).unwrap();
    let first_leaf = &first
        .graph
        .nodes
        .iter()
        .find(|node| node.kind == "evidence")
        .unwrap()
        .knowledge_ids[0];
    let target = first
        .graph
        .knowledge
        .iter()
        .find(|record| &record.id != first_leaf)
        .unwrap();
    let exception = "Never retry after settlement; the apparent retry permission applies only if the operation is still unsettled.";
    let (_, _, assertion, evidence) = capture(
        &conn,
        "rare-exception.md",
        &target.topic,
        &target.subject,
        "design",
        exception,
    );
    storage::assign(&conn, &assertion, &target.id).unwrap();
    storage::refresh_knowledge(&conn, "p").unwrap();
    let changed = knowledge::build_cached(&conn, &options, &directory).unwrap();
    assert!(!changed.refresh.topology_reused);
    assert_eq!(
        changed.refresh.summaries_generated,
        changed.graph.nodes.len()
    );
    assert!(
        changed
            .graph
            .nodes
            .iter()
            .any(|node| node.kind == "evidence" && node.knowledge_ids == [target.id.clone()])
    );
    let selected = query(&changed.graph, &evidence);
    assert!(
        selected
            .evidence
            .iter()
            .any(|item| item.id == evidence && item.excerpt == exception)
    );
    assert_matches_fresh(&changed.graph, &conn);
}

#[test]
fn a_new_relationship_rebuilds_grouping_but_inventory_only_changes_reuse_every_node() {
    let conn = database();
    // Source inventory is published only by initialized registries, as in an
    // actual completed project compilation.
    storage::set_meta(&conn, "initialized", "true").unwrap();
    let alpha = record(
        &conn,
        "alpha.md",
        "alpha",
        "alpha interface",
        "decision",
        "Alpha selects one interface.",
    );
    let beta = record(
        &conn,
        "beta.md",
        "beta",
        "beta interface",
        "decision",
        "Beta selects another interface.",
    );
    record(
        &conn,
        "gamma.md",
        "gamma",
        "gamma component",
        "design",
        "Gamma has a separate responsibility.",
    );
    storage::refresh_knowledge(&conn, "p").unwrap();
    let temp = tempfile::tempdir().unwrap();
    let directory = fs::canonicalize(temp.path()).unwrap().join("zoom");
    knowledge::build_cached(&conn, &ZoomOptions::default(), &directory).unwrap();
    storage::add_relation(
        &conn,
        &alpha.id,
        &beta.id,
        "contradicts",
        &alpha.assertion,
        &alpha.evidence,
    )
    .unwrap();
    storage::refresh_knowledge(&conn, "p").unwrap();
    let linked = knowledge::build_cached(&conn, &ZoomOptions::default(), &directory).unwrap();
    assert_eq!(storage::source_heads(&conn).unwrap().len(), 3);
    assert!(!linked.refresh.topology_reused);
    assert!(linked.refresh.layout_generation_work > 0);
    assert_matches_fresh(&linked.graph, &conn);
    // Captured material without an assigned knowledge unit advances the source
    // inventory, but it changes no eligible record or node dependency.
    capture(
        &conn,
        "unassigned.md",
        "notes",
        "unassigned source",
        "design",
        "This captured note has no assigned knowledge unit.",
    );
    assert_eq!(storage::source_heads(&conn).unwrap().len(), 4);
    let inventory = knowledge::build_cached(&conn, &ZoomOptions::default(), &directory).unwrap();
    assert_ne!(inventory.graph.revision, linked.graph.revision);
    assert!(inventory.refresh.topology_reused);
    assert_eq!(inventory.refresh.layout_generation_work, 0);
    assert_eq!(inventory.refresh.summaries_generated, 0);
    assert_eq!(inventory.refresh.dependency_revisions_generated, 0);
    assert_eq!(inventory.graph.nodes, linked.graph.nodes);
    assert!(inventory.wrote_snapshot);
    assert_matches_fresh(&inventory.graph, &conn);
}

fn resign_cache(value: &mut serde_json::Value) {
    let mut graph = value.clone();
    graph.as_object_mut().unwrap().remove("incremental");
    graph.as_object_mut().unwrap().remove("content_digest");
    value["content_digest"] = util::json_digest(&serde_json::json!({
        "graph":graph, "incremental":value["incremental"],
    }))
    .unwrap()
    .into();
}

#[test]
fn recomputing_the_cache_checksum_cannot_authorize_forged_summary_or_scope() {
    let conn = database();
    record(
        &conn,
        "alpha.md",
        "alpha",
        "alpha component",
        "constraint",
        "Alpha must preserve the full settlement exception.",
    );
    record(
        &conn,
        "beta.md",
        "beta",
        "beta component",
        "design",
        "Beta has an independent responsibility.",
    );
    storage::refresh_knowledge(&conn, "p").unwrap();
    let temp = tempfile::tempdir().unwrap();
    let directory = fs::canonicalize(temp.path()).unwrap().join("zoom");
    knowledge::build_cached(&conn, &ZoomOptions::default(), &directory).unwrap();
    let path = directory.join("snapshot.json");
    let original: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    for mutation in 0..3 {
        let mut changed = original.clone();
        let node = &mut changed["nodes"][0];
        match mutation {
            0 => {
                node["summary"] =
                    "Forged unsourced instruction; the exception does not apply.".into()
            }
            1 => node["title"] = "Forged authoritative title".into(),
            _ => node["critical_knowledge_ids"] = serde_json::json!(["ku_forged"]),
        }
        resign_cache(&mut changed);
        fs::write(&path, serde_json::to_vec(&changed).unwrap()).unwrap();
        let repaired = knowledge::build_cached(&conn, &ZoomOptions::default(), &directory).unwrap();
        assert!(!repaired.refresh.topology_reused);
        assert_eq!(
            repaired.refresh.summaries_generated,
            repaired.graph.nodes.len()
        );
        if mutation == 0 {
            assert!(
                repaired
                    .refresh
                    .fallback_reason
                    .as_deref()
                    .unwrap()
                    .contains("original-source plan")
            );
        }
        assert_matches_fresh(&repaired.graph, &conn);
    }
}

#[test]
fn a_resigned_cache_rejects_overflowing_summary_coverage_without_panicking() {
    let conn = database();
    record(
        &conn,
        "capacity.md",
        "dispatch",
        "dispatch",
        "constraint",
        "QUEUE_CAPACITY must remain at 128 jobs.",
    );
    storage::refresh_knowledge(&conn, "p").unwrap();
    let temp = tempfile::tempdir().unwrap();
    let directory = fs::canonicalize(temp.path()).unwrap().join("zoom");
    knowledge::build_cached(&conn, &ZoomOptions::default(), &directory).unwrap();
    let path = directory.join("snapshot.json");
    let mut changed: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    let node = changed["nodes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|node| node["summary_coverage"]["included_units"] == 1)
        .unwrap();
    assert_eq!(node["summary_coverage"]["eligible_units"], 1);
    node["summary_coverage"]["omitted_units"] = serde_json::json!(usize::MAX);
    resign_cache(&mut changed);
    fs::write(&path, serde_json::to_vec(&changed).unwrap()).unwrap();

    let repaired = knowledge::build_cached(&conn, &ZoomOptions::default(), &directory).unwrap();
    assert!(!repaired.refresh.topology_reused);
    assert!(
        repaired
            .refresh
            .fallback_reason
            .as_deref()
            .unwrap()
            .contains("summary coverage manifest mismatch")
    );
    assert_matches_fresh(&repaired.graph, &conn);
}

#[test]
fn an_incremental_cache_cannot_mask_corrupted_authoritative_evidence() {
    let conn = database();
    let original = record(
        &conn,
        "alpha.md",
        "alpha",
        "alpha component",
        "design",
        "Alpha has a retained original source.",
    );
    storage::refresh_knowledge(&conn, "p").unwrap();
    let temp = tempfile::tempdir().unwrap();
    let directory = fs::canonicalize(temp.path()).unwrap().join("zoom");
    knowledge::build_cached(&conn, &ZoomOptions::default(), &directory).unwrap();
    let bytes = fs::read(directory.join("snapshot.json")).unwrap();
    let corruption =
        "UPDATE evidence_snapshots SET exact_excerpt='A substituted source quote' WHERE id=?1";
    assert!(conn.execute(corruption, [&original.evidence]).is_err());
    // Simulate damage beyond the normal immutable-storage guard in this
    // isolated in-memory fixture. The cache must still inspect source bytes.
    conn.execute_batch("DROP TRIGGER forbid_snapshot_update")
        .unwrap();
    conn.execute(corruption, [&original.evidence]).unwrap();
    let error = knowledge::build_cached(&conn, &ZoomOptions::default(), &directory).unwrap_err();
    assert!(error.to_string().contains("stored evidence digest"));
    assert!(conn.is_autocommit());
    assert_eq!(fs::read(directory.join("snapshot.json")).unwrap(), bytes);
}

#[test]
fn rejected_partial_reuse_counts_discarded_work_before_full_rebuild() {
    let conn = database();
    let mut originals = Vec::new();
    for index in 0..3 {
        originals.push(record(
            &conn,
            &format!("source-{index}.md"),
            &format!("topic-{index}"),
            &format!("component {index}"),
            "design",
            &format!("Component {index} has an independent documented role."),
        ));
    }
    storage::refresh_knowledge(&conn, "p").unwrap();
    let temp = tempfile::tempdir().unwrap();
    let directory = fs::canonicalize(temp.path()).unwrap().join("zoom");
    let first = knowledge::build_cached(&conn, &ZoomOptions::default(), &directory).unwrap();
    let leaves = first
        .graph
        .nodes
        .iter()
        .filter(|node| node.kind == "evidence")
        .collect::<Vec<_>>();
    let retired = originals
        .iter()
        .find(|record| leaves[0].knowledge_ids.contains(&record.id))
        .unwrap();
    let poison = &leaves.last().unwrap().id;
    let path = directory.join("snapshot.json");
    let mut raw: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    let node = raw["nodes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|node| node["id"] == *poison)
        .unwrap();
    node["summary"] = "A forged statement in a later unchanged node.".into();
    resign_cache(&mut raw);
    fs::write(&path, serde_json::to_vec(&raw).unwrap()).unwrap();
    storage::retire_source(&conn, &retired.source).unwrap();
    storage::refresh_knowledge(&conn, "p").unwrap();
    let repaired = knowledge::build_cached(&conn, &ZoomOptions::default(), &directory).unwrap();
    assert!(!repaired.refresh.topology_reused);
    assert!(repaired.refresh.summaries_generated > repaired.graph.nodes.len());
    assert!(repaired.refresh.dependency_revisions_generated > repaired.graph.nodes.len());
    assert!(repaired.refresh.summaries_validated > 0);
    assert_eq!(repaired.refresh.layout_generation_work, 0);
    assert_eq!(
        repaired.refresh.topology_validation_work,
        repaired.graph.report.work_used
    );
    assert_eq!(repaired.regenerated_nodes, repaired.graph.nodes.len());
    assert_eq!(repaired.reused_nodes, 0);
    assert_matches_fresh(&repaired.graph, &conn);
}

fn crossing_memberships(conn: &Connection) -> Vec<Record> {
    [
        ("a.md", "north east alpha"),
        ("b.md", "north east beta"),
        ("c.md", "north gamma"),
        ("d.md", "east delta"),
    ]
    .into_iter()
    .map(|(path, subject)| {
        record(
            conn,
            path,
            "architecture",
            subject,
            "design",
            &format!("The {subject} component has a documented responsibility."),
        )
    })
    .collect()
}

fn force_cached_dependency_refresh(value: &mut serde_json::Value) {
    for dependency in value["incremental"]["record_dependencies"]
        .as_object_mut()
        .unwrap()
        .values_mut()
    {
        *dependency = "changed dependency sentinel".into();
    }
}

fn replace_cached_edges(value: &mut serde_json::Value, edges: Vec<knowledge::ViewEdge>) {
    let edges = edges.into_iter().collect::<BTreeSet<_>>();
    for node in value["nodes"].as_array_mut().unwrap() {
        let id = node["id"].as_str().unwrap().to_owned();
        node["parent_view_ids"] = serde_json::json!(
            edges
                .iter()
                .filter(|edge| edge.child == id)
                .map(|edge| &edge.parent)
                .collect::<Vec<_>>()
        );
        node["child_view_ids"] = serde_json::json!(
            edges
                .iter()
                .filter(|edge| edge.parent == id)
                .map(|edge| &edge.child)
                .collect::<Vec<_>>()
        );
    }
    value["edges"] = serde_json::to_value(edges).unwrap();
}

#[test]
fn a_resigned_cache_cannot_invent_warnings_or_conceal_real_truncation() {
    let conn = database();
    crossing_memberships(&conn);
    storage::refresh_knowledge(&conn, "p").unwrap();
    let options = ZoomOptions {
        max_nodes: 2,
        ..ZoomOptions::default()
    };
    let temp = tempfile::tempdir().unwrap();
    let directory = fs::canonicalize(temp.path()).unwrap().join("zoom");
    let first = knowledge::build_cached(&conn, &options, &directory).unwrap();
    assert!(first.graph.report.truncated);
    assert!(!first.graph.report.warnings.is_empty());
    let path = directory.join("snapshot.json");
    let original: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    for conceal in [false, true] {
        let mut changed = original.clone();
        if conceal {
            changed["report"]["warnings"] = serde_json::json!([]);
            changed["report"]["truncated"] = false.into();
        } else {
            changed["report"]["warnings"]
                .as_array_mut()
                .unwrap()
                .push("An invented statement that checkout behavior was verified.".into());
        }
        resign_cache(&mut changed);
        fs::write(&path, serde_json::to_vec(&changed).unwrap()).unwrap();
        let repaired = knowledge::build_cached(&conn, &options, &directory).unwrap();
        assert!(!repaired.refresh.topology_reused);
        assert!(
            repaired
                .refresh
                .fallback_reason
                .as_deref()
                .unwrap()
                .contains("build report differs")
        );
        assert!(repaired.graph.report.truncated);
        assert_eq!(repaired.graph.report.warnings, first.graph.report.warnings);
        assert_eq!(repaired.refresh.layout_generation_work, 0);
        assert_eq!(
            repaired.refresh.topology_validation_work,
            repaired.graph.report.work_used
        );
        assert_matches_fresh(&repaired.graph, &conn);
    }
}

#[test]
fn a_resigned_cache_cannot_drop_one_of_two_canonical_parents() {
    let conn = database();
    crossing_memberships(&conn);
    storage::refresh_knowledge(&conn, "p").unwrap();
    let temp = tempfile::tempdir().unwrap();
    let directory = fs::canonicalize(temp.path()).unwrap().join("zoom");
    let first = knowledge::build_cached(&conn, &ZoomOptions::default(), &directory).unwrap();
    let child = first
        .graph
        .nodes
        .iter()
        .find(|node| node.parent_view_ids.len() >= 2)
        .unwrap();
    let parent = &child.parent_view_ids[0];
    let edges = first
        .graph
        .edges
        .iter()
        .filter(|edge| !(edge.child == child.id && &edge.parent == parent))
        .cloned()
        .collect();
    let path = directory.join("snapshot.json");
    let mut changed: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    replace_cached_edges(&mut changed, edges);
    // A valid remaining path and fresh node fingerprints used to mask the
    // omitted parent. Neither the outer checksum nor forced refresh is proof
    // of complete navigation.
    force_cached_dependency_refresh(&mut changed);
    resign_cache(&mut changed);
    fs::write(&path, serde_json::to_vec(&changed).unwrap()).unwrap();
    let repaired = knowledge::build_cached(&conn, &ZoomOptions::default(), &directory).unwrap();
    assert!(!repaired.refresh.topology_reused);
    assert!(
        repaired
            .refresh
            .fallback_reason
            .as_deref()
            .unwrap()
            .contains("complete canonical source layout")
    );
    assert_eq!(repaired.graph.edges, first.graph.edges);
    assert_eq!(repaired.refresh.layout_generation_work, 0);
    assert_eq!(
        repaired.refresh.topology_validation_work,
        repaired.graph.report.work_used
    );
    assert_matches_fresh(&repaired.graph, &conn);
}

#[test]
fn a_resigned_cache_cannot_remove_a_derived_intersection_and_flatten_its_children() {
    let conn = database();
    let originals = crossing_memberships(&conn);
    storage::refresh_knowledge(&conn, "p").unwrap();
    let temp = tempfile::tempdir().unwrap();
    let directory = fs::canonicalize(temp.path()).unwrap().join("zoom");
    let first = knowledge::build_cached(&conn, &ZoomOptions::default(), &directory).unwrap();
    let members = BTreeSet::from([originals[0].id.clone(), originals[1].id.clone()]);
    let intersection = first
        .graph
        .nodes
        .iter()
        .find(|node| {
            node.kind == "concept"
                && node.knowledge_ids.iter().cloned().collect::<BTreeSet<_>>() == members
        })
        .unwrap();
    assert_eq!(
        intersection.grouping_basis,
        ["concept: east", "concept: north"]
    );
    assert_eq!(intersection.parent_view_ids.len(), 2);
    let mut edges = first
        .graph
        .edges
        .iter()
        .filter(|edge| edge.parent != intersection.id && edge.child != intersection.id)
        .cloned()
        .collect::<Vec<_>>();
    for parent in &intersection.parent_view_ids {
        for child in &intersection.child_view_ids {
            edges.push(knowledge::ViewEdge {
                parent: parent.clone(),
                child: child.clone(),
                kind: "contains".into(),
            });
        }
    }
    let path = directory.join("snapshot.json");
    let mut changed: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    changed["nodes"]
        .as_array_mut()
        .unwrap()
        .retain(|node| node["id"] != intersection.id);
    replace_cached_edges(&mut changed, edges);
    force_cached_dependency_refresh(&mut changed);
    resign_cache(&mut changed);
    fs::write(&path, serde_json::to_vec(&changed).unwrap()).unwrap();
    let repaired = knowledge::build_cached(&conn, &ZoomOptions::default(), &directory).unwrap();
    assert!(!repaired.refresh.topology_reused);
    assert!(
        repaired
            .refresh
            .fallback_reason
            .as_deref()
            .unwrap()
            .contains("complete canonical source layout")
    );
    assert_eq!(repaired.graph.nodes, first.graph.nodes);
    assert_eq!(repaired.graph.edges, first.graph.edges);
    assert_matches_fresh(&repaired.graph, &conn);
}

fn assert_compact_budget(result: &knowledge::CompactExploreResult) {
    let actual = lore::context::count_tokens(&(serde_json::to_string(result).unwrap() + "\n")).max(
        lore::context::count_tokens(&knowledge::render_compact_markdown(result).unwrap()),
    );
    assert!(
        actual <= result.used_tokens,
        "{actual} > {}",
        result.used_tokens
    );
    assert!(result.used_tokens <= result.max_tokens);
}

fn assert_compact_originals(conn: &Connection, result: &knowledge::CompactExploreResult) {
    assert_compact_budget(result);
    let resolved = result.resolve().unwrap();
    let originals = storage::views(conn).unwrap();
    for record in &resolved.knowledge {
        let original = originals
            .iter()
            .find(|original| original.id == record.id)
            .unwrap();
        assert_eq!(
            serde_json::to_value(record).unwrap(),
            serde_json::to_value(original).unwrap()
        );
    }
    for evidence in &resolved.evidence {
        let original = storage::evidence_snapshot(conn, &evidence.id).unwrap();
        assert_eq!(
            serde_json::to_value(evidence).unwrap(),
            serde_json::to_value(original).unwrap()
        );
    }
    for relation in &resolved.relations {
        assert!(
            resolved
                .knowledge
                .iter()
                .any(|record| record.id == relation.from)
        );
        assert!(
            resolved
                .knowledge
                .iter()
                .any(|record| record.id == relation.to)
        );
        assert!(
            resolved
                .evidence
                .iter()
                .any(|evidence| evidence.id == relation.evidence_id)
        );
    }
}

#[test]
fn dense_shared_topic_does_not_make_unrelated_conditions_mandatory() {
    let conn = database();
    let capacity = record(
        &conn,
        "queue-limit.md",
        "operations",
        "queue admission",
        "decision",
        "QUEUE_LIMIT is 8 pending jobs.",
    );
    let exception = record(
        &conn,
        "queue-drain.md",
        "operations",
        "queue admission",
        "constraint",
        "Queue admission must stop while a drain is active.",
    );
    for index in 0..24 {
        record(
            &conn,
            &format!("credentials-{index}.md"),
            "operations",
            &format!("credential rotation {index}"),
            "constraint",
            &format!("Credential {index} must be rotated after its individual expiry date."),
        );
    }
    let graph = build(&conn);
    let bundles = knowledge::inspect_bundles(&graph, std::slice::from_ref(&capacity.id)).unwrap();
    assert_eq!(bundles.len(), 1);
    assert_eq!(
        bundles[0].knowledge_ids,
        BTreeSet::from([capacity.id.clone(), exception.id.clone()])
    );
    assert!(bundles[0].dependencies.iter().any(|dependency| {
        dependency.from == capacity.id
            && dependency.to == exception.id
            && dependency.reason == "directly_applicable_explicit_condition"
    }));
    for max_tokens in [512, 1_000, 1_500, 3_000, 8_000] {
        let options = ExploreOptions {
            query: "QUEUE_LIMIT".into(),
            max_tokens,
            ..ExploreOptions::default()
        };
        let result = knowledge::select(&graph, &options).unwrap();
        assert_complete_budget(&result);
        let ids: BTreeSet<_> = result
            .knowledge
            .iter()
            .map(|record| record.id.as_str())
            .collect();
        assert_eq!(
            ids.contains(capacity.id.as_str()),
            ids.contains(exception.id.as_str())
        );
        let compact = knowledge::select_compact(&graph, &options).unwrap();
        assert_compact_originals(&conn, &compact);
        let resolved = compact.resolve().unwrap();
        let ids: BTreeSet<_> = resolved
            .knowledge
            .iter()
            .map(|record| record.id.as_str())
            .collect();
        assert_eq!(
            ids.contains(capacity.id.as_str()),
            ids.contains(exception.id.as_str())
        );
        if max_tokens >= 1_500 {
            assert!(
                ids.contains(capacity.id.as_str()),
                "complete compact pair did not fit {max_tokens}: {compact:#?}"
            );
        }
    }
}

#[test]
fn compact_conditions_survive_node_focus_and_cannot_be_replaced_by_navigation() {
    let conn = database();
    let capacity = record(
        &conn,
        "capacity.md",
        "dispatch",
        "dispatch",
        "decision",
        "QUEUE_CAPACITY is 128 jobs in production and 64 jobs in staging.",
    );
    let full = record(
        &conn,
        "full.md",
        "dispatch",
        "dispatch",
        "constraint",
        "A full production dispatch queue must return Busy without accepting a job.",
    );
    let exception = record(
        &conn,
        "drain.md",
        "dispatch",
        "dispatch",
        "constraint",
        "Except during an emergency drain, production dispatch must preserve tenant order; a drain still refuses new jobs.",
    );
    let purpose = record(
        &conn,
        "purpose.md",
        "dispatch",
        "dispatch",
        "decision",
        "Harbor isolates dispatch admission from worker execution so a slow tenant cannot consume every worker.",
    );
    let graph = build(&conn);
    let bundle = knowledge::inspect_bundles(&graph, std::slice::from_ref(&capacity.id)).unwrap();
    assert!(!bundle[0].knowledge_ids.contains(&purpose.id));
    let leaf = graph
        .nodes
        .iter()
        .find(|node| node.kind == "evidence" && node.knowledge_ids == [capacity.id.clone()])
        .unwrap();
    for node in [None, Some(leaf.id.clone())] {
        let options = ExploreOptions {
            query: "increase QUEUE_CAPACITY for production dispatch".into(),
            node,
            max_tokens: 1_500,
            ..ExploreOptions::default()
        };
        let compact = knowledge::select_compact(&graph, &options).unwrap();
        assert_compact_originals(&conn, &compact);
        let result = compact.resolve().unwrap();
        let ids: BTreeSet<_> = result
            .knowledge
            .iter()
            .map(|record| record.id.as_str())
            .collect();
        assert!(
            ids.contains(capacity.id.as_str()),
            "capacity missing: {compact:#?}"
        );
        assert!(
            ids.contains(full.id.as_str()),
            "full-queue condition missing"
        );
        assert!(
            ids.contains(exception.id.as_str()),
            "rare exception missing"
        );
    }
    // Independent minimum: original records and their exact evidence alone,
    // before adding any envelope, navigation, sources or qualifications, already
    // exceed v1's 1500-token allowance. This is not a successful v1 retrieval.
    let required = [&capacity.id, &full.id, &exception.id];
    let originals: Vec<_> = graph
        .knowledge
        .iter()
        .filter(|record| required.contains(&&record.id))
        .collect();
    let cited: BTreeSet<_> = originals
        .iter()
        .flat_map(|record| record.evidence.iter().map(|e| &e.id))
        .collect();
    let evidence: Vec<_> = graph
        .evidence
        .iter()
        .filter(|e| cited.contains(&e.id))
        .collect();
    let mandatory_originals = serde_json::to_string_pretty(&originals).unwrap();
    let mandatory_evidence = serde_json::to_string_pretty(&evidence).unwrap();
    assert!(lore::context::count_tokens(&(mandatory_originals + &mandatory_evidence)) > 1_500);
}

#[test]
fn compact_conceptual_queries_pack_complete_rules_before_explanatory_context() {
    let conn = database();
    let original = |path, statement| {
        let (proposal, source, assertion, evidence) = capture_with_excerpt(
            &conn,
            path,
            "dispatch",
            "dispatch",
            "decision",
            statement,
            &format!("DECISION dispatch: {statement}"),
        );
        let id = storage::create_unit(&conn, "p", &assertion, &proposal).unwrap();
        Record {
            id,
            source,
            assertion,
            evidence,
        }
    };
    let capacity = original(
        "capacity.md",
        "QUEUE_CAPACITY is 128 jobs in production and 64 jobs in staging.",
    );
    let full = original(
        "full.md",
        "A full production dispatch queue must return Busy without accepting a job.",
    );
    let exception = original(
        "drain.md",
        "Except during an emergency drain, production dispatch must preserve tenant order; a drain still refuses new jobs.",
    );
    let purpose = original(
        "purpose.md",
        "Harbor isolates dispatch admission from worker execution so a slow tenant cannot consume every worker.",
    );
    let graph = build(&conn);
    let options = ExploreOptions {
        query: "tenant worker dispatch admission".into(),
        max_tokens: 1_500,
        ..ExploreOptions::default()
    };
    let all = knowledge::select_compact_controlled(
        &graph,
        &ExploreOptions {
            max_tokens: 8_000,
            ..options.clone()
        },
        &[
            capacity.id.clone(),
            full.id.clone(),
            exception.id.clone(),
            purpose.id.clone(),
        ],
        knowledge::SelectionMode::DirectOnly,
    )
    .unwrap();
    assert_eq!(all.knowledge.len(), 4);
    assert!(
        lore::context::count_tokens(&(serde_json::to_string(&all).unwrap() + "\n")).max(
            lore::context::count_tokens(&knowledge::render_compact_markdown(&all).unwrap())
        ) > options.max_tokens,
        "fixture must require a choice between complete rules and explanatory context"
    );
    let compact = knowledge::select_compact(&graph, &options).unwrap();
    assert_compact_originals(&conn, &compact);
    let resolved = compact.resolve().unwrap();
    let ids: BTreeSet<_> = resolved.knowledge.iter().map(|record| &record.id).collect();
    for required in [&capacity, &full, &exception] {
        assert!(
            ids.contains(&required.id),
            "feasible complete rule omitted for explanatory context: {compact:#?}"
        );
    }
    // Preserve the existing closure even when a complete alternative seed
    // bundle can answer the request with less explanatory context.
    let exception_bundle =
        knowledge::inspect_bundles(&graph, std::slice::from_ref(&exception.id)).unwrap();
    assert!(exception_bundle[0].knowledge_ids.contains(&purpose.id));

    // An exact request for the rationale still outranks general rule packing.
    // Its documentary conditions remain complete at the same tight budget.
    for query in [
        purpose.id.clone(),
        "purpose.md".into(),
        "Why is dispatch admission isolated from worker execution?".into(),
    ] {
        let compact = knowledge::select_compact(
            &graph,
            &ExploreOptions {
                query,
                ..options.clone()
            },
        )
        .unwrap();
        assert_compact_originals(&conn, &compact);
        let resolved = compact.resolve().unwrap();
        for required in [&purpose, &full, &exception] {
            assert!(
                resolved
                    .knowledge
                    .iter()
                    .any(|record| record.id == required.id)
            );
        }
    }
}

#[test]
fn compact_navigation_disclosure_is_reserved_at_an_exact_packing_boundary() {
    let token_count = |result: &knowledge::CompactExploreResult| {
        lore::context::count_tokens(&(serde_json::to_string(result).unwrap() + "\n")).max(
            lore::context::count_tokens(&knowledge::render_compact_markdown(result).unwrap()),
        )
    };
    // Paths change the source/evidence identities; generated knowledge IDs also
    // vary. Derive the exact boundary from each fixture's real serialized text
    // instead of depending on a lucky UUID or repeating a flaky fixed budget.
    for path in [
        "capacity.md",
        "dispatch/queue-capacity-17.md",
        "queue-v2026.md",
    ] {
        let conn = database();
        let admission = format!(
            "QUEUE_CAPACITY is 128 jobs in production and 64 jobs in staging. {}",
            "The dispatcher records a reservation before worker execution and keeps the tenant's ownership until completion is acknowledged. A restart preserves that original ownership and the environment's pending-job limit. ".repeat(12)
        );
        let capacity = record(
            &conn,
            path,
            "dispatch",
            "queue admission",
            "decision",
            &admission,
        );
        let exception = record(
            &conn,
            "drain.md",
            "dispatch",
            "queue admission",
            "constraint",
            "An emergency drain is the only exception to normal tenant ordering. Queue admission must still refuse new jobs until the drain is finished; operators cannot bypass the pending-job limit by describing a request as recovery work.",
        );
        let graph = build(&conn);
        let leaf = graph
            .nodes
            .iter()
            .find(|node| node.kind == "evidence" && node.knowledge_ids == [capacity.id.clone()])
            .unwrap();
        for node in [None, Some(leaf.id.clone())] {
            let mut options = ExploreOptions {
                query: "QUEUE_CAPACITY for production queue admission".into(),
                node,
                max_tokens: 30_000,
                ..ExploreOptions::default()
            };
            let complete = knowledge::select_compact(&graph, &options).unwrap();
            assert_compact_originals(&conn, &complete);
            assert!(!complete.navigation_truncated);
            assert!(complete.nodes.len() > 1);

            // One navigation node fits exactly before the omitted-navigation
            // disclosure is charged. A later node must be rejected at this
            // budget, which used to grow the already accepted response.
            let mut prefix = complete.clone();
            prefix.nodes.truncate(1);
            let node_ids: BTreeSet<_> = prefix.nodes.iter().map(|node| node.id.as_str()).collect();
            prefix.edges.retain(|edge| {
                node_ids.contains(edge.parent.as_str()) && node_ids.contains(edge.child.as_str())
            });
            for _ in 0..16 {
                let count = token_count(&prefix);
                if prefix.max_tokens == count && prefix.used_tokens == count {
                    break;
                }
                prefix.max_tokens = count;
                prefix.used_tokens = count;
            }
            assert_eq!(token_count(&prefix), prefix.max_tokens);
            assert!((512..30_000).contains(&prefix.max_tokens));
            let mut disclosed = prefix.clone();
            disclosed.navigation_truncated = true;
            assert!(token_count(&disclosed) > prefix.max_tokens);

            options.max_tokens = prefix.max_tokens;
            let result = knowledge::select_compact(&graph, &options).unwrap_or_else(|error| {
                panic!(
                    "{path}, focus {:?}, exact boundary {}: {error:#}",
                    options.node, options.max_tokens
                )
            });
            assert_compact_originals(&conn, &result);
            let resolved = result.resolve().unwrap();
            let retained: BTreeSet<_> =
                resolved.knowledge.iter().map(|record| &record.id).collect();
            assert!(retained.contains(&capacity.id));
            assert!(retained.contains(&exception.id));
            assert!(result.navigation_truncated);
            assert!(result.nodes.len() < complete.nodes.len());
        }
    }
}

#[test]
fn compact_disagreement_preserves_both_endpoints_witness_and_exact_resolution() {
    let conn = database();
    let old = record(
        &conn,
        "mysql.md",
        "database",
        "ledger database",
        "decision",
        "MySQL is the selected database.",
    );
    let new = record(
        &conn,
        "postgresql.md",
        "database",
        "ledger database",
        "decision",
        "PostgreSQL is the selected database.",
    );
    storage::add_relation(
        &conn,
        &new.id,
        &old.id,
        "contradicts",
        &new.assertion,
        &new.evidence,
    )
    .unwrap();
    let graph = build(&conn);
    let options = ExploreOptions {
        query: "selected ledger database".into(),
        max_tokens: 1_500,
        ..ExploreOptions::default()
    };
    let compact = knowledge::select_compact(&graph, &options).unwrap();
    assert_compact_originals(&conn, &compact);
    let resolved = compact.resolve().unwrap();
    assert_eq!(resolved.knowledge.len(), 2, "{compact:#?}");
    assert_eq!(resolved.relations.len(), 1);
    assert_eq!(resolved.relations[0].evidence_id, new.evidence);
    let bundles = knowledge::inspect_bundles(&graph, std::slice::from_ref(&old.id)).unwrap();
    assert!(
        bundles[0]
            .dependencies
            .iter()
            .any(|dependency| dependency.evidence_id.as_deref() == Some(&new.evidence))
    );
    for mutation in 0..6 {
        let mut changed = compact.clone();
        match mutation {
            0 => changed.evidence[0].1 = usize::MAX,
            1 => changed.knowledge[0].12[0].0 = usize::MAX,
            2 => changed.relations[0].4 = usize::MAX,
            3 => changed.relations[0].1 = usize::MAX,
            4 => changed.strings[changed.evidence[0].5].push_str(" forged exception"),
            _ => changed.source_revisions.clear(),
        }
        assert!(
            changed.resolve().is_err(),
            "resolver accepted mutation {mutation}"
        );
    }
}

#[test]
fn compact_resolver_rejects_overflowing_coverage_without_panicking() {
    let conn = database();
    record(
        &conn,
        "capacity.md",
        "dispatch",
        "dispatch",
        "decision",
        "QUEUE_CAPACITY is 128 jobs.",
    );
    let graph = build(&conn);
    let mut compact = knowledge::select_compact(
        &graph,
        &ExploreOptions {
            query: "QUEUE_CAPACITY".into(),
            ..ExploreOptions::default()
        },
    )
    .unwrap();
    assert_compact_originals(&conn, &compact);
    assert_eq!(compact.coverage.included_units, 1);
    compact.coverage.eligible_units = 0;
    compact.coverage.omitted_units = usize::MAX;
    assert!(compact.resolve().is_err());
}

#[test]
fn compact_staging_proposal_retains_accepted_baseline_without_promoting_proposal() {
    let conn = database();
    let capacity = record(
        &conn,
        "capacity.md",
        "dispatch",
        "dispatch",
        "decision",
        "QUEUE_CAPACITY is 128 jobs in production and 64 jobs in staging.",
    );
    record(
        &conn,
        "drain.md",
        "dispatch",
        "dispatch",
        "constraint",
        "Except during an emergency drain, production dispatch must preserve tenant order.",
    );
    let text = "Proposed staging queues would hold 256 jobs after the next capacity review.";
    let proposed_id = record(
        &conn,
        "proposal.md",
        "dispatch",
        "dispatch",
        "proposal",
        text,
    )
    .id;
    let graph = build(&conn);
    let compact = knowledge::select_compact(
        &graph,
        &ExploreOptions {
            query: "proposed staging queue capacity".into(),
            max_tokens: 1_500,
            ..ExploreOptions::default()
        },
    )
    .unwrap();
    assert_compact_originals(&conn, &compact);
    let result = compact.resolve().unwrap();
    assert!(
        result
            .knowledge
            .iter()
            .any(|record| record.id == capacity.id && record.lifecycle == "accepted")
    );
    assert!(
        result
            .knowledge
            .iter()
            .any(|record| record.id == proposed_id && record.lifecycle == "proposed")
    );
}

#[test]
fn controlled_selection_has_identical_candidate_and_obligation_sets() {
    let conn = database();
    let rule = record(
        &conn,
        "rule.md",
        "payments",
        "payment retry",
        "decision",
        "RETRY_LIMIT is three attempts.",
    );
    let exception = record(
        &conn,
        "exception.md",
        "payments",
        "payment retry",
        "constraint",
        "Payment retry must stop after settlement.",
    );
    record(
        &conn,
        "unrelated.md",
        "payments",
        "identity privacy",
        "constraint",
        "Identity tokens must stay private.",
    );
    let graph = build(&conn);
    let options = ExploreOptions {
        query: "RETRY_LIMIT".into(),
        max_tokens: 8_000,
        ..ExploreOptions::default()
    };
    let mut ids = Vec::new();
    for mode in [
        knowledge::SelectionMode::DirectOnly,
        knowledge::SelectionMode::GraphGuided,
    ] {
        let result = knowledge::select_compact_controlled(
            &graph,
            &options,
            std::slice::from_ref(&rule.id),
            mode,
        )
        .unwrap();
        assert_compact_originals(&conn, &result);
        let resolved = result.resolve().unwrap();
        ids.push(
            resolved
                .knowledge
                .iter()
                .map(|record| record.id.clone())
                .collect::<BTreeSet<_>>(),
        );
    }
    assert_eq!(ids[0], ids[1]);
    assert_eq!(ids[0], BTreeSet::from([rule.id, exception.id]));
    assert!(
        knowledge::select_controlled(
            &graph,
            &options,
            &["ku_absent".into()],
            knowledge::SelectionMode::DirectOnly
        )
        .is_err()
    );
}

#[test]
fn compact_historical_evidence_and_unfit_obligations_are_explicit() {
    let conn = database();
    let old = record(
        &conn,
        "old.md",
        "retry",
        "retry",
        "decision",
        "OLD_RETRY_LIMIT was three attempts.",
    );
    storage::retire_source(&conn, &old.source).unwrap();
    let huge = record(
        &conn,
        "huge.md",
        "limits",
        "memory limits",
        "constraint",
        &format!(
            "HUGE_MEMORY_LIMIT applies only before initialization. {}",
            "a b c d e f g h i j ".repeat(170)
        ),
    );
    let graph = build(&conn);
    let original = knowledge::select_compact(
        &graph,
        &ExploreOptions {
            query: old.id.clone(),
            max_tokens: 1_500,
            ..ExploreOptions::default()
        },
    )
    .unwrap();
    assert_compact_originals(&conn, &original);
    let resolved = original.resolve().unwrap();
    assert_eq!(resolved.knowledge[0].support_state, "historical_only");
    assert!(
        resolved.knowledge[0]
            .evidence
            .iter()
            .all(|citation| !citation.active)
    );
    assert!(
        resolved
            .source_revisions
            .iter()
            .all(|source| !source.current)
    );
    for max_tokens in [512, 1_000, 1_500, 3_000] {
        let result = knowledge::select_compact(
            &graph,
            &ExploreOptions {
                query: huge.id.clone(),
                max_tokens,
                ..ExploreOptions::default()
            },
        )
        .unwrap();
        assert_compact_originals(&conn, &result);
        assert_eq!(result.status, "budget_limited");
        assert!(result.knowledge.is_empty());
        assert!(result.critical_groups_omitted > 0);
        assert_eq!(result.coverage.omitted_units, 1);
        assert!(!result.coverage.omission_reasons.is_empty());
    }
    let sparse_match = knowledge::select_compact(
        &graph,
        &ExploreOptions {
            query: "initialization".into(),
            max_tokens: 512,
            ..ExploreOptions::default()
        },
    )
    .unwrap();
    assert_compact_originals(&conn, &sparse_match);
    assert_eq!(sparse_match.status, "budget_limited");
    assert_eq!(sparse_match.coverage.omitted_units, 1);
    let absent = knowledge::select_compact(
        &graph,
        &ExploreOptions {
            query: "ABSENT_SYNCHRONIZER_IDENTIFIER".into(),
            max_tokens: 512,
            ..ExploreOptions::default()
        },
    )
    .unwrap();
    assert_compact_originals(&conn, &absent);
    assert_eq!(absent.status, "no_matches");
}

#[test]
fn explanatory_language_cannot_hide_explicit_permissions_or_mandatory_rules() {
    let conn = database();
    let rule = record(
        &conn,
        "rule.md",
        "dispatch",
        "dispatch",
        "decision",
        "QUEUE_LIMIT is 8 pending jobs.",
    );
    let permission = record(
        &conn,
        "permission.md",
        "dispatch",
        "dispatch",
        "decision",
        "Admission authenticates callers so unauthorized jobs cannot enter the queue.",
    );
    let mandatory = record(
        &conn,
        "mandatory.md",
        "dispatch",
        "dispatch",
        "decision",
        "Dispatch must require signed grants so invalid callers cannot enter.",
    );
    let typed = record(
        &conn,
        "typed.md",
        "dispatch",
        "dispatch",
        "constraint",
        "Credentials are checked so callers cannot exceed their authorization.",
    );
    let purpose = record(
        &conn,
        "purpose.md",
        "dispatch",
        "dispatch",
        "decision",
        "Harbor isolates dispatch admission from worker execution so a slow tenant cannot consume every worker.",
    );
    let graph = build(&conn);
    let bundles = knowledge::inspect_bundles(&graph, std::slice::from_ref(&rule.id)).unwrap();
    for record in [&rule, &permission, &mandatory, &typed] {
        assert!(bundles[0].knowledge_ids.contains(&record.id));
    }
    assert!(!bundles[0].knowledge_ids.contains(&purpose.id));
    let result = knowledge::select_compact(
        &graph,
        &ExploreOptions {
            query: "QUEUE_LIMIT".into(),
            max_tokens: 512,
            ..ExploreOptions::default()
        },
    )
    .unwrap();
    assert_compact_originals(&conn, &result);
    let resolved = result.resolve().unwrap();
    assert!(!resolved.knowledge.iter().any(|record| record.id == rule.id));
    assert_eq!(result.status, "budget_limited");
}

#[test]
fn generic_shared_cli_words_are_context_not_transitive_obligations() {
    let conn = database();
    let rule = record(
        &conn,
        "hidden.md",
        "search",
        "hidden files",
        "constraint",
        "File search includes hidden files only with --hidden.",
    );
    let exception = record(
        &conn,
        "confidential.md",
        "search",
        "confidential directories",
        "constraint",
        "Hidden files must remain ignored when their parent directory is confidential.",
    );
    let unrelated = record(
        &conn,
        "workers.md",
        "search",
        "parallel workers",
        "constraint",
        "File search uses four parallel workers when processing input files.",
    );
    let other = record(
        &conn,
        "binary.md",
        "search",
        "binary content",
        "constraint",
        "File search treats binary input as opaque content.",
    );
    let graph = build(&conn);
    let bundles = knowledge::inspect_bundles(&graph, std::slice::from_ref(&rule.id)).unwrap();
    assert_eq!(
        bundles[0].knowledge_ids,
        BTreeSet::from([rule.id.clone(), exception.id.clone()])
    );
    assert!(!bundles[0].knowledge_ids.contains(&unrelated.id));
    assert!(!bundles[0].knowledge_ids.contains(&other.id));
    let result = knowledge::select_compact(
        &graph,
        &ExploreOptions {
            query: rule.id.clone(),
            max_tokens: 1_500,
            ..ExploreOptions::default()
        },
    )
    .unwrap();
    assert_compact_originals(&conn, &result);
    let resolved = result.resolve().unwrap();
    assert!(resolved.knowledge.iter().any(|record| record.id == rule.id));
    assert!(
        resolved
            .knowledge
            .iter()
            .any(|record| record.id == exception.id)
    );
}

#[test]
fn sentence_punctuation_does_not_turn_prose_into_an_exact_reference() {
    let conn = database();
    let expected = record(
        &conn,
        "archive.md",
        "search",
        "compressed archives",
        "constraint",
        "Compressed archives are searched with the --archive flag.",
    );
    record(
        &conn,
        "plain.md",
        "search",
        "plain text",
        "constraint",
        "Ordinary text search accepts files.",
    );
    let graph = build(&conn);
    let result = knowledge::select_compact(
        &graph,
        &ExploreOptions {
            query: "Search compressed archives inside files.".into(),
            max_tokens: 1_500,
            ..ExploreOptions::default()
        },
    )
    .unwrap();
    assert_compact_originals(&conn, &result);
    assert_eq!(result.retrieval, "cross_level");
    assert!(
        result
            .resolve()
            .unwrap()
            .knowledge
            .iter()
            .any(|record| record.id == expected.id)
    );
}

#[test]
fn format_acronyms_do_not_bind_unrelated_options_but_explicit_flags_do() {
    let conn = database();
    let rule = record(
        &conn,
        "plain.md",
        "formatting",
        "plain output",
        "constraint",
        "The --plain-text option writes XML text directly.",
    );
    let exception = record(
        &conn,
        "nul.md",
        "formatting",
        "embedded characters",
        "constraint",
        "With --plain-text, embedded NUL characters must produce an error.",
    );
    let unrelated = record(
        &conn,
        "decode.md",
        "formatting",
        "input parsing",
        "constraint",
        "The decoder must parse XML input using the --decode option.",
    );
    let extension = record(
        &conn,
        "syntax.md",
        "formatting",
        "syntax checking",
        "constraint",
        "The --plain-text-syntax option must validate XML document structure.",
    );
    let graph = build(&conn);
    let bundle = knowledge::inspect_bundles(&graph, std::slice::from_ref(&rule.id)).unwrap();
    assert_eq!(
        bundle[0].knowledge_ids,
        BTreeSet::from([rule.id.clone(), exception.id.clone()])
    );
    assert!(!bundle[0].knowledge_ids.contains(&unrelated.id));
    assert!(!bundle[0].knowledge_ids.contains(&extension.id));
    let result = knowledge::select_compact(
        &graph,
        &ExploreOptions {
            query: "What does --plain-text output for XML?".into(),
            max_tokens: 1_500,
            ..ExploreOptions::default()
        },
    )
    .unwrap();
    assert_compact_originals(&conn, &result);
    assert_eq!(result.retrieval, "direct_reference");
    let resolved = result.resolve().unwrap();
    for expected in [&rule.id, &exception.id] {
        assert!(
            resolved
                .knowledge
                .iter()
                .any(|record| &record.id == expected)
        );
    }
}

#[test]
fn requested_body_conditions_outrank_a_broad_matching_subject() {
    let conn = database();
    let required = record(
        &conn,
        "headerless.md",
        "export",
        "headerless export",
        "constraint",
        "The --headerless mode writes binary content without an audit header.",
    );
    let related = record(
        &conn,
        "binary.md",
        "export",
        "binary payload",
        "constraint",
        "The --binary option exports a binary payload with an audit header. The header records the producing service, encoding version and creation time. This mode supports interchange between compatible services and retains the original creation information for downstream readers. Every reader must validate that information before it processes the message contents.",
    );
    let graph = build(&conn);
    // Both complete originals fit separately, so skipping an oversized rival
    // cannot explain the requested rule's selection under the shared budget.
    for id in [&required.id, &related.id] {
        let alone = knowledge::select_compact(
            &graph,
            &ExploreOptions {
                query: id.clone(),
                max_tokens: 800,
                ..ExploreOptions::default()
            },
        )
        .unwrap();
        assert!(
            alone
                .resolve()
                .unwrap()
                .knowledge
                .iter()
                .any(|record| &record.id == id)
        );
    }
    let result = knowledge::select_compact(
        &graph,
        &ExploreOptions {
            query: "Produce a binary payload without an audit header.".into(),
            max_tokens: 800,
            ..ExploreOptions::default()
        },
    )
    .unwrap();
    assert_compact_originals(&conn, &result);
    let resolved = result.resolve().unwrap();
    assert!(
        resolved
            .knowledge
            .iter()
            .any(|record| record.id == required.id)
    );
    assert!(
        !resolved
            .knowledge
            .iter()
            .any(|record| record.id == related.id),
        "both originals fit at {} tokens",
        result.used_tokens
    );
}
