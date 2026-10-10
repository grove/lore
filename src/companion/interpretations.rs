//! Read-only comparison of immutable cross-source interpretation revisions.
//! Checkpoint membership selects history; retained source revisions supply it.

use super::{
    Baseline, ChangeReport, ImportedRevision, MAX_BASELINE_BYTES, RevisionState, display, quote,
    render_revision,
};
use crate::{
    imports::{
        self, ImportedObservation,
        relationships::{CrossSourceEndpoint, CrossSourceRelation},
    },
    storage, util,
};
use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

const MAX_HISTORY_SOURCES: usize = 20_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterpretationState {
    /// Original payload. `active` is current membership at report time, as in
    /// the existing history reader; checkpoint presence is explicit below.
    pub relation: CrossSourceRelation,
    pub present_at_checkpoint: bool,
    /// Resolve by revision ID in the report's cross_source_knowledge, not by a
    /// current knowledge ID that might now name a different source statement.
    pub knowledge_revision_ids: Vec<String>,
    pub imported: Vec<ImportedRevision>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrentInterpretationReview {
    pub review_id: String,
    /// Current report-snapshot status only. None means the row is unavailable.
    pub status: Option<String>,
    pub qualification: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterpretationChange {
    /// The immutable evaluated comparison key, never inferred from prose.
    pub pair_key: String,
    pub change_kind: String,
    pub before: Option<InterpretationState>,
    pub after: Option<InterpretationState>,
    pub baseline_review_status: String,
    pub current_reviews: Vec<CurrentInterpretationReview>,
    pub qualification: String,
}

pub(super) fn content_hash(relation: &CrossSourceRelation) -> Result<String> {
    let mut payload = relation.clone();
    // `active` is the current-pointer projection, not immutable source text.
    payload.active = false;
    util::json_digest(&payload)
}

#[derive(Default)]
pub(super) struct InterpretationHistory {
    pub changes: Vec<InterpretationChange>,
    knowledge: BTreeMap<String, RevisionState>,
    evidence: BTreeMap<String, storage::EvidenceSnapshot>,
    imported: BTreeMap<String, ImportedObservation>,
    source_bytes: usize,
}

impl InterpretationHistory {
    pub fn read(
        conn: &Connection,
        baseline: &Baseline,
        current: &BTreeMap<String, String>,
        current_imported: &BTreeMap<String, ImportedRevision>,
        relevant: Option<&BTreeSet<String>>,
        relevant_imported: Option<&BTreeSet<String>>,
    ) -> Result<Self> {
        if baseline.cross_source_relationships == *current {
            return Ok(Self::default());
        }
        // The caller preflights the existing 20,000-record/8-MB payload bound.
        let originals: BTreeMap<_, _> = imports::relationships::history(conn)?
            .into_iter()
            .map(|relation| (relation.id.clone(), relation))
            .collect();
        let mut before = BTreeMap::new();
        let mut after = BTreeMap::new();
        for (manifest, grouped) in [
            (&baseline.cross_source_relationships, &mut before),
            (current, &mut after),
        ] {
            for (id, digest) in manifest {
                let relation = originals
                    .get(id)
                    .context("interpretation history is unavailable")?;
                ensure!(
                    content_hash(relation)? == *digest,
                    "checkpoint interpretation differs from its immutable original payload"
                );
                let key = evaluated_key(conn, relation)?;
                ensure!(
                    grouped.insert(key, relation).is_none(),
                    "checkpoint contains multiple interpretations of one comparison"
                );
            }
        }
        let mut history = Self::default();
        for key in before.keys().chain(after.keys()).collect::<BTreeSet<_>>() {
            let old = before.get(key).copied();
            let new = after.get(key).copied();
            if old.map(|r| &r.id) == new.map(|r| &r.id)
                || !old
                    .into_iter()
                    .chain(new)
                    .any(|relation| relevant_to(relation, relevant, relevant_imported))
            {
                continue;
            }
            let old_state = old
                .map(|relation| history.state(conn, key, relation, &baseline.imported))
                .transpose()?;
            let new_state = new
                .map(|relation| history.state(conn, key, relation, current_imported))
                .transpose()?;
            let mut current_reviews = Vec::new();
            for id in old
                .into_iter()
                .chain(new)
                .filter_map(|relation| relation.review_id.as_ref())
                .collect::<BTreeSet<_>>()
            {
                let stored: Option<(Option<String>, usize)> = conn
                    .query_row("SELECT CASE WHEN length(CAST(status AS BLOB))<=512 THEN status END,length(CAST(status AS BLOB)) FROM review_items WHERE id=?1", [id], |row| {
                        Ok((row.get(0)?, row.get(1)?))
                    })
                    .optional()?;
                ensure!(
                    stored.as_ref().is_none_or(|(_, bytes)| *bytes <= 512),
                    "current interpretation review status exceeds its byte limit"
                );
                current_reviews.push(CurrentInterpretationReview {
                    review_id: id.clone(),
                    status: stored.and_then(|(status, _)| status),
                    qualification: "Current review status at the report snapshot only. The baseline did not capture review disposition. A resolved or dismissed review does not independently verify implementation or establish agreement between sources.".into(),
                });
            }
            history.changes.push(InterpretationChange {
                pair_key: key.clone(),
                change_kind: match (old, new) {
                    (None, Some(_)) => "interpretation_added",
                    (Some(_), None) => "interpretation_withdrawn",
                    _ => "interpretation_revised",
                }.into(),
                baseline_review_status: if old.is_some_and(|r| r.review_id.is_some()) {
                    "not_captured"
                } else {
                    "not_applicable"
                }.into(),
                before: old_state,
                after: new_state,
                current_reviews,
                qualification: "A change in retained source interpretation membership or inputs. Reasons, qualifications and upstream status retain their source meanings. Withdrawal does not reverse a source claim, replace accepted policy, resolve a review, or verify runtime behavior.".into(),
            });
        }
        Ok(history)
    }

    fn charge(&mut self, bytes: usize) -> Result<()> {
        self.source_bytes = self.source_bytes.saturating_add(bytes);
        ensure!(
            self.source_bytes <= MAX_BASELINE_BYTES,
            "complete interpretation history endpoint evidence exceeds its byte limit"
        );
        Ok(())
    }

    fn documentary(&mut self, conn: &Connection, id: &str) -> Result<()> {
        if self.evidence.contains_key(id) {
            return Ok(());
        }
        ensure!(
            self.evidence.len() + self.imported.len() < MAX_HISTORY_SOURCES,
            "interpretation history exceeds its complete evidence count limit"
        );
        let bytes = conn.query_row(
            "SELECT length(CAST(exact_excerpt AS BLOB))+length(CAST(context_before AS BLOB))+length(CAST(context_after AS BLOB)) FROM evidence_snapshots WHERE id=?1",
            [id], |row| row.get(0),
        )?;
        self.charge(bytes)?;
        let evidence = storage::evidence_snapshot(conn, id)?;
        ensure!(
            !evidence.excerpt.is_empty() && evidence.digest == util::digest(&evidence.excerpt),
            "interpretation historical evidence digest mismatch"
        );
        self.evidence.insert(id.into(), evidence);
        Ok(())
    }

    fn knowledge(
        &mut self,
        conn: &Connection,
        endpoint: &CrossSourceEndpoint,
    ) -> Result<&RevisionState> {
        if !self.knowledge.contains_key(&endpoint.revision_id) {
            ensure!(
                self.knowledge.len() < MAX_HISTORY_SOURCES,
                "interpretation history exceeds its complete knowledge revision limit"
            );
            let bytes = conn.query_row(
                "SELECT length(CAST(r.statement AS BLOB))+length(CAST(r.kind AS BLOB))+length(CAST(r.lifecycle AS BLOB))+length(CAST(r.support_state AS BLOB))+length(CAST(t.slug AS BLOB))+length(CAST(d.subject AS BLOB))+length(CAST(d.scope AS BLOB))+length(CAST(d.effective_at AS BLOB))+length(CAST(d.base_lifecycle AS BLOB)) FROM knowledge_revisions r JOIN knowledge_details d ON d.knowledge_id=r.knowledge_id JOIN topics t ON t.id=d.topic_id WHERE r.id=?1 AND r.knowledge_id=?2 AND EXISTS(SELECT 1 FROM sealed_knowledge s WHERE s.id=r.id)",
                [&endpoint.revision_id, &endpoint.id], |row| row.get(0),
            ).context("interpretation knowledge endpoint is not a sealed retained revision")?;
            self.charge(bytes)?;
            let mut state = conn.query_row(
                "SELECT r.knowledge_id,r.id,r.statement,r.kind,r.lifecycle,d.base_lifecycle,r.support_state,t.slug,d.subject,d.scope,d.effective_at FROM knowledge_revisions r JOIN knowledge_details d ON d.knowledge_id=r.knowledge_id JOIN topics t ON t.id=d.topic_id WHERE r.id=?1 AND r.knowledge_id=?2",
                [&endpoint.revision_id, &endpoint.id], |row| Ok(RevisionState {
                    id: row.get(0)?, revision_id: row.get(1)?, statement: row.get(2)?,
                    kind: row.get(3)?, lifecycle: row.get(4)?, original_lifecycle: row.get(5)?,
                    support_state: row.get(6)?, topic: row.get(7)?, subject: row.get(8)?,
                    scope: row.get(9)?, effective_at: row.get(10)?, evidence_ids: Vec::new(),
                    current_evidence_ids: None,
                }),
            )?;
            state.evidence_ids = conn.prepare(
                "SELECT DISTINCT e.evidence_id FROM knowledge_support s JOIN assertion_evidence e ON e.assertion_revision_id=s.assertion_revision_id WHERE s.knowledge_revision_id=?1 ORDER BY e.evidence_id LIMIT ?2",
            )?.query_map(rusqlite::params![endpoint.revision_id, MAX_HISTORY_SOURCES + 1], |row| row.get(0))?
                .collect::<rusqlite::Result<_>>()?;
            ensure!(
                !state.evidence_ids.is_empty() && state.evidence_ids.len() <= MAX_HISTORY_SOURCES,
                "interpretation knowledge endpoint has unavailable or excessive historical support"
            );
            for id in &state.evidence_ids {
                self.documentary(conn, id)?;
            }
            self.knowledge.insert(endpoint.revision_id.clone(), state);
        }
        let state = &self.knowledge[&endpoint.revision_id];
        ensure!(
            state.id == endpoint.id,
            "interpretation knowledge revision belongs to another endpoint"
        );
        Ok(state)
    }

    fn native(&mut self, conn: &Connection, id: &str) -> Result<&ImportedObservation> {
        if !self.imported.contains_key(id) {
            ensure!(
                self.evidence.len() + self.imported.len() < MAX_HISTORY_SOURCES,
                "interpretation history exceeds its complete evidence count limit"
            );
            let bytes = conn.query_row(
                "SELECT length(CAST(record_json AS BLOB)) FROM native_snapshots WHERE evidence_id=?1",
                [id], |row| row.get(0),
            )?;
            self.charge(bytes)?;
            self.imported
                .insert(id.into(), imports::evidence(conn, id)?);
        }
        Ok(&self.imported[id])
    }

    fn state(
        &mut self,
        conn: &Connection,
        key: &str,
        relation: &CrossSourceRelation,
        checkpoint_imported: &BTreeMap<String, ImportedRevision>,
    ) -> Result<InterpretationState> {
        let mut state = InterpretationState {
            relation: relation.clone(),
            present_at_checkpoint: true,
            knowledge_revision_ids: Vec::new(),
            imported: Vec::new(),
        };
        let supplied: BTreeSet<_> = relation.evidence_ids.iter().cloned().collect();
        ensure!(
            !supplied.is_empty() && supplied.len() == relation.evidence_ids.len(),
            "interpretation has missing or duplicate source evidence"
        );
        let mut complete = BTreeSet::new();
        for endpoint in relation.endpoints() {
            let bound = match endpoint.kind.as_str() {
                "knowledge" => {
                    let record = self.knowledge(conn, endpoint)?;
                    state
                        .knowledge_revision_ids
                        .push(record.revision_id.clone());
                    record.evidence_ids.iter().cloned().collect::<BTreeSet<_>>()
                }
                "observation" => {
                    let id: String = conn.query_row(
                        "SELECT evidence_id FROM native_snapshots WHERE observation_id=?1 AND id=?2",
                        [&endpoint.id, &endpoint.revision_id], |row| row.get(0),
                    ).context("interpretation native endpoint revision is unavailable")?;
                    let record = self.native(conn, &id)?;
                    ensure!(
                        record.id == endpoint.id && record.snapshot_id == endpoint.revision_id,
                        "interpretation native endpoint does not match its snapshot"
                    );
                    state.imported.push(ImportedRevision {
                        id: record.id.clone(),
                        snapshot_id: record.snapshot_id.clone(),
                        evidence_id: record.evidence_id.clone(),
                        current: checkpoint_imported.get(&record.id).is_some_and(|saved| {
                            saved.current
                                && saved.snapshot_id == record.snapshot_id
                                && saved.evidence_id == record.evidence_id
                        }),
                    });
                    BTreeSet::from([id])
                }
                _ => anyhow::bail!("invalid interpretation endpoint kind"),
            };
            ensure!(
                !bound.is_disjoint(&supplied),
                "interpretation source evidence does not cover every immutable endpoint"
            );
            complete.extend(bound);
        }
        ensure!(
            supplied.is_subset(&complete),
            "interpretation evidence is substituted from another revision"
        );
        if relation.active {
            imports::relationships::validate_current_relation(conn, relation)?;
        }
        if let Some(kind) = &relation.upstream_kind {
            ensure!(
                relation.from.kind == "observation",
                "upstream relationship has no native origin"
            );
            let from = state
                .imported
                .iter()
                .find(|item| item.id == relation.from.id)
                .context("upstream source endpoint is missing")?;
            let original = &self.imported[&from.evidence_id];
            let native = original
                .record
                .relationships
                .iter()
                .find(|native| {
                    native.kind == *kind
                        && native.upstream_status == relation.upstream_status
                        && Some(native.active) == relation.upstream_active
                        && util::json_digest(&(
                            "upstream",
                            &original.id,
                            &native.native_id,
                            &native.target_native_id,
                            &native.kind,
                        ))
                        .is_ok_and(|expected| expected == key)
                })
                .context(
                    "upstream interpretation status or vocabulary has no exact native witness",
                )?;
            if let Some(target) = &relation.to {
                ensure!(
                    target.kind == "observation",
                    "upstream target was promoted to documentary authority"
                );
                let bound = state
                    .imported
                    .iter()
                    .find(|item| item.id == target.id)
                    .context("upstream target endpoint is missing")?;
                let target = &self.imported[&bound.evidence_id];
                ensure!(
                    original.import_id == target.import_id
                        && native.target_native_id == target.record.native_id,
                    "upstream interpretation has a different native target or source namespace"
                );
            }
        }
        state.knowledge_revision_ids.sort();
        state.knowledge_revision_ids.dedup();
        state
            .imported
            .sort_by(|a, b| a.evidence_id.cmp(&b.evidence_id));
        state
            .imported
            .dedup_by(|a, b| a.evidence_id == b.evidence_id);
        Ok(state)
    }

    /// Rebuild references only from included complete pairs after every trim.
    pub fn populate(&self, report: &mut ChangeReport) -> Result<()> {
        let states = || {
            report
                .cross_source_changes
                .iter()
                .flat_map(|change| change.before.iter().chain(&change.after))
        };
        let revisions: BTreeSet<_> = states()
            .flat_map(|state| &state.knowledge_revision_ids)
            .collect();
        let native_ids: BTreeSet<_> = states()
            .flat_map(|state| state.imported.iter().map(|r| &r.evidence_id))
            .collect();
        let knowledge = revisions
            .into_iter()
            .map(|revision| {
                self.knowledge
                    .get(revision)
                    .cloned()
                    .context("interpretation endpoint escaped its source pool")
            })
            .collect::<Result<Vec<_>>>()?;
        let mut documentary: BTreeMap<_, _> = report
            .evidence
            .iter()
            .cloned()
            .map(|e| (e.id.clone(), e))
            .collect();
        for id in knowledge.iter().flat_map(|record| &record.evidence_ids) {
            documentary.insert(
                id.clone(),
                self.evidence
                    .get(id)
                    .context("interpretation documentary evidence escaped its source pool")?
                    .clone(),
            );
        }
        let mut native: BTreeMap<_, _> = report
            .imported_evidence
            .iter()
            .cloned()
            .map(|e| (e.evidence_id.clone(), e))
            .collect();
        for id in native_ids {
            native.insert(
                id.clone(),
                self.imported
                    .get(id)
                    .context("interpretation native evidence escaped its source pool")?
                    .clone(),
            );
        }
        report.cross_source_knowledge = knowledge;
        report.evidence = documentary.into_values().collect();
        report.imported_evidence = native.into_values().collect();
        Ok(())
    }
}

fn relevant_to(
    relation: &CrossSourceRelation,
    knowledge: Option<&BTreeSet<String>>,
    imported: Option<&BTreeSet<String>>,
) -> bool {
    if knowledge.is_none() && imported.is_none() {
        return true;
    }
    relation
        .endpoints()
        .any(|endpoint| match endpoint.kind.as_str() {
            "knowledge" => knowledge.is_some_and(|ids| ids.contains(&endpoint.id)),
            "observation" => imported.is_some_and(|ids| ids.contains(&endpoint.id)),
            _ => false,
        })
}

fn evaluated_key(conn: &Connection, relation: &CrossSourceRelation) -> Result<String> {
    let (key, signature, from, to) = conn.query_row(
        "SELECT e.pair_key,e.input_signature,e.from_kind,e.from_id,e.from_revision_id,e.to_kind,e.to_id,e.to_revision_id FROM cross_source_relations r JOIN cross_source_evaluations e ON e.input_signature=r.input_signature WHERE r.id=?1",
        [&relation.id], |row| {
            let to = match (row.get::<_, Option<String>>(5)?, row.get::<_, Option<String>>(6)?, row.get::<_, Option<String>>(7)?) {
                (None, None, None) => None,
                (Some(kind), Some(id), Some(revision_id)) => Some(CrossSourceEndpoint {kind, id, revision_id}),
                _ => return Err(rusqlite::Error::InvalidQuery),
            };
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, CrossSourceEndpoint {
                kind: row.get(2)?, id: row.get(3)?, revision_id: row.get(4)?,
            }, to))
        },
    )?;
    ensure!(
        relation.input_signature == signature
            && relation.from == from
            && relation.to == to
            && relation.id
                == format!(
                    "xrel_{}",
                    &util::digest(format!("{signature}:{}", relation.kind))[7..]
                ),
        "interpretation payload differs from its immutable evaluated identity or endpoints"
    );
    Ok(key)
}

pub(super) fn render(report: &ChangeReport, text: &mut String) {
    for change in &report.cross_source_changes {
        text.push_str(&format!(
            "## Cross-source interpretation: {}\n\nComparison `{}`. {}\n\n",
            display(&change.change_kind),
            display(&change.pair_key),
            display(&change.qualification)
        ));
        for (label, state) in [("Previously", &change.before), ("Now", &change.after)] {
            let Some(state) = state else {
                text.push_str(&format!("{label}: no retained interpretation for this comparison at that checkpoint.\n\n"));
                continue;
            };
            let relation = &state.relation;
            text.push_str(&format!("### {label}: `{}`\n\n{}: {}\n\nPresent at this checkpoint: {}. Currently retained at report time: {}.\n\n",
                display(&relation.id), display(&relation.kind), display(&relation.reason),
                state.present_at_checkpoint, relation.active));
            for qualification in &relation.qualifications {
                text.push_str(&format!("{}\n\n", display(qualification)));
            }
            if relation.upstream_kind.is_some()
                || relation.upstream_status.is_some()
                || relation.upstream_active.is_some()
            {
                text.push_str(&format!("Original upstream relationship: {}. Upstream status: {}. Asserted upstream: {}. These are source-native states.\n\n",
                    display(relation.upstream_kind.as_deref().unwrap_or("unspecified")),
                    display(relation.upstream_status.as_deref().unwrap_or("unspecified")),
                    relation.upstream_active.map_or_else(|| "unspecified".into(), |active| active.to_string())));
            }
            for endpoint in relation.endpoints() {
                text.push_str(&format!(
                    "Endpoint {} `{}` at immutable revision `{}`.\n\n",
                    display(&endpoint.kind),
                    display(&endpoint.id),
                    display(&endpoint.revision_id)
                ));
            }
            text.push_str(&format!(
                "Original relationship evidence: {}.\n\n",
                relation
                    .evidence_ids
                    .iter()
                    .map(|id| display(id))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            for revision in &state.knowledge_revision_ids {
                text.push_str(&format!("Complete documentary endpoint: revision `{}` in the original records below.\n\n", display(revision)));
            }
            for source in &state.imported {
                text.push_str(&format!("Native endpoint `{}`: evidence `{}` at snapshot `{}`; source-current at this checkpoint: {}.\n\n",
                    display(&source.id), display(&source.evidence_id), display(&source.snapshot_id), source.current));
            }
        }
        if change.baseline_review_status == "not_captured" {
            text.push_str("Baseline review disposition was not captured. It is not reconstructed from today's review status.\n\n");
        }
        for review in &change.current_reviews {
            text.push_str(&format!(
                "Current review `{}`: {}. {}\n\n",
                display(&review.review_id),
                display(review.status.as_deref().unwrap_or("unavailable")),
                display(&review.qualification)
            ));
        }
    }
    // Render shared originals once. Repeating a large endpoint for every pair
    // would multiply output work even though its source pool is bounded.
    for record in &report.cross_source_knowledge {
        text.push_str(&format!(
            "### Original documentary interpretation endpoint `{}`\n\nKnowledge `{}`.\n\n",
            display(&record.revision_id),
            display(&record.id)
        ));
        render_revision("Original source statement", record, text);
        text.push_str(&format!(
            "Complete revision support evidence: {}.\n\n",
            record
                .evidence_ids
                .iter()
                .map(|id| display(id))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    let native_ids: BTreeSet<_> = report
        .cross_source_changes
        .iter()
        .flat_map(|change| change.before.iter().chain(&change.after))
        .flat_map(|state| state.imported.iter().map(|source| &source.evidence_id))
        .collect();
    let native: BTreeMap<_, _> = report
        .imported_evidence
        .iter()
        .map(|source| (&source.evidence_id, source))
        .collect();
    for id in native_ids {
        if let Some(source) = native.get(id) {
            text.push_str(&format!("### Original native relationship evidence `{}`\n\nSource {}:{}; immutable snapshot `{}`; content hash `{}`. Current source membership at report time: {}.\n\nSource kind: {}; format: {}; captured at {} from {}.\n\nFull source-owned payload, scope and verification metadata:\n\n",
                display(id), display(&source.import_id), display(&source.record.native_id),
                display(&source.snapshot_id), display(&source.content_hash), source.current,
                source.origin.as_str(), display(&source.format), display(&source.captured_at), display(&source.source_path)));
            text.push_str(&quote(
                &serde_json::to_string(&source.record).unwrap_or_default(),
            ));
            text.push('\n');
        }
    }
}
