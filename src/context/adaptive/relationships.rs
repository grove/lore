//! Source-owned relationship groups retained outside optional decision prose.
//! The manifest is a bounded projection of one selected read snapshot, not a
//! second registry. Original context/evidence types retain native vocabulary.

use crate::{
    context::{
        self, ContextBudget, ContextEvidence, ContextItem, ContextOmissions, ContextResult,
        ContextSections,
        imports::{ContextDiscrepancy, ContextImportedEvidence, ContextObservation},
        intelligence::EvidenceCatalog,
    },
    imports::{self, relationships::CrossSourceRelation},
    reviews, storage,
};
use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

const MAX_RELATIONS: usize = 128;
const MAX_ENDPOINTS: usize = 256;
const MAX_EVIDENCE: usize = 1_024;
const MAX_BYTES: usize = 512 * 1_024;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SourceRelationships {
    pub relations: Vec<CrossSourceRelation>,
    /// Review disposition and its qualifications do not establish source
    /// agreement or independent verification of an upstream report.
    pub discrepancies: Vec<ContextDiscrepancy>,
    pub knowledge: Vec<ContextItem>,
    /// Knowledge revision identities are distinct from source revisions.
    pub knowledge_revisions: BTreeMap<String, String>,
    pub observations: Vec<ContextObservation>,
    pub evidence: Vec<ContextEvidence>,
    pub imported_evidence: Vec<ContextImportedEvidence>,
}

impl SourceRelationships {
    pub fn is_empty(&self) -> bool {
        self.relations.is_empty()
            && self.discrepancies.is_empty()
            && self.knowledge.is_empty()
            && self.knowledge_revisions.is_empty()
            && self.observations.is_empty()
            && self.evidence.is_empty()
            && self.imported_evidence.is_empty()
    }

    pub(super) fn capture(conn: &Connection, selected: &ContextResult) -> Result<Self> {
        if selected.cross_source_relations.is_empty() {
            return Ok(Self::default());
        }
        ensure!(
            selected.cross_source_relations.len() <= MAX_RELATIONS,
            "source relationship selection exceeds its complete-group limit"
        );
        let mut result = Self {
            relations: selected.cross_source_relations.clone(),
            ..Self::default()
        };
        let relation_ids: BTreeSet<_> = result.relations.iter().map(|r| &r.id).collect();
        result.discrepancies = selected
            .discrepancies
            .iter()
            .filter(|d| relation_ids.contains(&d.id))
            .cloned()
            .collect();
        let mut native_ids = BTreeSet::new();
        for endpoint in result.relations.iter().flat_map(|r| r.endpoints()) {
            match endpoint.kind.as_str() {
                "knowledge" => {
                    if let Some(revision) = result
                        .knowledge_revisions
                        .insert(endpoint.id.clone(), endpoint.revision_id.clone())
                    {
                        ensure!(
                            revision == endpoint.revision_id,
                            "conflicting endpoint revisions"
                        );
                    }
                }
                "observation" => {
                    native_ids.insert(&endpoint.id);
                }
                _ => anyhow::bail!("invalid source relationship endpoint kind"),
            }
        }
        result.knowledge = selected
            .sections
            .items()
            .filter(|item| result.knowledge_revisions.contains_key(&item.id))
            .cloned()
            .collect();
        result.observations = selected
            .imported_observations
            .iter()
            .filter(|item| native_ids.contains(&item.id))
            .cloned()
            .collect();
        let evidence_ids = result.referenced_evidence();
        result.evidence = selected
            .evidence
            .iter()
            .filter(|item| evidence_ids.contains(&item.id))
            .cloned()
            .collect();
        result.imported_evidence = selected
            .imported_evidence
            .iter()
            .filter(|item| evidence_ids.contains(&item.id))
            .cloned()
            .collect();
        result.validate(conn)?;
        Ok(result)
    }

    fn referenced_evidence(&self) -> BTreeSet<String> {
        self.relations
            .iter()
            .flat_map(|r| &r.evidence_ids)
            .chain(self.discrepancies.iter().flat_map(|d| &d.evidence_ids))
            .chain(self.knowledge.iter().flat_map(|k| &k.evidence_ids))
            .chain(self.observations.iter().flat_map(|o| &o.evidence_ids))
            .cloned()
            .collect()
    }

    /// Recheck a shared result before a human adapter uses its source IDs.
    /// A matching registry digest alone does not authenticate caller-supplied
    /// relation reasons, qualifications, endpoint fields or evidence metadata.
    pub(crate) fn validate(&self, conn: &Connection) -> Result<EvidenceCatalog> {
        if self.is_empty() {
            // All seven source collections are empty, so there is nothing to
            // authenticate. Avoid the normal validator's registry-wide reads
            // on this common no-relationship path.
            return Ok(EvidenceCatalog {
                evidence: BTreeMap::new(),
                known: BTreeMap::new(),
                constraints: BTreeMap::new(),
                inspection_paths: BTreeSet::new(),
                applicable_evidence: BTreeSet::new(),
            });
        }
        ensure!(
            self.relations.len() <= MAX_RELATIONS
                && self.discrepancies.len() <= MAX_RELATIONS
                && self.knowledge.len() + self.observations.len() <= MAX_ENDPOINTS
                && self.evidence.len() + self.imported_evidence.len() <= MAX_EVIDENCE
                && serde_json::to_vec(self)?.len() <= MAX_BYTES,
            "source relationship manifest exceeds its bounded complete-source limits"
        );
        let relation_ids: BTreeSet<_> = self.relations.iter().map(|r| &r.id).collect();
        ensure!(
            relation_ids.len() == self.relations.len(),
            "duplicate source relationship"
        );
        let mut knowledge_ids = BTreeSet::new();
        let mut native_ids = BTreeSet::new();
        for relation in &self.relations {
            let payload: Option<String> = conn.query_row(
                "SELECT CASE WHEN length(CAST(r.payload_json AS BLOB))<=?2 THEN r.payload_json END FROM cross_source_relations r JOIN cross_source_current c ON c.input_signature=r.input_signature WHERE r.id=?1",
                rusqlite::params![relation.id, MAX_BYTES],
                |row| row.get(0),
            ).optional()?.flatten();
            let mut original: CrossSourceRelation = serde_json::from_str(
                &payload.context("source relationship is missing, historical or oversized")?,
            )?;
            original.active = true;
            ensure!(
                original == *relation,
                "source-owned relationship fields changed"
            );
            imports::relationships::validate_current_relation(conn, relation)?;
            for endpoint in relation.endpoints() {
                if endpoint.kind == "knowledge" {
                    knowledge_ids.insert(endpoint.id.clone());
                    ensure!(
                        self.knowledge_revisions.get(&endpoint.id) == Some(&endpoint.revision_id),
                        "source relationship knowledge revision is not retained"
                    );
                } else {
                    native_ids.insert(endpoint.id.clone());
                    ensure!(
                        self.imported_evidence.iter().any(|e| {
                            e.observation_id == endpoint.id && e.snapshot_id == endpoint.revision_id
                        }),
                        "source relationship native endpoint revision is not retained"
                    );
                }
            }
        }
        let retained_knowledge: BTreeSet<_> = self.knowledge.iter().map(|k| k.id.clone()).collect();
        let retained_native: BTreeSet<_> = self.observations.iter().map(|o| o.id.clone()).collect();
        ensure!(
            retained_knowledge == knowledge_ids
                && retained_knowledge.len() == self.knowledge.len()
                && self
                    .knowledge_revisions
                    .keys()
                    .cloned()
                    .collect::<BTreeSet<_>>()
                    == knowledge_ids
                && retained_native == native_ids
                && retained_native.len() == self.observations.len(),
            "source relationship endpoint group is incomplete or contains unrelated records"
        );
        let retained_evidence: BTreeSet<_> = self
            .evidence
            .iter()
            .map(|e| e.id.clone())
            .chain(self.imported_evidence.iter().map(|e| e.id.clone()))
            .collect();
        ensure!(
            retained_evidence == self.referenced_evidence()
                && retained_evidence.len() == self.evidence.len() + self.imported_evidence.len(),
            "source relationship evidence group is incomplete or duplicated"
        );
        let review_items = reviews::list(conn, true)?;
        let mut reviewed = BTreeSet::new();
        for review in review_items
            .iter()
            .filter(|review| review.status == "pending")
        {
            if let Some(id) = &review.target_unit_id {
                reviewed.insert(id.clone());
            }
            if let Some(assertion) = &review.assertion_revision_id
                && let Some(id) = storage::assigned(conn, assertion)?
            {
                reviewed.insert(id);
            }
        }
        let original_knowledge = storage::views(conn)?
            .into_iter()
            .map(|item| (item.id.clone(), item))
            .collect::<BTreeMap<_, _>>();
        let facts = storage::relation_facts(conn)?;
        for item in &self.knowledge {
            let original = original_knowledge
                .get(&item.id)
                .context("relationship endpoint disappeared")?;
            ensure!(
                self.knowledge_revisions.get(&item.id) == Some(&original.revision_id),
                "relationship knowledge revision changed"
            );
            let edges = facts
                .iter()
                .filter(|f| f.from == item.id || f.to == item.id)
                .collect::<Vec<_>>();
            let (expected, _) = context::context_item(
                original,
                &edges,
                reviewed.contains(&item.id),
                &item.relevance,
            );
            ensure!(
                serde_json::to_value(item)? == serde_json::to_value(expected)?,
                "relationship endpoint text or qualifications changed"
            );
        }
        for item in &self.observations {
            ensure!(
                item.evidence_ids.len() == 1,
                "native relationship endpoint has incomplete evidence"
            );
            let original = imports::evidence(conn, &item.evidence_ids[0])?;
            let expected = context::imports::observation(&original, &item.relevance);
            ensure!(
                serde_json::to_value(item)? == serde_json::to_value(expected)?,
                "relationship native status, scope or qualifications changed"
            );
        }
        let discrepancies = self
            .discrepancies
            .iter()
            .map(|d| (&d.id, d))
            .collect::<BTreeMap<_, _>>();
        ensure!(
            discrepancies.len() == self.discrepancies.len()
                && discrepancies.len() == self.relations.iter().filter(|r| r.is_question()).count(),
            "source relationship review disposition is incomplete or duplicated"
        );
        for relation in self.relations.iter().filter(|r| r.is_question()) {
            let review = relation
                .review_id
                .as_ref()
                .and_then(|id| review_items.iter().find(|r| r.id == *id));
            let expected = context::imports::discrepancy(relation, review);
            let retained = discrepancies
                .get(&relation.id)
                .context("source relationship review disposition is missing")?;
            ensure!(
                serde_json::to_value(retained)? == serde_json::to_value(&expected)?,
                "source relationship review status or qualifications changed"
            );
        }
        EvidenceCatalog::validate(conn, &self.context())
    }

    /// Manifest-internal validity cannot detect deletion of the whole field.
    /// Check the registry's current incident relationships for every source
    /// record actually retained by a presentation, including evidence-only
    /// premises whose optional schema-4 fact was removed during packing.
    pub(crate) fn validate_retained(
        &self,
        conn: &Connection,
        records: &BTreeSet<String>,
    ) -> Result<()> {
        ensure!(
            records.len() <= MAX_EVIDENCE,
            "too many retained relationship premises"
        );
        if records.is_empty() {
            ensure!(
                self.is_empty(),
                "relationship manifest has no retained premises"
            );
            return Ok(());
        }
        let available: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='cross_source_evaluations')",
            [], |row| row.get(0),
        )?;
        if !available {
            ensure!(self.is_empty(), "relationship registry is unavailable");
            return Ok(());
        }
        let parameters = (1..=records.len())
            .map(|n| format!("?{n}"))
            .collect::<Vec<_>>()
            .join(",");
        let mut query = conn.prepare(&format!(
            "SELECT DISTINCT r.id FROM cross_source_relations r JOIN cross_source_current c ON c.input_signature=r.input_signature JOIN cross_source_evaluations e ON e.input_signature=r.input_signature WHERE e.from_id IN ({parameters}) OR e.to_id IN ({parameters}) ORDER BY r.id LIMIT {}",
            MAX_RELATIONS + 1,
        ))?;
        let expected = query
            .query_map(rusqlite::params_from_iter(records.iter()), |row| {
                row.get::<_, String>(0)
            })?
            .collect::<rusqlite::Result<BTreeSet<_>>>()?;
        ensure!(
            expected.len() <= MAX_RELATIONS
                && expected
                    == self
                        .relations
                        .iter()
                        .map(|r| r.id.clone())
                        .collect::<BTreeSet<_>>(),
            "retained source records have an omitted source relationship group; rebuild shared intelligence"
        );
        Ok(())
    }

    // Reuse the established evidence validator and native renderer with a
    // transient endpoint-only adapter. This ContextResult is never published,
    // cached, or stored alongside the shared response.
    fn context(&self) -> ContextResult {
        ContextResult {
            schema_version: context::CONTEXT_SCHEMA_VERSION,
            task: String::new(),
            paths: Vec::new(),
            empty: self.is_empty(),
            model_calls: 0,
            retrieval_truncated: false,
            budget: ContextBudget {
                max_tokens: 0,
                used_tokens: 0,
                tokenizer: "cl100k_base".into(),
            },
            sections: ContextSections {
                relevant_knowledge: self.knowledge.clone(),
                ..Default::default()
            },
            relations: Vec::new(),
            reviews: Vec::new(),
            evidence: self.evidence.clone(),
            suggested_inspection: Vec::new(),
            omissions: ContextOmissions::default(),
            warnings: Vec::new(),
            imported_observations: self.observations.clone(),
            imported_evidence: self.imported_evidence.clone(),
            cross_source_relations: self.relations.clone(),
            discrepancies: self.discrepancies.clone(),
            recommended_verification: Vec::new(),
        }
    }

    pub(crate) fn token_overhead(&self) -> Result<usize> {
        if self.is_empty() {
            return Ok(0);
        }
        Ok(context::count_tokens(&serde_json::to_string(self)?)
            .max(context::count_tokens(&self.render()))
            + 32)
    }

    pub fn render(&self) -> String {
        if self.is_empty() {
            return String::new();
        }
        let text = context::display_text;
        let mut output = String::from(
            "\n## Source-owned relationships\n\nThese are retained source interpretations. Current import membership, upstream status and review disposition do not verify current implementation or policy adoption.\n\n",
        );
        for relation in &self.relations {
            output.push_str(&format!(
                "- [{}] {}: {}\n",
                text(&relation.id),
                text(&relation.kind),
                text(&relation.reason)
            ));
            for endpoint in relation.endpoints() {
                output.push_str(&format!(
                    "  Endpoint: {} `{}` at revision `{}`.\n",
                    text(&endpoint.kind),
                    text(&endpoint.id),
                    text(&endpoint.revision_id)
                ));
            }
            output.push_str(&format!(
                "  Current retained interpretation: {}. Evidence: {}\n",
                relation.active,
                relation
                    .evidence_ids
                    .iter()
                    .map(|id| text(id))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            for qualification in &relation.qualifications {
                output.push_str(&format!("  {}\n", text(qualification)));
            }
            if let Some(kind) = &relation.upstream_kind {
                output.push_str(&format!("  Upstream relationship: {}.\n", text(kind)));
            }
            if let Some(status) = &relation.upstream_status {
                output.push_str(&format!("  Upstream status: {}.\n", text(status)));
            }
            if let Some(active) = relation.upstream_active {
                output.push_str(&format!("  Asserted upstream: {active}.\n"));
            }
            if let Some(id) = &relation.review_id {
                output.push_str(&format!("  Review: {}.\n", text(id)));
            }
        }
        if !self.knowledge.is_empty() {
            output.push_str("\n### Documentary relationship endpoints\n\n");
        }
        for item in &self.knowledge {
            output.push_str(&format!(
                "- [{}] {}\n  {}, {}; {}; scope: {}.\n",
                text(&item.id),
                text(&item.statement),
                text(&item.kind),
                text(&item.lifecycle),
                text(&item.support_state),
                text(&item.scope)
            ));
            if !item.effective_at.is_empty() {
                output.push_str(&format!("  Effective: {}.\n", text(&item.effective_at)));
            }
            for qualification in &item.qualifications {
                output.push_str(&format!("  {}\n", text(qualification)));
            }
            output.push_str(&format!(
                "  Evidence: {}\n",
                item.evidence_ids
                    .iter()
                    .map(|id| text(id))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        let mut context = self.context();
        context.cross_source_relations.clear();
        context::imports::render(&context, &mut output);
        if !self.evidence.is_empty() {
            output.push_str("\n### Documentary relationship evidence\n\n");
        }
        for evidence in &self.evidence {
            output.push_str(&format!("\n### Source {}\n\n", text(&evidence.id)));
            output.push_str(&format!("- [{}] {} at source revision `{}`; {} material; current documentary support: {}.\n  {}\n", text(&evidence.id), text(&evidence.source), text(&evidence.source_revision_id), evidence.material.as_str(), evidence.current, text(&evidence.excerpt)));
            if let Some(origin) = &evidence.origin {
                output.push_str(&format!("  Source origin: {}.\n", text(origin)));
            }
            if let (Some(start), Some(end)) = (evidence.line_start, evidence.line_end) {
                output.push_str(&format!("  Captured lines: {start}–{end}.\n"));
            }
        }
        for evidence in &self.imported_evidence {
            output.push_str(&format!("\n### Source {}\n\n{}:{} at native snapshot `{}`; content hash `{}`. These are imported source records, not independent checkout verification.\n", text(&evidence.id), text(&evidence.import_id), text(&evidence.native_id), text(&evidence.snapshot_id), text(&evidence.content_hash)));
        }
        output.push_str(
            "\nResolve complete immutable snapshots with `lore evidence <evidence-id>`.\n",
        );
        output
    }
}
