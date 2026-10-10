//! Re-resolve presentation support against original retained sources, rather
//! than treating a generated decision brief as another source of project truth.

use super::*;
use crate::{
    context::{
        decision::runtime::DecisionContextResult,
        inspection::{CodeObservation, validate_observation},
    },
    domain::SourceMaterial,
    imports,
};
use anyhow::ensure;
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize)]
pub(super) struct Record {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub lifecycle: String,
    pub scope: String,
    pub current: bool,
    pub claim: Claim,
}

impl Record {
    fn is_future_intent(&self) -> bool {
        self.lifecycle == "proposed" || matches!(self.kind.as_str(), "proposal" | "plan")
    }

    /// Source freshness and adoption are independent. A newly retained plan
    /// can inform a question without describing the project's current design.
    pub fn describes_present_project(&self) -> bool {
        self.current && !self.is_future_intent()
    }
}

pub(super) struct Catalog {
    pub evidence: BTreeMap<String, ExactReference>,
    pub records: Vec<Record>,
    pub observations: BTreeMap<String, CodeObservation>,
    source_relationships: adaptive::SourceRelationships,
    contexts: BTreeMap<String, (String, String)>,
    paths: BTreeSet<String>,
}

impl Catalog {
    pub fn load(
        conn: &Connection,
        result: &DecisionContextResult,
        source_relationships: &adaptive::SourceRelationships,
    ) -> Result<Self> {
        let relationship_catalog = source_relationships.validate(conn)?;
        let mut catalog = Self {
            evidence: BTreeMap::new(),
            records: Vec::new(),
            observations: BTreeMap::new(),
            source_relationships: source_relationships.clone(),
            contexts: BTreeMap::new(),
            paths: BTreeSet::new(),
        };
        let manifests = match result {
            DecisionContextResult::Brief(result) => {
                for observation in &result.inspection.observations {
                    validate_observation(observation)?;
                    catalog.paths.insert(observation.path.clone());
                    ensure!(
                        catalog
                            .observations
                            .insert(observation.id.clone(), observation.clone())
                            .is_none(),
                        "duplicate human-view observation"
                    );
                }
                result.evidence.clone()
            }
            DecisionContextResult::FastFallback(result) => {
                // Existing validation provides matching historical/current and
                // native snapshot semantics, including older schema readers.
                crate::context::intelligence::EvidenceCatalog::validate(conn, &result.context)?
                    .evidence
                    .into_values()
                    .collect()
            }
        };
        let mut combined = BTreeMap::new();
        for manifest in manifests
            .into_iter()
            .chain(relationship_catalog.evidence.into_values())
        {
            if let Some(previous) = combined.insert(manifest.id.clone(), manifest.clone()) {
                ensure!(
                    serde_json::to_value(previous)? == serde_json::to_value(&manifest)?,
                    "shared relationship and decision evidence disagree"
                );
            }
        }
        ensure!(
            combined.len() <= 1024,
            "too many human-view source references"
        );
        for manifest in combined.into_values() {
            let reference = if manifest.id.starts_with("ne_") {
                let archived = imports::evidence(conn, &manifest.id)?;
                ensure!(
                    archived.snapshot_id == manifest.revision_id
                        && archived.content_hash == manifest.content_hash
                        && archived.current == manifest.current
                        && format!("{}:{}", archived.import_id, archived.record.native_id)
                            == manifest.source,
                    "human-view native evidence changed"
                );
                catalog.records.push(Record {
                    id: archived.id, title: archived.record.title,
                    kind: format!("{:?}", archived.record.kind).to_lowercase(),
                    lifecycle: archived.record.lifecycle.clone(),
                    scope: serde_json::to_string(&archived.record.scope)?,
                    current: archived.current,
                    claim: Claim {
                        text: archived.record.statement.clone(), basis: ClaimBasis::ImportedReport,
                        evidence_ids: vec![manifest.id.clone()], observation_ids: Vec::new(),
                        qualifications: vec![format!("Imported {} report; source lifecycle {}. Current means present in the imported snapshot, not verified implementation.",
                            archived.origin.as_str(), archived.record.lifecycle)],
                    },
                });
                ExactReference {
                    evidence_id: manifest.id,
                    source: manifest.source,
                    revision_id: manifest.revision_id,
                    content_hash: manifest.content_hash,
                    current: manifest.current,
                    basis: manifest.basis,
                    excerpt: archived.record.statement,
                    line_start: None,
                    line_end: None,
                }
            } else {
                let archived = storage::evidence_snapshot(conn, &manifest.id)?;
                ensure!(
                    archived.source_revision_id == manifest.revision_id
                        && archived.digest == manifest.content_hash
                        && util::digest(&archived.excerpt) == archived.digest
                        && format!("{}:{}", archived.root_id, archived.path) == manifest.source,
                    "human-view documentary evidence differs from its exact snapshot"
                );
                catalog.contexts.insert(
                    manifest.id.clone(),
                    (archived.context_before, archived.context_after),
                );
                catalog.paths.insert(archived.path);
                catalog.paths.extend(paths(&archived.excerpt));
                ExactReference {
                    evidence_id: manifest.id,
                    source: manifest.source,
                    revision_id: manifest.revision_id,
                    content_hash: manifest.content_hash,
                    current: manifest.current,
                    basis: manifest.basis,
                    excerpt: archived.excerpt,
                    line_start: archived.line_start,
                    line_end: archived.line_end,
                }
            };
            ensure!(
                catalog
                    .evidence
                    .insert(reference.evidence_id.clone(), reference)
                    .is_none(),
                "duplicate human-view evidence identity"
            );
        }
        for record in storage::views(conn)? {
            let ids = record
                .evidence
                .iter()
                .filter(|e| catalog.evidence.contains_key(&e.id))
                .map(|e| e.id.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            if ids.is_empty() {
                continue;
            }
            let mut qualifications = vec![format!(
                "Documented {} ({}), scope: {}; support: {}.",
                record.kind, record.lifecycle, record.scope, record.support_state
            )];
            if !record.effective_at.is_empty() {
                qualifications.push(format!("Effective at: {}.", record.effective_at));
            }
            if record
                .evidence
                .iter()
                .filter(|e| ids.contains(&e.id))
                .all(|e| e.material == SourceMaterial::Derived)
            {
                qualifications
                    .push("Derived documentation; not independent primary evidence.".into());
            }
            let current = !matches!(record.lifecycle.as_str(), "rejected" | "superseded")
                && record.support_state != "historical_only"
                && ids.iter().any(|id| catalog.evidence[id].current);
            if !current {
                qualifications.push("Historical or unadopted context; this does not establish a current project requirement.".into());
            }
            catalog.records.push(Record {
                id: record.id,
                title: record.subject,
                kind: record.kind,
                lifecycle: record.lifecycle,
                scope: record.scope,
                current,
                claim: Claim {
                    text: record.statement,
                    basis: ClaimBasis::Documentary,
                    evidence_ids: ids,
                    observation_ids: Vec::new(),
                    qualifications,
                },
            });
        }
        source_relationships.validate_retained(
            conn,
            &catalog
                .records
                .iter()
                .map(|record| record.id.clone())
                .collect(),
        )?;
        for record in &mut catalog.records {
            // A simplified endpoint claim keeps the source-owned caveats of
            // every retained relationship that applies to it. Generated prose
            // cannot turn an upstream status into a verified outcome.
            let mut qualifiers = Vec::new();
            if let Some(endpoint) = source_relationships
                .knowledge
                .iter()
                .find(|k| k.id == record.id)
            {
                qualifiers.extend(endpoint.qualifications.clone());
            }
            if let Some(endpoint) = source_relationships
                .observations
                .iter()
                .find(|o| o.id == record.id)
            {
                qualifiers.extend(endpoint.qualifications.clone());
            }
            for relation in source_relationships.relations.iter().filter(|relation| {
                relation
                    .endpoints()
                    .any(|endpoint| endpoint.id == record.id)
            }) {
                qualifiers.push(format!(
                    "Retained {} relationship [{}]: {}",
                    relation.kind, relation.id, relation.reason
                ));
                qualifiers.extend(relation.qualifications.clone());
                if relation.upstream_kind.is_some()
                    || relation.upstream_status.is_some()
                    || relation.upstream_active.is_some()
                {
                    qualifiers.push(format!("Source-reported relationship: {}; upstream status: {}; asserted upstream: {}. This does not independently verify implementation or policy adoption.",
                        relation.upstream_kind.as_deref().unwrap_or("unspecified"),
                        relation.upstream_status.as_deref().unwrap_or("unspecified"),
                        relation.upstream_active.map_or_else(|| "unspecified".into(), |active| active.to_string())));
                }
            }
            for discrepancy in source_relationships.discrepancies.iter().filter(|d| {
                d.knowledge_ids.contains(&record.id) || d.observation_ids.contains(&record.id)
            }) {
                qualifiers.extend(discrepancy.qualifications.clone());
            }
            for qualification in qualifiers {
                if !record.claim.qualifications.contains(&qualification) {
                    record.claim.qualifications.push(qualification);
                }
            }
            if record.is_future_intent() {
                record.claim.qualifications.push("Future or proposed intent; this source does not establish adoption or current implementation.".into());
            }
        }
        catalog
            .records
            .sort_by(|a, b| (!a.current, &a.id).cmp(&(!b.current, &b.id)));
        Ok(catalog)
    }

    pub fn constraints(&self) -> Vec<Claim> {
        self.records
            .iter()
            .filter(|r| matches!(r.kind.as_str(), "constraint" | "decision" | "risk"))
            .map(|r| r.claim.clone())
            .collect()
    }

    pub fn references(&self, query: &str) -> Vec<ExactReference> {
        let terms = query
            .split(|c: char| !c.is_alphanumeric())
            .filter(|term| term.len() > 2)
            .map(str::to_lowercase)
            .collect::<BTreeSet<_>>();
        let score = |reference: &ExactReference| {
            let text = format!("{} {}", reference.source, reference.excerpt).to_lowercase();
            terms
                .iter()
                .filter(|term| text.contains(term.as_str()))
                .count()
        };
        let mut references = self.evidence.values().cloned().collect::<Vec<_>>();
        references.sort_by(|a, b| {
            score(b)
                .cmp(&score(a))
                .then_with(|| b.current.cmp(&a.current))
                .then_with(|| a.evidence_id.cmp(&b.evidence_id))
        });
        references
    }

    pub fn orientation(&self) -> Orientation {
        let current = self
            .records
            .iter()
            .filter(|r| r.describes_present_project())
            .collect::<Vec<_>>();
        let design = current.iter().find(|r| r.kind == "design").or_else(|| {
            current
                .iter()
                .find(|r| r.kind != "constraint" && r.kind != "decision")
        });
        let purpose = design.map(|r| r.claim.clone());
        let rationale = current
            .iter()
            .find(|r| {
                let text = r.claim.text.to_lowercase();
                ["because", "so that", "in order to", "rationale"]
                    .iter()
                    .any(|term| text.contains(term))
            })
            .map(|r| r.claim.clone());
        let mut titles = BTreeSet::new();
        let concepts = current
            .iter()
            .filter(|r| titles.insert(r.title.to_lowercase()))
            .take(5)
            .map(|r| Concept {
                id: concept_id(&r.title),
                title: r.title.clone(),
                description: r.claim.clone(),
            })
            .collect::<Vec<_>>();
        let architecture = current
            .iter()
            .filter(|r| r.kind == "design")
            .take(4)
            .map(|r| r.claim.clone())
            .collect();
        // Retain an original procedure as one narrative. Paragraph order or
        // filename order alone cannot manufacture a chain of runtime calls.
        let workflow = current.iter().find(|r| r.kind == "procedure").map(|record| Workflow {
            title: record.title.clone(), sequence_supported: false,
            stops: vec![TourStop { title: "Documented workflow".into(),
                role: TourRole::DocumentedWorkflow, description: record.claim.clone() }],
            limitations: vec!["This is the source's procedure. Lore has not observed it execute or inferred additional transitions.".into()],
        });
        let mut gaps = Vec::new();
        if purpose.is_none() {
            gaps.push(
                "The selected evidence does not describe the project's overall purpose.".into(),
            );
        }
        if rationale.is_none() {
            gaps.push("The selected sources do not state an explicit project motivation.".into());
        }
        if workflow.is_none() {
            gaps.push("A complete ordered workflow is not established by the selected documentary evidence.".into());
        }
        let next_exploration = concepts
            .iter()
            .take(3)
            .map(|c| ExploreNext {
                label: format!("Explore {}", c.title),
                goal: format!("Explain {}", c.title),
                mode: ExperienceMode::Explanation,
                evidence_ids: c.description.evidence_ids.clone(),
                observation_ids: c.description.observation_ids.clone(),
            })
            .collect();
        Orientation {
            purpose,
            rationale,
            concepts,
            architecture,
            workflow,
            constraints: self.constraints(),
            next_exploration,
            gaps,
        }
    }

    pub fn model_input(&self, include_checkout: bool) -> Value {
        json!({
            "records":self.records,
            "source_relationships":self.source_relationships.relations,
            "source_discrepancies":self.source_relationships.discrepancies,
            "original_evidence":self.evidence.values().map(|e| {
                let context = self.contexts.get(&e.evidence_id);
                json!({"reference": e,
                    "context_before":context.map(|c| &c.0),
                    "context_after":context.map(|c| &c.1)})
            }).collect::<Vec<_>>(),
            "static_observations":if include_checkout {
                self.observations.values().collect::<Vec<_>>()
            } else { Vec::new() },
        })
    }

    pub fn validate_claim(&self, claim: &Claim, include_checkout: bool) -> Result<()> {
        self.validate_text(&claim.text, 4_000)?;
        self.validate_references(
            &claim.evidence_ids,
            &claim.observation_ids,
            include_checkout,
        )?;
        ensure!(
            claim.qualifications.len() <= 12,
            "too many claim qualifications"
        );
        for qualification in &claim.qualifications {
            self.validate_text(qualification, 1_000)?;
        }
        match claim.basis {
            ClaimBasis::Documentary => ensure!(
                !claim.evidence_ids.is_empty()
                    && claim
                        .evidence_ids
                        .iter()
                        .any(|id| self.evidence[id].basis.contains("documentary")),
                "documentary claim lacks documentary support"
            ),
            ClaimBasis::ImportedReport => ensure!(
                !claim.evidence_ids.is_empty()
                    && claim
                        .evidence_ids
                        .iter()
                        .all(|id| self.evidence[id].basis.ends_with("_report")),
                "imported claim is not source-owned report evidence"
            ),
            ClaimBasis::StaticInference => ensure!(
                !claim.observation_ids.is_empty(),
                "static inference lacks an inspected observation"
            ),
            ClaimBasis::Inference | ClaimBasis::Hypothetical => {}
        }
        let current = claim
            .evidence_ids
            .iter()
            .any(|id| self.evidence[id].current)
            || !claim.observation_ids.is_empty();
        ensure!(
            current
                || claim
                    .qualifications
                    .iter()
                    .any(|q| q.to_lowercase().contains("histor")),
            "historical support requires a visible historical qualification"
        );
        Ok(())
    }

    pub fn validate_references(
        &self,
        evidence: &[String],
        observations: &[String],
        include_checkout: bool,
    ) -> Result<()> {
        ensure!(
            (1..=32).contains(&(evidence.len() + observations.len()))
                && evidence.iter().collect::<BTreeSet<_>>().len() == evidence.len()
                && observations.iter().collect::<BTreeSet<_>>().len() == observations.len()
                && evidence.iter().all(|id| self.evidence.contains_key(id))
                && observations
                    .iter()
                    .all(|id| include_checkout && self.observations.contains_key(id)),
            "human claim cites missing, duplicated or unauthorized evidence"
        );
        Ok(())
    }

    pub fn validate_text(&self, text: &str, limit: usize) -> Result<()> {
        ensure!(
            !text.trim().is_empty() && text.len() <= limit && !text.chars().any(char::is_control),
            "empty, oversized or unsafe human-view text"
        );
        for path in paths(text) {
            ensure!(
                self.paths.contains(&path),
                "human view invented a source location"
            );
        }
        for word in text.split(|c: char| !c.is_alphanumeric() && c != '_') {
            if word.starts_with("ev_") || word.starts_with("ne_") || word.starts_with("co_") {
                ensure!(
                    self.evidence.contains_key(word) || self.observations.contains_key(word),
                    "human prose contains an unknown evidence identity"
                );
            }
        }
        let lower = text.to_lowercase();
        for assertion in [
            "i ran",
            "we ran",
            "lore ran",
            "i executed",
            "we executed",
            "lore executed",
            "tests passed",
            "test suite passed",
            "verified at runtime",
            "runtime is verified",
            "you have mastered",
            "you mastered",
            "mastery achieved",
            "contribution is verified",
        ] {
            ensure!(
                !lower.contains(assertion),
                "human view claims execution or learner mastery without proof"
            );
        }
        Ok(())
    }

    pub fn qualify(&self, claim: &mut Claim) {
        // Scope/authority from original records survives simplification. These
        // are source qualifications, never additional independently proved facts.
        for record in &self.records {
            if record
                .claim
                .evidence_ids
                .iter()
                .any(|id| claim.evidence_ids.contains(id))
            {
                for qualifier in &record.claim.qualifications {
                    if !claim.qualifications.contains(qualifier) {
                        claim.qualifications.push(qualifier.clone());
                    }
                }
            }
        }
        if !claim.observation_ids.is_empty() {
            let qualification =
                "Static source inspection; not an observed execution or a passing test."
                    .to_string();
            if !claim.qualifications.contains(&qualification) {
                claim.qualifications.push(qualification);
            }
        }
    }
}

pub(super) fn concept_id(title: &str) -> String {
    format!(
        "concept_{}",
        &util::digest(title.trim().to_lowercase())[7..23]
    )
}

fn paths(text: &str) -> BTreeSet<String> {
    text.split(|c: char| {
        c.is_whitespace()
            || matches!(
                c,
                '`' | '"' | '\'' | '(' | ')' | '[' | ']' | '<' | '>' | ',' | ';'
            )
    })
    .map(|word| {
        word.trim_end_matches(['.', ':', '!', '?'])
            .replace('\\', "/")
    })
    .filter(|word| !word.contains("://"))
    .map(|word| word.split(':').next().unwrap_or(&word).to_string())
    .filter(|word| {
        word.starts_with("src/")
            || word.starts_with("tests/")
            || word.starts_with("../")
            || word.starts_with('/')
            || matches!(
                word.rsplit('.').next(),
                Some(
                    "rs" | "py"
                        | "js"
                        | "ts"
                        | "go"
                        | "java"
                        | "cpp"
                        | "c"
                        | "h"
                        | "md"
                        | "json"
                        | "toml"
                        | "yml"
                        | "yaml"
                        | "sh"
                )
            )
    })
    .collect()
}
