//! Shared revision-bound evidence validation for the schema-5 decision engine.

use super::ContextResult;
use crate::{domain::SourceMaterial, imports, storage, util};
use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnownContext {
    pub record_id: String,
    pub basis: String,
    pub statement: String,
    pub kind: String,
    pub lifecycle: String,
    pub scope: String,
    pub evidence_ids: Vec<String>,
    pub qualifications: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskCategory {
    Destructive,
    Security,
    FinancialCorrectness,
    AcceptedConstraint,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConstraintDisposition {
    Preserved,
    NeedsVerification,
    ProposedDeviation,
}

/// A compact manifest, not another mutable source of truth. Exact primary
/// excerpts/native records remain available through `lore evidence <id>`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BriefEvidence {
    pub id: String,
    pub source: String,
    pub revision_id: String,
    pub content_hash: String,
    pub basis: String,
    /// Documentary: active assertion/relationship support. Native: present in
    /// the latest import. Neither certifies a whole-file revision or checkout.
    pub current: bool,
}

pub(crate) struct EvidenceCatalog {
    pub(crate) evidence: BTreeMap<String, BriefEvidence>,
    pub(crate) known: BTreeMap<String, KnownContext>,
    /// Includes all selected current adopted constraints/decisions, even those
    /// the model would prefer to omit from its summary.
    pub(crate) constraints: BTreeMap<String, BTreeSet<String>>,
    pub(crate) inspection_paths: BTreeSet<String>,
    pub(crate) applicable_evidence: BTreeSet<String>,
}

impl EvidenceCatalog {
    pub(crate) fn validate(conn: &Connection, selected: &ContextResult) -> Result<Self> {
        let stored: BTreeMap<_, _> = storage::views(conn)?
            .into_iter()
            .map(|record| (record.id.clone(), record))
            .collect();
        let selected_ids: BTreeSet<_> = selected.sections.items().map(|item| &item.id).collect();
        let facts = storage::relation_facts(conn)?;
        let mut active_evidence: BTreeMap<String, bool> = BTreeMap::new();
        for id in &selected_ids {
            let record = stored
                .get(*id)
                .context("selected knowledge record disappeared")?;
            for evidence in &record.evidence {
                *active_evidence.entry(evidence.id.clone()).or_default() |= evidence.active;
            }
        }
        for fact in facts
            .iter()
            .filter(|fact| selected_ids.contains(&fact.from) && selected_ids.contains(&fact.to))
        {
            *active_evidence.entry(fact.evidence_id.clone()).or_default() |= fact.active;
        }
        for relation in &selected.relations {
            ensure!(
                facts.iter().any(|fact| fact.id == relation.id
                    && fact.from == relation.from
                    && fact.to == relation.to
                    && fact.kind == relation.kind
                    && fact.evidence_id == relation.evidence_id
                    && fact.active == relation.current),
                "selected relationship changed"
            );
        }
        for relation in &selected.cross_source_relations {
            imports::relationships::validate_current_relation(conn, relation)?;
        }
        let mut catalog = Self {
            evidence: BTreeMap::new(),
            known: BTreeMap::new(),
            constraints: BTreeMap::new(),
            inspection_paths: BTreeSet::new(),
            applicable_evidence: BTreeSet::new(),
        };
        for evidence in &selected.evidence {
            let archived = storage::evidence_snapshot(conn, &evidence.id)?;
            ensure!(
                archived.digest == util::digest(&archived.excerpt)
                    && archived.excerpt == evidence.excerpt
                    && archived.source_revision_id == evidence.source_revision_id
                    && archived.material == evidence.material
                    && archived.origin == evidence.origin
                    && archived.line_start == evidence.line_start
                    && archived.line_end == evidence.line_end
                    && archived.root_path.is_some() == evidence.provenance_recorded
                    && format!("{}:{}", archived.root_id, archived.path) == evidence.source,
                "selected documentary evidence does not match its retained snapshot"
            );
            // Match the established registry contract: an unchanged section's
            // captured evidence can remain active while another section of
            // the same source advances to a newer whole-file revision.
            let current = match active_evidence.get(&evidence.id) {
                Some(active) => *active,
                // Cross-source relationship-only evidence is populated by the
                // native adapter using the captured source revision's status.
                None => conn.query_row("SELECT EXISTS(SELECT 1 FROM source_current WHERE source_id=?1 AND source_revision_id=?2)", [&archived.source_id, &archived.source_revision_id], |row| row.get(0))?,
            };
            ensure!(
                current == evidence.current,
                "selected documentary revision changed"
            );
            ensure!(
                catalog
                    .evidence
                    .insert(
                        evidence.id.clone(),
                        BriefEvidence {
                            id: evidence.id.clone(),
                            source: evidence.source.clone(),
                            revision_id: archived.source_revision_id,
                            content_hash: archived.digest,
                            basis: if evidence.material == SourceMaterial::Primary {
                                "primary_documentary"
                            } else {
                                "derived_documentary"
                            }
                            .into(),
                            current,
                        }
                    )
                    .is_none(),
                "duplicate selected evidence identity"
            );
        }
        for evidence in &selected.imported_evidence {
            let archived = imports::evidence(conn, &evidence.id)?;
            ensure!(
                archived.snapshot_id == evidence.snapshot_id
                    && archived.content_hash == evidence.content_hash
                    && archived.id == evidence.observation_id
                    && archived.current == evidence.current
                    && archived.import_id == evidence.import_id
                    && archived.origin == evidence.origin
                    && archived.source_path == evidence.source
                    && archived.format == evidence.format
                    && archived.captured_at == evidence.captured_at
                    && archived.record.native_id == evidence.native_id
                    && archived.record.observed_at == evidence.observed_at
                    && archived.record.verification == evidence.verification
                    && archived.record.evidence == evidence.locators
                    && archived.record.metadata == evidence.metadata,
                "selected native evidence does not match its retained revision"
            );
            ensure!(
                catalog
                    .evidence
                    .insert(
                        evidence.id.clone(),
                        BriefEvidence {
                            id: evidence.id.clone(),
                            source: format!("{}:{}", evidence.import_id, evidence.native_id),
                            revision_id: archived.snapshot_id,
                            content_hash: archived.content_hash,
                            basis: format!("{}_report", evidence.origin.as_str()),
                            current: evidence.current,
                        }
                    )
                    .is_none(),
                "duplicate selected evidence identity"
            );
        }
        for item in selected.sections.items() {
            let record = stored
                .get(&item.id)
                .context("selected knowledge record disappeared")?;
            ensure!(
                record.statement == item.statement
                    && record.lifecycle == item.lifecycle
                    && record.kind == item.kind
                    && record.support_state == item.support_state
                    && record.scope == item.scope
                    && !item.evidence_ids.is_empty()
                    && item
                        .evidence_ids
                        .iter()
                        .all(|id| catalog.evidence.contains_key(id)
                            && record.evidence.iter().any(|e| &e.id == id)),
                "selected knowledge changed or has unretained support"
            );
            let adopted = matches!(item.lifecycle.as_str(), "accepted" | "active")
                && matches!(item.kind.as_str(), "constraint" | "decision")
                && record
                    .evidence
                    .iter()
                    .any(|e| e.active && e.material == SourceMaterial::Primary);
            if adopted {
                catalog
                    .constraints
                    .insert(item.id.clone(), item.evidence_ids.iter().cloned().collect());
            }
            if !historical_lifecycle(&item.lifecycle) {
                catalog.applicable_evidence.extend(
                    item.evidence_ids
                        .iter()
                        .filter(|id| catalog.evidence[*id].current)
                        .cloned(),
                );
            }
            let mut qualifications = item.qualifications.clone();
            qualifications.push(format!(
                "Documented {} ({}); {}. This is source documentation, not checkout verification.",
                item.kind, item.lifecycle, item.support_state
            ));
            catalog.known.insert(
                item.id.clone(),
                KnownContext {
                    record_id: item.id.clone(),
                    basis: "documented".into(),
                    statement: item.statement.clone(),
                    kind: item.kind.clone(),
                    lifecycle: item.lifecycle.clone(),
                    scope: item.scope.clone(),
                    evidence_ids: item.evidence_ids.clone(),
                    qualifications,
                },
            );
        }
        for observation in &selected.imported_observations {
            ensure!(
                !observation.evidence_ids.is_empty(),
                "native observation has no evidence"
            );
            for id in &observation.evidence_ids {
                ensure!(
                    catalog.evidence.contains_key(id),
                    "native observation cites omitted evidence"
                );
                let archived = imports::evidence(conn, id)?;
                ensure!(
                    archived.id == observation.id
                        && archived.record.statement == observation.statement
                        && archived.record.kind == observation.kind
                        && archived.record.lifecycle == observation.lifecycle
                        && archived.record.title == observation.title
                        && archived.record.subject == observation.subject
                        && archived.record.verification == observation.verification
                        && archived.import_id == observation.import_id
                        && archived.origin == observation.origin
                        && archived.record.native_id == observation.native_id
                        && archived.record.scope == observation.scope
                        && archived.current == observation.current,
                    "selected native observation changed"
                );
            }
            // A current import, even with matching resource bytes, cannot by
            // itself support a hypothesis about the current implementation.
            if observation.current
                && !(observation.origin == crate::domain::ImportKind::Openwiki
                    && observation.kind == imports::adapters::ObservationKind::Implementation)
                && !historical_lifecycle(&observation.lifecycle)
            {
                catalog
                    .applicable_evidence
                    .extend(observation.evidence_ids.iter().cloned());
            }
            catalog.known.insert(
                observation.id.clone(),
                KnownContext {
                    record_id: observation.id.clone(),
                    basis: "observed".into(),
                    statement: observation.statement.clone(),
                    kind: serde_json::to_value(observation.kind)?
                        .as_str()
                        .unwrap_or("report")
                        .into(),
                    lifecycle: observation.lifecycle.clone(),
                    scope: serde_json::to_string(&observation.scope)?,
                    evidence_ids: observation.evidence_ids.clone(),
                    qualifications: observation.qualifications.clone(),
                },
            );
        }
        for path in &selected.suggested_inspection {
            ensure!(
                catalog.evidence.contains_key(&path.evidence_id),
                "inspection path cites omitted evidence"
            );
            if path.evidence_id.starts_with("ne_") {
                let archived = imports::evidence(conn, &path.evidence_id)?;
                ensure!(
                    path.root_id == archived.import_id
                        && archived
                            .record
                            .evidence
                            .iter()
                            .any(|locator| locator.locator == path.path),
                    "inspection path is not present in its native evidence"
                );
            } else {
                let archived = storage::evidence_snapshot(conn, &path.evidence_id)?;
                let live: Option<(String, String)> = conn.query_row(
                    "SELECT s.root_id,s.relative_path FROM sources s JOIN source_current c ON c.source_id=s.id WHERE s.id=?1 AND s.removed_at IS NULL",
                    [&archived.source_id], |row| Ok((row.get(0)?, row.get(1)?)),
                ).optional()?;
                let (root, source_path) =
                    live.unwrap_or_else(|| (archived.root_id.clone(), archived.path.clone()));
                let supported = match path.basis.as_str() {
                    "source_record" => path.root_id == root && path.path == source_path,
                    "mentioned_in_evidence" => {
                        path.root_id == archived.root_id
                            && super::mentioned_paths(&archived.excerpt).contains(&path.path)
                    }
                    _ => false,
                };
                ensure!(
                    supported,
                    "inspection path is not present in its documentary evidence"
                );
            }
            catalog.inspection_paths.insert(path.path.clone());
        }
        Ok(catalog)
    }
}

fn historical_lifecycle(lifecycle: &str) -> bool {
    matches!(
        lifecycle.to_lowercase().as_str(),
        "deleted"
            | "deprecated"
            | "superseded"
            | "withdrawn"
            | "rejected"
            | "retracted"
            | "orphaned"
    )
}

pub(crate) fn evidence_input(selected: &ContextResult, catalog: &EvidenceCatalog) -> Value {
    json!({
        "knowledge":selected.sections.items().collect::<Vec<_>>(),
        "observations":selected.imported_observations,
        "documentary_evidence":selected.evidence,
        "native_evidence":selected.imported_evidence.iter().map(|e| json!({"id":e.id,"observation_id":e.observation_id,"snapshot_id":e.snapshot_id,"content_hash":e.content_hash,"source":e.source,"origin":e.origin,"native_id":e.native_id,"current":e.current,"observed_at":e.observed_at,"locators":e.locators,"verification":e.verification})).collect::<Vec<_>>(),
        "recorded_relationships":selected.relations,
        "cross_source_relationships":selected.cross_source_relations,
        "discrepancies":selected.discrepancies,
        "reviews":selected.reviews,
        "adopted_constraints":catalog.constraints,
        "suggested_inspection":selected.suggested_inspection,
        "omissions":selected.omissions,
        "warnings":selected.warnings,
    })
}
