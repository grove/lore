//! Explicit schema 2: a lossless, normalized wire format for small budgets.
//!
//! String indexes address `strings`; source, evidence and knowledge indexes
//! address the corresponding table. Row columns are versioned here and in
//! docs/KNOWLEDGE_ZOOM.md. The resolver restores every original v1 field,
//! including historical evidence, source provenance and relationship witnesses.
use super::*;
use crate::domain::EvidenceView;
use anyhow::Context;

pub const COMPACT_ZOOM_SCHEMA_VERSION: u32 = 2;

/// id, revision, statement, topic, topic title, subject, kind, lifecycle,
/// base lifecycle, scope, effective time, support, evidence links, relations.
/// Each evidence link is (evidence index, assertion string index, active).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactKnowledge(
    pub usize,
    pub usize,
    pub usize,
    pub usize,
    pub usize,
    pub usize,
    pub usize,
    pub usize,
    pub usize,
    pub usize,
    pub usize,
    pub usize,
    pub Vec<(usize, usize, bool)>,
    pub Vec<usize>,
);

/// id, source index, root path, material, origin, excerpt, preceding context,
/// following context, digest, first line, last line, captured time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactEvidence(
    pub usize,
    pub usize,
    pub Option<usize>,
    pub usize,
    pub Option<usize>,
    pub usize,
    pub usize,
    pub usize,
    pub usize,
    pub Option<i64>,
    pub Option<i64>,
    pub usize,
);

/// id, source id, root id, observed path, content digest, current.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactSourceRevision(
    pub usize,
    pub usize,
    pub usize,
    pub usize,
    pub usize,
    pub bool,
);

/// id, from knowledge index, to knowledge index, kind, evidence index,
/// assertion revision, source locator, active. Source identity, revision and
/// exact excerpt resolve through the original evidence row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactRelation(
    pub usize,
    pub usize,
    pub usize,
    pub usize,
    pub usize,
    pub usize,
    pub usize,
    pub bool,
);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompactExploreResult {
    pub schema_version: u32,
    pub snapshot_revision: String,
    pub query: String,
    pub selected_node_id: Option<String>,
    pub retrieval: String,
    pub status: String,
    pub model_calls: u32,
    pub strings: Vec<String>,
    pub knowledge: Vec<CompactKnowledge>,
    pub evidence: Vec<CompactEvidence>,
    pub relations: Vec<CompactRelation>,
    pub source_revisions: Vec<CompactSourceRevision>,
    pub nodes: Vec<NavigationNode>,
    pub edges: Vec<ViewEdge>,
    pub coverage: Coverage,
    pub critical_groups_omitted: usize,
    pub navigation_truncated: bool,
    pub max_tokens: usize,
    pub used_tokens: usize,
    pub warnings: Vec<String>,
}

#[derive(Default)]
struct Strings {
    values: Vec<String>,
    indexes: BTreeMap<String, usize>,
}

impl Strings {
    fn add(&mut self, value: &str) -> usize {
        if let Some(index) = self.indexes.get(value) {
            return *index;
        }
        let index = self.values.len();
        self.values.push(value.to_owned());
        self.indexes.insert(value.to_owned(), index);
        index
    }
    fn optional(&mut self, value: &Option<String>) -> Option<usize> {
        value.as_deref().map(|value| self.add(value))
    }
}

fn evidence_view(
    snapshot: &storage::EvidenceSnapshot,
    assertion: String,
    active: bool,
) -> EvidenceView {
    EvidenceView {
        id: snapshot.id.clone(),
        assertion_id: assertion,
        source_id: snapshot.source_id.clone(),
        source: format!("{}:{}", snapshot.root_id, snapshot.path),
        root_id: snapshot.root_id.clone(),
        path: snapshot.path.clone(),
        source_revision_id: snapshot.source_revision_id.clone(),
        root_path: snapshot.root_path.clone(),
        material: snapshot.material,
        origin: snapshot.origin.clone(),
        line_start: snapshot.line_start,
        line_end: snapshot.line_end,
        excerpt: snapshot.excerpt.clone(),
        captured_at: snapshot.captured_at.clone(),
        active,
    }
}

impl CompactExploreResult {
    pub(super) fn from_expanded(result: &ExploreResult) -> Result<Self> {
        let mut strings = Strings::default();
        let source_indexes: BTreeMap<_, _> = result
            .source_revisions
            .iter()
            .enumerate()
            .map(|(i, source)| (source.id.as_str(), i))
            .collect();
        let evidence_indexes: BTreeMap<_, _> = result
            .evidence
            .iter()
            .enumerate()
            .map(|(i, evidence)| (evidence.id.as_str(), i))
            .collect();
        let knowledge_indexes: BTreeMap<_, _> = result
            .knowledge
            .iter()
            .enumerate()
            .map(|(i, record)| (record.id.as_str(), i))
            .collect();
        let mut source_revisions = Vec::new();
        for source in &result.source_revisions {
            source_revisions.push(CompactSourceRevision(
                strings.add(&source.id),
                strings.add(&source.source_id),
                strings.add(&source.root_id),
                strings.add(&source.observed_path),
                strings.add(&source.content_digest),
                source.current,
            ));
        }
        let mut evidence = Vec::new();
        for snapshot in &result.evidence {
            let source = *source_indexes
                .get(snapshot.source_revision_id.as_str())
                .context("compact evidence has no source revision")?;
            let manifest = &result.source_revisions[source];
            ensure!(
                manifest.source_id == snapshot.source_id
                    && manifest.root_id == snapshot.root_id
                    && manifest.observed_path == snapshot.path,
                "compact evidence source metadata cannot resolve exactly"
            );
            evidence.push(CompactEvidence(
                strings.add(&snapshot.id),
                source,
                strings.optional(&snapshot.root_path),
                strings.add(snapshot.material.as_str()),
                strings.optional(&snapshot.origin),
                strings.add(&snapshot.excerpt),
                strings.add(&snapshot.context_before),
                strings.add(&snapshot.context_after),
                strings.add(&snapshot.digest),
                snapshot.line_start,
                snapshot.line_end,
                strings.add(&snapshot.captured_at),
            ));
        }
        let mut knowledge = Vec::new();
        for record in &result.knowledge {
            let mut links = Vec::new();
            for citation in &record.evidence {
                let index = *evidence_indexes
                    .get(citation.id.as_str())
                    .context("compact knowledge has no original evidence")?;
                let resolved = evidence_view(
                    &result.evidence[index],
                    citation.assertion_id.clone(),
                    citation.active,
                );
                ensure!(
                    serde_json::to_value(&resolved)? == serde_json::to_value(citation)?,
                    "compact evidence reference cannot resolve every original field"
                );
                links.push((index, strings.add(&citation.assertion_id), citation.active));
            }
            knowledge.push(CompactKnowledge(
                strings.add(&record.id),
                strings.add(&record.revision_id),
                strings.add(&record.statement),
                strings.add(&record.topic),
                strings.add(&record.topic_title),
                strings.add(&record.subject),
                strings.add(&record.kind),
                strings.add(&record.lifecycle),
                strings.add(&record.base_lifecycle),
                strings.add(&record.scope),
                strings.add(&record.effective_at),
                strings.add(&record.support_state),
                links,
                record.relations.iter().map(|s| strings.add(s)).collect(),
            ));
        }
        let mut relations = Vec::new();
        for relation in &result.relations {
            let witness = *evidence_indexes
                .get(relation.evidence_id.as_str())
                .context("compact relation has no witness")?;
            let snapshot = &result.evidence[witness];
            ensure!(
                relation.source_id == snapshot.source_id
                    && relation.source_revision_id == snapshot.source_revision_id
                    && relation.exact_excerpt == snapshot.excerpt,
                "compact relationship witness cannot resolve exactly"
            );
            relations.push(CompactRelation(
                strings.add(&relation.id),
                *knowledge_indexes
                    .get(relation.from.as_str())
                    .context("compact relationship from endpoint is missing")?,
                *knowledge_indexes
                    .get(relation.to.as_str())
                    .context("compact relationship to endpoint is missing")?,
                strings.add(&relation.kind),
                witness,
                strings.add(&relation.assertion_revision_id),
                strings.add(&relation.source_locator),
                relation.active,
            ));
        }
        Ok(Self {
            schema_version: COMPACT_ZOOM_SCHEMA_VERSION,
            snapshot_revision: result.snapshot_revision.clone(),
            query: result.query.clone(),
            selected_node_id: result.selected_node_id.clone(),
            retrieval: result.retrieval.clone(),
            status: result.status.clone(),
            model_calls: result.model_calls,
            strings: strings.values,
            knowledge,
            evidence,
            relations,
            source_revisions,
            nodes: result.nodes.clone(),
            edges: result.edges.clone(),
            coverage: result.coverage.clone(),
            critical_groups_omitted: result.critical_groups_omitted,
            navigation_truncated: result.navigation_truncated,
            max_tokens: result.max_tokens,
            used_tokens: result.used_tokens,
            warnings: result.warnings.clone(),
        })
    }

    /// Resolve the portable tables without a database or cache. `schema_version`
    /// remains 2: `used_tokens` describes the compact wire response, not this
    /// expanded in-memory representation. An independent registry comparison is
    /// still required to establish authenticity of supplied response bytes.
    pub fn resolve(&self) -> Result<ExploreResult> {
        ensure!(
            self.schema_version == COMPACT_ZOOM_SCHEMA_VERSION,
            "unsupported compact exploration schema"
        );
        let text = |index: usize| {
            self.strings
                .get(index)
                .cloned()
                .context("dangling compact string reference")
        };
        let optional = |index: Option<usize>| index.map(&text).transpose();
        let mut source_revisions = Vec::new();
        let mut source_ids = BTreeSet::new();
        for row in &self.source_revisions {
            let id = text(row.0)?;
            ensure!(
                source_ids.insert(id.clone()),
                "duplicate compact source revision"
            );
            source_revisions.push(SourceRevision {
                id,
                source_id: text(row.1)?,
                root_id: text(row.2)?,
                observed_path: text(row.3)?,
                content_digest: text(row.4)?,
                current: row.5,
            });
        }
        let mut evidence = Vec::new();
        let mut evidence_ids = BTreeSet::new();
        for row in &self.evidence {
            let source = source_revisions
                .get(row.1)
                .context("dangling compact source revision reference")?;
            let id = text(row.0)?;
            ensure!(
                evidence_ids.insert(id.clone()),
                "duplicate compact evidence"
            );
            let excerpt = text(row.5)?;
            let digest = text(row.8)?;
            ensure!(
                util::digest(&excerpt) == digest,
                "compact evidence digest mismatch"
            );
            evidence.push(storage::EvidenceSnapshot {
                id,
                source_id: source.source_id.clone(),
                source_revision_id: source.id.clone(),
                root_id: source.root_id.clone(),
                path: source.observed_path.clone(),
                root_path: optional(row.2)?,
                material: serde_json::from_value(serde_json::Value::String(text(row.3)?))?,
                origin: optional(row.4)?,
                excerpt,
                context_before: text(row.6)?,
                context_after: text(row.7)?,
                digest,
                line_start: row.9,
                line_end: row.10,
                captured_at: text(row.11)?,
            });
        }
        let mut knowledge = Vec::new();
        let mut knowledge_ids = BTreeSet::new();
        for row in &self.knowledge {
            let id = text(row.0)?;
            ensure!(
                knowledge_ids.insert(id.clone()),
                "duplicate compact knowledge"
            );
            let mut citations = Vec::new();
            for (index, assertion, active) in &row.12 {
                citations.push(evidence_view(
                    evidence
                        .get(*index)
                        .context("dangling compact evidence reference")?,
                    text(*assertion)?,
                    *active,
                ));
            }
            ensure!(!citations.is_empty(), "compact knowledge has no evidence");
            knowledge.push(KnowledgeView {
                id,
                revision_id: text(row.1)?,
                statement: text(row.2)?,
                topic: text(row.3)?,
                topic_title: text(row.4)?,
                subject: text(row.5)?,
                kind: text(row.6)?,
                lifecycle: text(row.7)?,
                base_lifecycle: text(row.8)?,
                scope: text(row.9)?,
                effective_at: text(row.10)?,
                support_state: text(row.11)?,
                evidence: citations,
                relations: row
                    .13
                    .iter()
                    .map(|index| text(*index))
                    .collect::<Result<_>>()?,
            });
        }
        let mut relations = Vec::new();
        let mut relation_ids = BTreeSet::new();
        for row in &self.relations {
            let snapshot = evidence
                .get(row.4)
                .context("dangling compact relation witness")?;
            let id = text(row.0)?;
            ensure!(
                relation_ids.insert(id.clone()),
                "duplicate compact relationship"
            );
            relations.push(EvidenceRelation {
                id,
                from: knowledge
                    .get(row.1)
                    .context("dangling compact from endpoint")?
                    .id
                    .clone(),
                to: knowledge
                    .get(row.2)
                    .context("dangling compact to endpoint")?
                    .id
                    .clone(),
                kind: text(row.3)?,
                evidence_id: snapshot.id.clone(),
                assertion_revision_id: text(row.5)?,
                source_id: snapshot.source_id.clone(),
                source_revision_id: snapshot.source_revision_id.clone(),
                source_locator: text(row.6)?,
                active: row.7,
                exact_excerpt: snapshot.excerpt.clone(),
            });
        }
        let used_evidence: BTreeSet<_> = knowledge
            .iter()
            .flat_map(|record| record.evidence.iter().map(|citation| &citation.id))
            .chain(relations.iter().map(|relation| &relation.evidence_id))
            .collect();
        ensure!(
            used_evidence.len() == evidence.len(),
            "extra compact evidence manifest entry"
        );
        let used_sources: BTreeSet<_> = evidence.iter().map(|e| &e.source_revision_id).collect();
        ensure!(
            used_sources.len() == source_revisions.len(),
            "extra compact source manifest entry"
        );
        ensure!(
            self.coverage.included_units == knowledge.len()
                && self.coverage.eligible_units
                    == self.coverage.included_units + self.coverage.omitted_units,
            "inconsistent compact coverage"
        );
        Ok(ExploreResult {
            schema_version: COMPACT_ZOOM_SCHEMA_VERSION,
            snapshot_revision: self.snapshot_revision.clone(),
            query: self.query.clone(),
            selected_node_id: self.selected_node_id.clone(),
            retrieval: self.retrieval.clone(),
            status: self.status.clone(),
            model_calls: self.model_calls,
            nodes: self.nodes.clone(),
            edges: self.edges.clone(),
            knowledge,
            evidence,
            relations,
            source_revisions,
            coverage: self.coverage.clone(),
            critical_groups_omitted: self.critical_groups_omitted,
            navigation_truncated: self.navigation_truncated,
            max_tokens: self.max_tokens,
            used_tokens: self.used_tokens,
            warnings: self.warnings.clone(),
        })
    }
}

/// The same complete source-bound facts and original evidence as the compact
/// JSON, presented for people. This never renders the expanded JSON as a hidden
/// uncharged payload or depends on an external reference service.
pub fn render_compact_markdown(result: &CompactExploreResult) -> Result<String> {
    Ok(selection::render_markdown(&result.resolve()?))
}
