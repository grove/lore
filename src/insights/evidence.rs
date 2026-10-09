use super::{DocumentedFacet, FacetKind, FacetRecord};
use crate::{context::ContextItem, domain::KnowledgeView, storage, util};
use anyhow::{Result, ensure};
use rusqlite::{Connection, OptionalExtension};
use std::collections::{BTreeMap, BTreeSet};

const MAX_FACET_EVIDENCE: usize = 64;

pub(super) struct Details<'a> {
    conn: &'a Connection,
    views: Vec<KnowledgeView>,
    headings: BTreeMap<String, Vec<String>>,
}

fn normalized(value: &str) -> String {
    value
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|part| !part.is_empty() && !part.chars().all(char::is_numeric))
        .collect::<Vec<_>>()
        .join(" ")
}

fn category(value: &str) -> Option<FacetKind> {
    Some(match normalized(value).as_str() {
        "context"
        | "background"
        | "starting conditions"
        | "preconditions"
        | "initial conditions"
        | "context and problem statement" => FacetKind::StartingConditions,
        "rationale" | "reason" | "reasons" | "decision drivers" | "why" | "why this worked"
        | "why this failed" => FacetKind::Rationale,
        "alternatives" | "considered options" | "options considered" => FacetKind::Alternatives,
        "trade offs" | "tradeoffs" | "tradeoff" | "trade off" => FacetKind::TradeOffs,
        "assumptions" => FacetKind::Assumptions,
        "constraints" | "invariants" | "requirements" => FacetKind::Constraints,
        "conditions" | "applicability" | "applicability conditions" | "when this applies" => {
            FacetKind::Conditions
        }
        "exceptions" | "exception" | "boundary conditions" | "important exceptions" => {
            FacetKind::Exceptions
        }
        "expected behavior" | "expected outcome" | "expected result" => FacetKind::ExpectedBehavior,
        "actual behavior" | "actual outcome" | "actual result" | "outcome" | "observed outcome"
        | "reported outcome" | "results" => FacetKind::ReportedOutcome,
        "consequences" | "positive consequences" | "negative consequences" => {
            FacetKind::Consequences
        }
        "reconsideration"
        | "reconsideration triggers"
        | "review triggers"
        | "when to revisit"
        | "revisit when" => FacetKind::ReconsiderationTriggers,
        "rejected approaches" | "rejected options" | "rejected alternatives" => {
            FacetKind::RejectedApproaches
        }
        "failures" | "failure modes" | "known failures" | "counterexamples" | "failure case" => {
            FacetKind::FailureModes
        }
        _ => return None,
    })
}

fn section(headings: &[String]) -> Option<(FacetKind, &[String])> {
    headings
        .iter()
        .enumerate()
        .rev()
        .find_map(|(index, heading)| category(heading).map(|kind| (kind, &headings[..index])))
}

fn family(headings: &[String]) -> Vec<String> {
    if let Some((_, parent)) = section(headings) {
        return parent.to_vec();
    }
    if headings.last().is_some_and(|heading| {
        matches!(
            normalized(heading).as_str(),
            "decision"
                | "decision outcome"
                | "selected option"
                | "resolution"
                | "accepted decision"
                | "procedure"
                | "steps"
                | "workflow"
                | "scenario"
                | "case"
        )
    }) {
        headings[..headings.len() - 1].to_vec()
    } else {
        headings.to_vec()
    }
}

fn explicit_marker(excerpt: &str) -> Option<FacetKind> {
    for line in excerpt.lines() {
        let value = line.trim().trim_start_matches(['-', '*']).trim();
        if let Some((label, _)) = value.split_once(':')
            && let Some(kind) = category(label)
        {
            return Some(kind);
        }
    }
    None
}

impl<'a> Details<'a> {
    pub(super) fn new(conn: &'a Connection) -> Result<Self> {
        Ok(Self {
            conn,
            views: storage::views(conn)?,
            headings: BTreeMap::new(),
        })
    }

    pub(super) fn original_lifecycle(&self, id: &str) -> String {
        self.views
            .iter()
            .find(|record| record.id == id)
            .map(|record| record.base_lifecycle.clone())
            .unwrap_or_else(|| "unknown".into())
    }

    fn heading(&mut self, id: &str) -> Result<Vec<String>> {
        if let Some(value) = self.headings.get(id) {
            return Ok(value.clone());
        }
        let raw: Option<String> = self.conn.query_row(
            "SELECT s.heading_path_json FROM evidence_snapshots e LEFT JOIN section_revisions s ON s.id=e.section_revision_id WHERE e.id=?1",
            [id], |row| row.get(0),
        ).optional()?.flatten();
        let value: Vec<String> = raw
            .map(|raw| serde_json::from_str(&raw))
            .transpose()?
            .unwrap_or_default();
        self.headings.insert(id.into(), value.clone());
        Ok(value)
    }

    /// Preserve every structurally associated retained facet as an atomic
    /// lens. A large source family is omitted whole, never selectively stripped
    /// of an exception or unfavorable rationale to fit a budget.
    pub(super) fn facets(&mut self, record: &ContextItem) -> Result<Option<Vec<DocumentedFacet>>> {
        let anchor_ids: BTreeSet<_> = record.evidence_ids.iter().cloned().collect();
        let mut anchors = Vec::new();
        for id in &record.evidence_ids {
            let snapshot = storage::evidence_snapshot(self.conn, id)?;
            let active = self
                .views
                .iter()
                .flat_map(|view| &view.evidence)
                .any(|e| e.id == *id && e.active);
            anchors.push((
                snapshot.source_id,
                snapshot.source_revision_id,
                active,
                family(&self.heading(id)?),
            ));
        }
        let mut candidates = Vec::new();
        for view in &self.views {
            for evidence in &view.evidence {
                if anchors.iter().any(|(source, revision, active, _)| {
                    evidence.source_id == *source
                        && ((*active && evidence.active)
                            || evidence.source_revision_id == *revision)
                }) {
                    candidates.push((
                        FacetRecord {
                            knowledge_id: view.id.clone(),
                            kind: view.kind.clone(),
                            lifecycle: view.lifecycle.clone(),
                            original_lifecycle: view.base_lifecycle.clone(),
                            scope: view.scope.clone(),
                            support_state: view.support_state.clone(),
                        },
                        evidence.clone(),
                    ));
                }
            }
        }
        let mut seen = BTreeSet::new();
        let mut facets = BTreeMap::new();
        for (record, evidence) in candidates {
            let headings = self.heading(&evidence.id)?;
            let categorized = section(&headings)
                .map(|(kind, parent)| (kind, parent.to_vec(), "captured_source_heading"))
                .or_else(|| {
                    explicit_marker(&evidence.excerpt)
                        .map(|kind| (kind, family(&headings), "explicit_retained_source_marker"))
                });
            let Some((kind, parent, basis)) = categorized else {
                continue;
            };
            // Synthetic Document/Preamble nodes (and an empty parent) supply
            // no boundary between distinct choices in a plain-text source.
            // A marker can organize the anchor's own quote, but cannot attach
            // another record's rationale merely because it shares that file.
            let unstructured = parent.is_empty()
                || parent
                    .iter()
                    .all(|heading| matches!(normalized(heading).as_str(), "document" | "preamble"));
            if unstructured && !anchor_ids.contains(&evidence.id) {
                continue;
            }
            if !anchors.iter().any(|(source, revision, active, expected)| {
                evidence.source_id == *source
                    && ((*active && evidence.active) || evidence.source_revision_id == *revision)
                    && parent == *expected
            }) {
                continue;
            }
            if seen.insert(evidence.id.clone()) && seen.len() > MAX_FACET_EVIDENCE {
                return Ok(None);
            }
            let key = (kind, evidence.id.clone());
            if !facets.contains_key(&key) {
                let snapshot = storage::evidence_snapshot(self.conn, &evidence.id)?;
                ensure!(
                    util::digest(&snapshot.excerpt) == snapshot.digest,
                    "retained decision evidence has an invalid excerpt digest"
                );
                facets.insert(
                    key.clone(),
                    DocumentedFacet {
                        kind,
                        classification_basis: basis.into(),
                        heading_path: headings,
                        records: Vec::new(),
                        evidence_id: evidence.id.clone(),
                        current: evidence.active,
                        evidence: snapshot,
                    },
                );
            }
            let facet = facets.get_mut(&key).expect("facet inserted above");
            if !facet
                .records
                .iter()
                .any(|item| item.knowledge_id == record.knowledge_id)
            {
                facet.records.push(record);
            }
        }
        Ok(Some(
            facets
                .into_values()
                .map(|mut facet| {
                    facet
                        .records
                        .sort_by(|a, b| a.knowledge_id.cmp(&b.knowledge_id));
                    facet
                })
                .collect(),
        ))
    }
}
