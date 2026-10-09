//! Disposable, evidence-bound views over the existing knowledge registry.
//!
//! The containment DAG is a navigation hypothesis. Original records and exact
//! evidence remain independently retrievable; neither a group nor its summary
//! creates a new authority. All default operations are deterministic and local.
mod cache;
mod graph;
mod selection;

pub use cache::{CacheUpdate, build_cached, purge_cache};
pub use graph::validate;
pub use selection::{render_markdown, select};

use crate::{domain::KnowledgeView, storage, util};
use anyhow::{Result, ensure};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const ZOOM_SCHEMA_VERSION: u32 = 1;
pub const GROUPING_VERSION: &str = "semantic-membership-dag-v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ZoomOptions {
    pub max_nodes: usize,
    pub max_edges: usize,
    pub max_work: usize,
    pub max_records: usize,
    pub max_input_bytes: usize,
}

impl Default for ZoomOptions {
    fn default() -> Self {
        Self {
            max_nodes: 2_048,
            max_edges: 8_192,
            max_work: 500_000,
            max_records: 10_000,
            max_input_bytes: 32 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExploreOptions {
    pub query: String,
    pub node: Option<String>,
    pub max_tokens: usize,
    pub max_nodes: usize,
}

impl Default for ExploreOptions {
    fn default() -> Self {
        Self {
            query: String::new(),
            node: None,
            max_tokens: 8_000,
            max_nodes: 24,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Coverage {
    pub eligible_units: usize,
    pub included_units: usize,
    pub omitted_units: usize,
    pub omission_reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ViewNode {
    pub id: String,
    pub revision: String,
    pub kind: String,
    pub title: String,
    /// A description of the grouping, never independent evidence for a claim.
    pub summary: String,
    /// Original records quoted in a bounded extractive summary.
    pub summary_knowledge_ids: Vec<String>,
    pub summary_coverage: Coverage,
    pub grouping_basis: Vec<String>,
    pub knowledge_ids: Vec<String>,
    pub critical_knowledge_ids: Vec<String>,
    pub child_view_ids: Vec<String>,
    pub parent_view_ids: Vec<String>,
    pub coverage: Coverage,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct ViewEdge {
    pub parent: String,
    pub child: String,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceRelation {
    pub id: String,
    pub from: String,
    pub to: String,
    pub kind: String,
    pub evidence_id: String,
    pub assertion_revision_id: String,
    pub source_id: String,
    pub source_revision_id: String,
    pub source_locator: String,
    pub active: bool,
    pub exact_excerpt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceRevision {
    pub id: String,
    pub source_id: String,
    pub root_id: String,
    pub observed_path: String,
    pub content_digest: String,
    pub current: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BuildReport {
    pub work_used: usize,
    pub work_limit: usize,
    pub truncated: bool,
    pub omitted_navigation_units: usize,
    pub unsupported_units: usize,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeGraph {
    pub schema_version: u32,
    pub grouping_version: String,
    pub revision: String,
    pub root_id: String,
    pub model_calls: u32,
    pub options: ZoomOptions,
    pub nodes: Vec<ViewNode>,
    pub edges: Vec<ViewEdge>,
    /// Original, revision-bound records; their contents are never replaced by
    /// generated summaries and are not limited by the navigation node budget.
    pub knowledge: Vec<KnowledgeView>,
    pub evidence: Vec<storage::EvidenceSnapshot>,
    pub relations: Vec<EvidenceRelation>,
    pub source_revisions: Vec<SourceRevision>,
    pub report: BuildReport,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavigationNode {
    pub id: String,
    pub revision: String,
    pub title: String,
    pub kind: String,
    pub summary: String,
    pub summary_knowledge_ids: Vec<String>,
    pub parent_view_ids: Vec<String>,
    pub child_view_ids: Vec<String>,
    pub omitted_parent_nodes: usize,
    pub omitted_child_nodes: usize,
    pub eligible_units: usize,
    pub critical_units: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExploreResult {
    pub schema_version: u32,
    pub snapshot_revision: String,
    pub query: String,
    pub selected_node_id: Option<String>,
    pub retrieval: String,
    pub status: String,
    pub model_calls: u32,
    pub nodes: Vec<NavigationNode>,
    pub edges: Vec<ViewEdge>,
    pub knowledge: Vec<KnowledgeView>,
    pub evidence: Vec<storage::EvidenceSnapshot>,
    pub relations: Vec<EvidenceRelation>,
    pub source_revisions: Vec<SourceRevision>,
    pub coverage: Coverage,
    pub critical_groups_omitted: usize,
    pub navigation_truncated: bool,
    pub max_tokens: usize,
    pub used_tokens: usize,
    pub warnings: Vec<String>,
}

/// Build one coherent, read-only SQLite snapshot. No schema migration, source
/// read, network call, model invocation or authoritative write is performed.
pub fn build(conn: &Connection, options: &ZoomOptions) -> Result<KnowledgeGraph> {
    ensure!(
        (2..=65_536).contains(&options.max_nodes)
            && options.max_edges >= options.max_nodes - 1
            && options.max_edges <= 262_144
            && options.max_work > 0
            && options.max_work <= 50_000_000
            && (1..=50_000).contains(&options.max_records)
            && options.max_input_bytes > 0
            && options.max_input_bytes <= 256 * 1024 * 1024,
        "invalid Knowledge Zoom resource limits"
    );
    let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    ensure!(
        (4..=storage::SCHEMA_VERSION).contains(&version),
        "Knowledge Zoom requires a compatible compiled registry; no migration was performed"
    );
    conn.execute_batch("SAVEPOINT lore_knowledge_zoom")?;
    let result = load_and_build(conn, options);
    if result.is_err() {
        let _ = conn.execute_batch("ROLLBACK TO lore_knowledge_zoom");
    }
    let released = conn.execute_batch("RELEASE lore_knowledge_zoom");
    let result = result?;
    released?;
    Ok(result)
}

fn load_and_build(conn: &Connection, options: &ZoomOptions) -> Result<KnowledgeGraph> {
    let count: usize =
        conn.query_row("SELECT count(*) FROM knowledge_current", [], |r| r.get(0))?;
    let evidence_bytes: usize = conn.query_row(
        "SELECT COALESCE(sum(length(CAST(exact_excerpt AS BLOB)) + length(CAST(context_before AS BLOB)) + length(CAST(context_after AS BLOB))),0) FROM evidence_snapshots",
        [],
        |r| r.get(0),
    )?;
    ensure!(
        count <= options.max_records && evidence_bytes <= options.max_input_bytes,
        "Knowledge Zoom input exceeds its explicit record/byte budget; direct lore context and evidence retrieval remain available"
    );
    let mut knowledge = storage::views(conn)?;
    knowledge.sort_by(|a, b| a.id.cmp(&b.id));
    let unsupported = knowledge
        .iter()
        .filter(|v| v.support_state == "unsupported" || v.evidence.is_empty())
        .count();
    knowledge.retain(|v| v.support_state != "unsupported" && !v.evidence.is_empty());
    let ids: BTreeSet<_> = knowledge.iter().map(|v| v.id.as_str()).collect();
    let relations: Vec<_> = storage::relation_facts(conn)?
        .into_iter()
        .filter(|r| ids.contains(r.from.as_str()) && ids.contains(r.to.as_str()))
        .map(|r| EvidenceRelation {
            id: r.id,
            from: r.from,
            to: r.to,
            kind: r.kind,
            evidence_id: r.evidence_id,
            assertion_revision_id: r.assertion_revision_id,
            source_id: r.source_id,
            source_revision_id: r.source_revision_id,
            source_locator: r.source_locator,
            active: r.active,
            exact_excerpt: r.exact_excerpt,
        })
        .collect();
    let mut evidence_ids: BTreeSet<_> = knowledge
        .iter()
        .flat_map(|v| v.evidence.iter().map(|e| e.id.clone()))
        .collect();
    evidence_ids.extend(relations.iter().map(|r| r.evidence_id.clone()));
    let evidence = evidence_ids
        .iter()
        .map(|id| storage::evidence_snapshot(conn, id))
        .collect::<Result<Vec<_>>>()?;
    for e in &evidence {
        ensure!(
            e.digest == util::digest(&e.excerpt),
            "stored evidence digest does not match its original excerpt: {}",
            e.id
        );
    }
    let source_ids: BTreeSet<_> = evidence.iter().map(|e| &e.source_revision_id).collect();
    let mut source_revisions = Vec::new();
    for id in source_ids {
        let mut source = conn.query_row(
            "SELECT r.source_id,s.root_id,r.observed_path,r.content_digest,EXISTS(SELECT 1 FROM source_current c WHERE c.source_revision_id=r.id AND s.removed_at IS NULL) FROM source_revisions r JOIN sources s ON s.id=r.source_id WHERE r.id=?1",
            [id],
            |r| {
                Ok(SourceRevision {
                    id: id.clone(),
                    source_id: r.get(0)?,
                    root_id: r.get(1)?,
                    observed_path: r.get(2)?,
                    content_digest: r.get(3)?,
                    current: r.get(4)?,
                })
            },
        )?;
        // A source identity can survive changed import/root provenance. Keep
        // the root captured with this revision, as the evidence endpoint does.
        source.root_id = storage::source_provenance(conn, id)?.root_id;
        source_revisions.push(source);
    }
    // Include the source inventory even when a new/removed document yielded no
    // eligible assertion. Node revisions below depend only on their own inputs.
    let revision = util::json_digest(&(
        ZOOM_SCHEMA_VERSION,
        GROUPING_VERSION,
        options,
        &knowledge,
        &evidence,
        &relations,
        &source_revisions,
        storage::source_heads(conn)?,
    ))?;
    let mut graph = KnowledgeGraph {
        schema_version: ZOOM_SCHEMA_VERSION,
        grouping_version: GROUPING_VERSION.into(),
        revision,
        root_id: stable_id("root", "project"),
        model_calls: 0,
        options: options.clone(),
        nodes: Vec::new(),
        edges: Vec::new(),
        knowledge,
        evidence,
        relations,
        source_revisions,
        report: BuildReport {
            unsupported_units: unsupported,
            work_limit: options.max_work,
            ..BuildReport::default()
        },
    };
    graph::assemble(&mut graph)?;
    validate(&graph)?;
    Ok(graph)
}

pub fn explore(conn: &Connection, options: &ExploreOptions) -> Result<ExploreResult> {
    explore_inner(conn, options, None)
}

/// Revalidate a disposable graph publication and combine its navigation with
/// the same direct retrieval signals as the nonpersistent experience.
pub fn explore_cached(
    conn: &Connection,
    options: &ExploreOptions,
    directory: &std::path::Path,
) -> Result<ExploreResult> {
    explore_inner(conn, options, Some(directory))
}

fn explore_inner(
    conn: &Connection,
    options: &ExploreOptions,
    directory: Option<&std::path::Path>,
) -> Result<ExploreResult> {
    conn.execute_batch("SAVEPOINT lore_exploration")?;
    let result = (|| {
        let graph = if let Some(directory) = directory {
            build_cached(conn, &ZoomOptions::default(), directory)?.graph
        } else {
            build(conn, &ZoomOptions::default())?
        };
        let candidates = if options.query.trim().is_empty() {
            Vec::new()
        } else {
            crate::context::retrieval::retrieve_with_report(conn, &options.query, &[])?
                .hits
                .into_iter()
                .map(|hit| hit.knowledge.id)
                .collect()
        };
        selection::select_with_candidates(&graph, options, &candidates)
    })();
    let released = conn.execute_batch("RELEASE lore_exploration");
    let result = result?;
    released?;
    Ok(result)
}

pub(crate) fn stable_id(kind: &str, value: &str) -> String {
    let digest = util::digest(format!("{GROUPING_VERSION}:{kind}:{value}"));
    format!("kv_{}", digest.trim_start_matches("blake3:"))
}

pub(crate) fn critical(view: &KnowledgeView) -> bool {
    if matches!(view.kind.as_str(), "constraint" | "decision" | "risk")
        || !view.relations.is_empty()
        || view.lifecycle == "superseded"
    {
        return true;
    }
    let text = format!(
        "{} {}",
        view.statement,
        view.evidence
            .iter()
            .map(|e| e.excerpt.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    )
    .to_lowercase();
    [
        "must ",
        "never ",
        "unless ",
        "except",
        "only if",
        "do not ",
        "cannot ",
        "contraindicat",
    ]
    .iter()
    .any(|term| text.contains(term))
}

pub(crate) fn terms(text: &str) -> BTreeSet<String> {
    const STOP: &[&str] = &[
        "a",
        "an",
        "and",
        "are",
        "as",
        "at",
        "be",
        "by",
        "can",
        "for",
        "from",
        "how",
        "in",
        "is",
        "it",
        "of",
        "on",
        "or",
        "our",
        "the",
        "this",
        "to",
        "what",
        "when",
        "where",
        "which",
        "why",
        "with",
        "you",
        "your",
        "explain",
        "project",
        "knowledge",
    ];
    text.split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|t| t.chars().count() >= 3)
        .map(str::to_lowercase)
        .filter(|t| !STOP.contains(&t.as_str()))
        .take(48)
        .collect()
}

pub(crate) fn record_map(graph: &KnowledgeGraph) -> BTreeMap<&str, &KnowledgeView> {
    graph.knowledge.iter().map(|v| (v.id.as_str(), v)).collect()
}
