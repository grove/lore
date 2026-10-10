//! Local retrieval and presentation of source-owned native observations.
//!
//! An imported record's `current` bit means current in the last imported
//! snapshot. Neither that bit nor an upstream verification token certifies the
//! currently checked-out code. Context preserves those independent dimensions.

use super::{
    ContextEvidence, ContextResult, ContextReview, InspectionPath, display_text, failure,
    retrieval,
    semantic::{self, SemanticHit},
};
use crate::{
    domain::ImportKind,
    imports::{
        self, ImportedObservation,
        adapters::{NativeEvidence, ObservationKind, ObservationScope, ObservationVerification},
        relationships::CrossSourceRelation,
    },
    reviews, storage, util,
};
use anyhow::Result;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

const MAX_NATIVE_SEEDS: usize = 48;
const MAX_NATIVE_CANDIDATES: usize = 128;
const MAX_COMPONENT_NEIGHBORS: usize = 12;
const MAX_SOURCE_WARNINGS: usize = 2;
const MAX_WARNING_CHARACTERS: usize = 160;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextObservation {
    pub id: String,
    pub import_id: String,
    pub origin: ImportKind,
    pub native_id: String,
    pub kind: ObservationKind,
    pub title: String,
    pub subject: String,
    pub statement: String,
    pub scope: ObservationScope,
    pub lifecycle: String,
    pub verification: ObservationVerification,
    /// Current in the last imported snapshot; never a checkout assertion.
    pub current: bool,
    pub freshness: String,
    pub evidence_ids: Vec<String>,
    pub qualifications: Vec<String>,
    pub relevance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextImportedEvidence {
    pub id: String,
    pub observation_id: String,
    pub snapshot_id: String,
    pub import_id: String,
    pub origin: ImportKind,
    pub native_id: String,
    pub source: String,
    pub format: String,
    pub content_hash: String,
    pub captured_at: String,
    pub observed_at: Option<String>,
    pub current: bool,
    pub verification: ObservationVerification,
    pub locators: Vec<NativeEvidence>,
    /// The complete immutable native record is available through `lore evidence`.
    /// Context carries its content hash and exact source-owned metadata without
    /// duplicating a potentially very large issue or memory export.
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextDiscrepancy {
    pub id: String,
    pub kind: String,
    pub status: String,
    pub reason: String,
    pub knowledge_ids: Vec<String>,
    pub observation_ids: Vec<String>,
    pub evidence_ids: Vec<String>,
    pub qualifications: Vec<String>,
    pub review_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextVerification {
    pub action: String,
    pub record_ids: Vec<String>,
    pub evidence_ids: Vec<String>,
    pub relationship_id: Option<String>,
}

#[derive(Default)]
pub(super) struct NativeRetrieval {
    pub observations: BTreeMap<String, ImportedObservation>,
    pub scores: BTreeMap<String, f64>,
    pub reasons: BTreeMap<String, Vec<String>>,
    pub relations: Vec<CrossSourceRelation>,
    pub warnings: Vec<String>,
    pub omitted_warnings: usize,
    pub reviews: BTreeMap<String, reviews::ReviewItem>,
    pub truncated: bool,
}

#[derive(Default)]
struct Candidate {
    score: f64,
    reasons: BTreeSet<String>,
}

fn add_candidate(
    candidates: &mut BTreeMap<String, Candidate>,
    id: &str,
    score: f64,
    reason: String,
) {
    let candidate = candidates.entry(id.to_owned()).or_default();
    candidate.score = candidate.score.max(score);
    candidate.reasons.insert(reason);
}

fn ranked(candidates: &BTreeMap<String, Candidate>) -> Vec<(String, f64)> {
    let mut ranked: Vec<_> = candidates
        .iter()
        .map(|(id, candidate)| (id.clone(), candidate.score))
        .collect();
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    ranked
}

fn retain_best(candidates: &mut BTreeMap<String, Candidate>, limit: usize) -> bool {
    if candidates.len() <= limit {
        return false;
    }
    let keep: BTreeSet<_> = ranked(candidates)
        .into_iter()
        .take(limit)
        .map(|(id, _)| id)
        .collect();
    candidates.retain(|id, _| keep.contains(id));
    true
}

fn words(text: &str) -> BTreeSet<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// Native identifiers are matched as complete identifiers, including their
/// punctuation. An issue PAY-17 must not select PAY-170 or MY-PAY-17.
fn mentions_identifier(text: &str, identifier: &str) -> bool {
    if identifier.is_empty() {
        return false;
    }
    let text = text.to_lowercase();
    let identifier = identifier.to_lowercase();
    let identifier_character = |c: char| c.is_alphanumeric() || matches!(c, '_' | '-');
    text.match_indices(&identifier).any(|(start, matched)| {
        let before = text[..start].chars().next_back();
        let after = text[start + matched.len()..].chars().next();
        !before.is_some_and(identifier_character) && !after.is_some_and(identifier_character)
    })
}

fn compatible_scope(a: &ObservationScope, b: &ObservationScope) -> bool {
    [&a.repository, &a.environment]
        .into_iter()
        .zip([&b.repository, &b.environment])
        .all(|(left, right)| match (left, right) {
            (Some(left), Some(right)) => left.eq_ignore_ascii_case(right),
            _ => true,
        })
}

fn locator_path(locator: &str) -> String {
    let locator = locator.replace('\\', "/");
    let locator = locator.strip_prefix("repo://").unwrap_or(&locator);
    locator
        .split(['#', '?'])
        .next()
        .unwrap_or(locator)
        .trim_start_matches("./")
        .to_lowercase()
}

fn lexical_hits(conn: &Connection, terms: &[String]) -> Result<(Vec<(String, f64)>, bool)> {
    if terms.is_empty() {
        return Ok((Vec::new(), false));
    }
    let expression = terms
        .iter()
        .map(|term| format!("\"{term}\""))
        .collect::<Vec<_>>()
        .join(" OR ");
    let mut query = conn.prepare(
        "SELECT observation_id,bm25(native_fts,0.0,4.0,2.0,2.0,1.0,1.0,1.0,2.0)
         FROM native_fts WHERE native_fts MATCH ?1
         ORDER BY bm25(native_fts,0.0,4.0,2.0,2.0,1.0,1.0,1.0,2.0),observation_id LIMIT ?2",
    )?;
    let mut rows: Vec<(String, f64)> = query
        .query_map(params![expression, (MAX_NATIVE_SEEDS + 1) as i64], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?
        .collect::<rusqlite::Result<_>>()?;
    let truncated = rows.len() > MAX_NATIVE_SEEDS;
    rows.truncate(MAX_NATIVE_SEEDS);
    let best = rows
        .first()
        .map_or(1.0, |(_, rank)| (-rank).max(f64::EPSILON));
    Ok((
        rows.into_iter()
            .map(|(id, rank)| (id, ((-rank) / best).clamp(0.0, 1.0)))
            .collect(),
        truncated,
    ))
}

fn historical(observation: &ImportedObservation) -> bool {
    !observation.current
        || matches!(
            observation.record.lifecycle.to_lowercase().as_str(),
            "deleted"
                | "deprecated"
                | "superseded"
                | "withdrawn"
                | "rejected"
                | "retracted"
                | "orphaned"
        )
}

fn summarize_warnings(mut warnings: Vec<String>) -> (Vec<String>, usize) {
    warnings.sort();
    warnings.dedup();
    let total = warnings.len();
    let mut omitted = total.saturating_sub(MAX_SOURCE_WARNINGS);
    let mut summary = Vec::new();
    for warning in warnings.into_iter().take(MAX_SOURCE_WARNINGS) {
        let mut safe = warning
            .chars()
            .map(|c| if c.is_control() { ' ' } else { c });
        let mut sample: String = safe.by_ref().take(MAX_WARNING_CHARACTERS).collect();
        if safe.next().is_some() {
            omitted += 1;
            sample.push_str("… (diagnostic shortened)");
        }
        summary.push(sample);
    }
    if omitted > 0 {
        summary.push(format!("Source diagnostics abbreviated: {omitted} of {total} warnings omitted or shortened. Inspect the update diagnostics for the complete list; selected observations retain their own freshness qualifications."));
    }
    (summary, omitted)
}

pub(super) fn retrieve(conn: &Connection, task: &str, paths: &[String]) -> Result<NativeRetrieval> {
    retrieve_hybrid(conn, task, paths, &[])
}

pub(super) fn retrieve_hybrid(
    conn: &Connection,
    task: &str,
    paths: &[String],
    semantic_hits: &[SemanticHit],
) -> Result<NativeRetrieval> {
    // Old registries remain queryable without a write or migration.
    let available: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='native_records')",
        [],
        |row| row.get(0),
    )?;
    if !available {
        return Ok(NativeRetrieval::default());
    }
    let mut report = NativeRetrieval {
        observations: imports::views(conn)?
            .into_iter()
            .map(|v| (v.id.clone(), v))
            .collect(),
        relations: imports::relationships::relations(conn)?,
        warnings: imports::warnings(conn)?,
        reviews: reviews::list(conn, true)?
            .into_iter()
            .map(|review| (review.id.clone(), review))
            .collect(),
        ..NativeRetrieval::default()
    };
    report
        .warnings
        .extend(imports::relationships::warnings(conn)?);
    (report.warnings, report.omitted_warnings) = summarize_warnings(report.warnings);
    let (terms, limited) = retrieval::query_terms_with_report(task);
    report.truncated |= limited;
    let mut candidates = BTreeMap::new();
    let (lexical, limited) = lexical_hits(conn, &terms)?;
    report.truncated |= limited;
    for (id, rank) in &lexical {
        if let Some(observation) = report.observations.get(id) {
            add_candidate(
                &mut candidates,
                id,
                5.0 + 3.0 * rank - if historical(observation) { 0.5 } else { 0.0 },
                "task keyword relevance in native source record (BM25)".into(),
            );
        }
    }
    let semantic_hits: Vec<_> = semantic_hits
        .iter()
        .filter(|hit| {
            hit.score.is_finite()
                && hit.score > 0.0
                && hit.score <= 1.0
                && report.observations.contains_key(&hit.id)
        })
        .cloned()
        .collect();
    if !semantic_hits.is_empty() {
        for (id, rank) in semantic::fuse_ranks(&lexical, &semantic_hits) {
            if let Some(observation) = report.observations.get(&id) {
                let candidate = candidates.entry(id).or_default();
                candidate.score =
                    5.0 + 3.0 * rank - if historical(observation) { 0.5 } else { 0.0 };
            }
        }
        for hit in &semantic_hits {
            candidates
                .entry(hit.id.clone())
                .or_default()
                .reasons
                .insert(
                    "semantic relevance in native source record (cosine; reciprocal rank fusion)"
                        .into(),
                );
        }
    }
    for observation in report.observations.values() {
        let record = &observation.record;
        let mut path_corpus = None;
        let historical_penalty = if historical(observation) { 0.5 } else { 0.0 };
        if mentions_identifier(task, &record.native_id)
            || mentions_identifier(task, &observation.id)
        {
            add_candidate(
                &mut candidates,
                &observation.id,
                13.0 - historical_penalty,
                format!("native record identifier: {}", record.native_id),
            );
        }
        for relationship in &record.relationships {
            if mentions_identifier(task, &relationship.target_native_id) {
                add_candidate(
                    &mut candidates,
                    &observation.id,
                    10.0 - historical_penalty,
                    format!(
                        "upstream issue or record reference: {}",
                        relationship.target_native_id
                    ),
                );
            }
        }
        for link in &record.links {
            if mentions_identifier(task, link) {
                add_candidate(
                    &mut candidates,
                    &observation.id,
                    10.0 - historical_penalty,
                    format!("upstream reference: {link}"),
                );
            }
        }
        if let Some(component) = &record.scope.component
            && mentions_identifier(task, component)
        {
            add_candidate(
                &mut candidates,
                &observation.id,
                9.0 - historical_penalty,
                format!("source-reported component: {component}"),
            );
        }
        for hint in paths {
            let hint = hint.trim().replace('\\', "/");
            let (root, path) = if hint.contains("://") {
                (None, hint.as_str())
            } else {
                hint.split_once(':')
                    .map_or((None, hint.as_str()), |(root, path)| (Some(root), path))
            };
            if root.is_some_and(|root| root != observation.import_id) {
                continue;
            }
            let path = locator_path(path);
            if path.is_empty() {
                continue;
            }
            let evidence_match = record.evidence.iter().any(|evidence| {
                let locator = locator_path(&evidence.locator);
                locator == path || locator.starts_with(&format!("{path}/"))
            });
            let mentioned = record.statement.to_lowercase().contains(&path);
            if evidence_match || mentioned {
                add_candidate(
                    &mut candidates,
                    &observation.id,
                    10.0 - historical_penalty,
                    format!("upstream evidence path: {hint}"),
                );
            } else {
                let (path_terms, limited) = retrieval::query_terms_with_report(&path);
                report.truncated |= limited;
                let corpus = path_corpus.get_or_insert_with(|| {
                    words(&format!(
                        "{} {} {} {} {} {} {}",
                        record.title,
                        record.subject,
                        record.statement,
                        record.native_id,
                        record.tags.join(" "),
                        record.links.join(" "),
                        record.scope.component.as_deref().unwrap_or("")
                    ))
                });
                if path_terms.iter().any(|term| corpus.contains(term)) {
                    add_candidate(
                        &mut candidates,
                        &observation.id,
                        4.0 - historical_penalty,
                        format!("path terminology in native source: {hint}"),
                    );
                }
            }
        }
    }
    report.truncated |= retain_best(&mut candidates, MAX_NATIVE_SEEDS);
    let seeds = ranked(&candidates);
    // Expand only original seeds. Source-owned component/topic labels are
    // retrieval anchors, never evidence of agreement or shared authority.
    for (seed_id, score) in seeds {
        let seed = &report.observations[&seed_id];
        let mut added = 0;
        for observation in report.observations.values() {
            if seed.id == observation.id
                || !compatible_scope(&seed.record.scope, &observation.record.scope)
            {
                continue;
            }
            let same_component = seed
                .record
                .scope
                .component
                .as_ref()
                .is_some_and(|component| {
                    observation
                        .record
                        .scope
                        .component
                        .as_ref()
                        .is_some_and(|other| component.eq_ignore_ascii_case(other))
                });
            let explicit_reference = seed
                .record
                .relationships
                .iter()
                .any(|r| r.target_native_id == observation.record.native_id)
                && seed.import_id == observation.import_id;
            if !same_component && !explicit_reference {
                continue;
            }
            if !candidates.contains_key(&observation.id) {
                if added >= MAX_COMPONENT_NEIGHBORS {
                    report.truncated = true;
                    continue;
                }
                added += 1;
            }
            add_candidate(
                &mut candidates,
                &observation.id,
                score * 0.65,
                if explicit_reference {
                    format!("native relationship from {seed_id}")
                } else {
                    format!("same source-reported component as {seed_id}")
                },
            );
        }
    }
    report.truncated |= retain_best(&mut candidates, MAX_NATIVE_CANDIDATES);
    for (id, candidate) in candidates {
        report.scores.insert(id.clone(), candidate.score);
        report
            .reasons
            .insert(id, candidate.reasons.into_iter().take(6).collect());
    }
    Ok(report)
}

pub(super) fn qualifications(observation: &ImportedObservation) -> Vec<String> {
    let mut qualifications = Vec::new();
    if !observation.current {
        qualifications.push("This record is absent from the latest imported snapshot; retain it as historical evidence only.".into());
    }
    if observation.current && historical(observation) {
        qualifications.push(format!("The upstream record is {}; retain its content as historical or withdrawn source material, not current guidance.", observation.record.lifecycle));
    }
    match observation.record.kind {
        ObservationKind::Implementation => qualifications.push(
            "Implementation observation reported by the upstream tool; Lore has not inspected the currently checked-out implementation.".into()),
        ObservationKind::WorkState => qualifications.push(
            "Workflow status describes the source issue only; closed or completed work does not verify implementation behavior or approve a decision.".into()),
        ObservationKind::Recollection => qualifications.push(
            "Agent recollection is reported experience; verify its applicability and outcome against current primary evidence.".into()),
        ObservationKind::Documentation => qualifications.push(
            "Imported documentation retains its upstream meaning; adoption or policy authority must not be inferred.".into()),
    }
    if observation.record.verification == ObservationVerification::UpstreamVerifiedAtRevision {
        qualifications.push("Upstream verification applies only to the recorded evidence revision. Its opaque revision token has not been compared with the current checkout.".into());
    }
    qualifications
}

pub(super) fn evidence(observation: &ImportedObservation) -> ContextImportedEvidence {
    ContextImportedEvidence {
        id: observation.evidence_id.clone(),
        observation_id: observation.id.clone(),
        snapshot_id: observation.snapshot_id.clone(),
        import_id: observation.import_id.clone(),
        origin: observation.origin,
        native_id: observation.record.native_id.clone(),
        source: observation.source_path.clone(),
        format: observation.format.clone(),
        content_hash: observation.content_hash.clone(),
        captured_at: observation.captured_at.clone(),
        observed_at: observation.record.observed_at.clone(),
        current: observation.current,
        verification: observation.record.verification,
        locators: observation.record.evidence.clone(),
        metadata: observation.record.metadata.clone(),
    }
}

pub(super) fn observation(
    observation: &ImportedObservation,
    reasons: &[String],
) -> ContextObservation {
    let record = &observation.record;
    ContextObservation {
        id: observation.id.clone(),
        import_id: observation.import_id.clone(),
        origin: observation.origin,
        native_id: record.native_id.clone(),
        kind: record.kind,
        title: record.title.clone(),
        subject: record.subject.clone(),
        statement: record.statement.clone(),
        scope: record.scope.clone(),
        lifecycle: record.lifecycle.clone(),
        verification: record.verification,
        current: observation.current,
        freshness: if !observation.current {
            "historical_import"
        } else if historical(observation) {
            "upstream_historical_or_withdrawn"
        } else if record.verification == ObservationVerification::UpstreamVerifiedAtRevision {
            "upstream_revision_not_checked_against_checkout"
        } else {
            "latest_import_not_current_behavior_verification"
        }
        .into(),
        evidence_ids: vec![observation.evidence_id.clone()],
        qualifications: qualifications(observation),
        relevance: reasons.to_vec(),
    }
}

pub(super) fn discrepancy(
    relation: &CrossSourceRelation,
    review: Option<&reviews::ReviewItem>,
) -> ContextDiscrepancy {
    let status = review.map_or("unresolved", |review| review.status.as_str());
    let mut qualifications = relation.qualifications.clone();
    if let Some(review) = review.filter(|review| review.status != "pending") {
        qualifications.push(format!("Review was {}; this disposition does not independently verify implementation or establish agreement between the sources.", review.status));
    }
    ContextDiscrepancy {
        id: relation.id.clone(),
        kind: relation.kind.clone(),
        status: status.into(),
        reason: relation.reason.clone(),
        knowledge_ids: relation
            .endpoints()
            .filter(|e| e.kind == "knowledge")
            .map(|e| e.id.clone())
            .collect(),
        observation_ids: relation
            .endpoints()
            .filter(|e| e.kind == "observation")
            .map(|e| e.id.clone())
            .collect(),
        evidence_ids: relation.evidence_ids.clone(),
        qualifications,
        review_id: relation.review_id.clone(),
    }
}

pub(super) fn populate(
    conn: &Connection,
    report: &NativeRetrieval,
    selected: &BTreeSet<String>,
    reasons: &BTreeMap<String, Vec<String>>,
    result: &mut ContextResult,
) -> Result<()> {
    let mut selected_evidence: BTreeMap<String, ContextImportedEvidence> = BTreeMap::new();
    for observation in report
        .observations
        .values()
        .filter(|observation| selected.contains(&observation.id))
    {
        // Resolve the immutable archive before publishing any normalized claim.
        imports::evidence(conn, &observation.evidence_id).map_err(|error| failure(
            "invalid_registry", format!("A selected native evidence snapshot failed validation: {error}; run lore audit."),
        ))?;
        let record = &observation.record;
        result.imported_observations.push(self::observation(
            observation,
            reasons.get(&observation.id).map_or(&[], Vec::as_slice),
        ));
        selected_evidence.insert(observation.evidence_id.clone(), evidence(observation));
        for locator in &record.evidence {
            result.suggested_inspection.push(InspectionPath {
                root_id: observation.import_id.clone(),
                path: locator.locator.clone(),
                basis: "upstream_locator_not_checkout_confirmation".into(),
                evidence_id: observation.evidence_id.clone(),
                historical: historical(observation),
            });
        }
    }
    for relation in report.relations.iter().filter(|relation| {
        selected.contains(&relation.from.id)
            && relation
                .to
                .as_ref()
                .is_none_or(|endpoint| selected.contains(&endpoint.id))
    }) {
        imports::relationships::validate_current_relation(conn, relation).map_err(|error| failure(
            "invalid_registry", format!("A selected cross-source relationship failed revision/evidence validation: {error}; run lore update and lore audit."),
        ))?;
        for evidence_id in &relation.evidence_ids {
            if selected_evidence.contains_key(evidence_id)
                || result.evidence.iter().any(|e| e.id == *evidence_id)
            {
                continue;
            }
            if evidence_id.starts_with("ne_") {
                let archived = imports::evidence(conn, evidence_id)?;
                selected_evidence.insert(evidence_id.clone(), evidence(&archived));
            } else {
                let snapshot = storage::evidence_snapshot(conn, evidence_id)?;
                if snapshot.excerpt.is_empty() || util::digest(&snapshot.excerpt) != snapshot.digest
                {
                    return Err(failure(
                        "invalid_registry",
                        "A selected cross-source evidence snapshot failed its integrity check; run lore audit.",
                    ));
                }
                let current: bool = conn.query_row(
                    "SELECT EXISTS(SELECT 1 FROM source_current WHERE source_id=?1 AND source_revision_id=?2)",
                    [&snapshot.source_id, &snapshot.source_revision_id], |row| row.get(0),
                )?;
                result.evidence.push(ContextEvidence {
                    id: evidence_id.clone(),
                    source: format!("{}:{}", snapshot.root_id, snapshot.path),
                    source_revision_id: snapshot.source_revision_id,
                    material: snapshot.material,
                    origin: snapshot.origin,
                    provenance_recorded: snapshot.root_path.is_some(),
                    current,
                    excerpt: snapshot.excerpt,
                    line_start: snapshot.line_start,
                    line_end: snapshot.line_end,
                });
            }
        }
        result.cross_source_relations.push(relation.clone());
        if matches!(
            relation.kind.as_str(),
            "potential_discrepancy" | "uncertain" | "verification_question"
        ) {
            let endpoints: Vec<_> = std::iter::once(&relation.from)
                .chain(relation.to.as_ref())
                .collect();
            let review = relation
                .review_id
                .as_ref()
                .and_then(|id| report.reviews.get(id));
            result.discrepancies.push(discrepancy(relation, review));
            if let Some(review) = review.filter(|review| review.status == "pending")
                && !result
                    .reviews
                    .iter()
                    .any(|existing| existing.id == review.id)
            {
                result.reviews.push(ContextReview {
                    id: review.id.clone(),
                    reason: review.reason.clone(),
                    knowledge_ids: endpoints
                        .iter()
                        .filter(|endpoint| endpoint.kind == "knowledge")
                        .map(|endpoint| endpoint.id.clone())
                        .collect(),
                    evidence_ids: relation.evidence_ids.clone(),
                });
            }
            result.recommended_verification.push(ContextVerification {
                action: if relation.kind == "verification_question" {
                    "Verify the reported outcome against current implementation or primary evidence before relying on the issue status or recollection."
                } else {
                    "Compare the cited intent and source observations, confirm their scope and current revisions, and check for an explicit policy replacement before changing behavior."
                }.into(),
                record_ids: endpoints.iter().map(|e| e.id.clone()).collect(),
                evidence_ids: relation.evidence_ids.clone(), relationship_id: Some(relation.id.clone()),
            });
        }
    }
    for observation in &result.imported_observations {
        let covered = result
            .recommended_verification
            .iter()
            .any(|item| item.record_ids.contains(&observation.id));
        if !covered
            && (observation.kind != ObservationKind::Documentation
                || !observation.current
                || observation.freshness == "upstream_historical_or_withdrawn")
        {
            result.recommended_verification.push(ContextVerification {
                action: if observation.kind == ObservationKind::Implementation {
                    "Inspect the cited implementation in the current checkout; upstream verification only applies to its recorded evidence revision."
                } else {
                    "Check this source report against current primary evidence before applying it to the change."
                }.into(),
                record_ids: vec![observation.id.clone()], evidence_ids: observation.evidence_ids.clone(),
                relationship_id: None,
            });
        }
    }
    result.imported_evidence = selected_evidence.into_values().collect();
    result.suggested_inspection.sort();
    result.suggested_inspection.dedup();
    if result.imported_observations.iter().any(|observation| {
        !observation.current || observation.freshness == "upstream_historical_or_withdrawn"
    }) {
        result.warnings.push("Historical native records are included. They are absent from the latest import or marked historical/withdrawn upstream and do not establish current applicability.".into());
    }
    if result
        .imported_observations
        .iter()
        .any(|observation| observation.kind == ObservationKind::Implementation)
    {
        result.warnings.push("Implementation freshness is unverified against the current checkout. Upstream verification and source revisions refer to the imported evidence only.".into());
    }
    result.warnings.extend(report.warnings.iter().cloned());
    Ok(())
}

pub(super) fn render(result: &ContextResult, text: &mut String) {
    for (heading, kind) in [
        ("Observed implementation", ObservationKind::Implementation),
        ("Related work history", ObservationKind::WorkState),
        ("Related agent observations", ObservationKind::Recollection),
        ("Imported documentation", ObservationKind::Documentation),
    ] {
        let observations: Vec<_> = result
            .imported_observations
            .iter()
            .filter(|o| o.kind == kind)
            .collect();
        if observations.is_empty() {
            continue;
        }
        text.push_str(&format!("## {heading}\n\n"));
        for observation in observations {
            text.push_str(&format!(
                "- {} ({} / {}: {}; lifecycle: {}) [{}]\n",
                display_text(&observation.statement),
                observation.origin.as_str(),
                display_text(&observation.import_id),
                display_text(&observation.native_id),
                display_text(&observation.lifecycle),
                observation.id
            ));
            for qualification in &observation.qualifications {
                text.push_str(&format!("  {}\n", display_text(qualification)));
            }
            text.push_str(&format!(
                "  Evidence: {}\n",
                observation.evidence_ids.join(", ")
            ));
            let scope: Vec<_> = [
                ("repository", observation.scope.repository.as_ref()),
                ("component", observation.scope.component.as_ref()),
                ("environment", observation.scope.environment.as_ref()),
            ]
            .into_iter()
            .filter_map(|(label, value)| {
                value.map(|value| format!("{label}: {}", display_text(value)))
            })
            .collect();
            if !scope.is_empty() {
                text.push_str(&format!("  Source-reported scope: {}\n", scope.join(", ")));
            }
        }
        text.push('\n');
    }
    if !result.discrepancies.is_empty() {
        text.push_str("## Potential discrepancies and verification questions\n\n");
        for discrepancy in &result.discrepancies {
            text.push_str(&format!(
                "- {} ({}; {}) [{}]\n",
                display_text(&discrepancy.reason),
                discrepancy.kind,
                discrepancy.status,
                discrepancy.id
            ));
            for qualification in &discrepancy.qualifications {
                text.push_str(&format!("  {}\n", display_text(qualification)));
            }
            text.push_str(&format!(
                "  Evidence: {}\n",
                discrepancy.evidence_ids.join(", ")
            ));
        }
        text.push('\n');
    }
    let relationships: Vec<_> = result
        .cross_source_relations
        .iter()
        .filter(|r| {
            !matches!(
                r.kind.as_str(),
                "potential_discrepancy" | "uncertain" | "verification_question"
            )
        })
        .collect();
    if !relationships.is_empty() {
        text.push_str("## Recorded cross-source relationships\n\n");
        for relation in relationships {
            text.push_str(&format!(
                "- {} {} {} [{}]: {}\n  Evidence: {}\n",
                relation.from.id,
                display_text(&relation.kind),
                relation.to.as_ref().map_or("", |e| e.id.as_str()),
                relation.id,
                display_text(&relation.reason),
                relation.evidence_ids.join(", ")
            ));
            for qualification in &relation.qualifications {
                text.push_str(&format!("  {}\n", display_text(qualification)));
            }
            if let Some(kind) = &relation.upstream_kind {
                text.push_str(&format!(
                    "  Upstream relationship: {}{}{}\n",
                    display_text(kind),
                    relation
                        .upstream_status
                        .as_ref()
                        .map_or(String::new(), |status| format!(
                            "; status: {}",
                            display_text(status)
                        )),
                    relation.upstream_active.map_or("", |active| if active {
                        "; asserted upstream"
                    } else {
                        "; not currently asserted upstream"
                    })
                ));
            }
        }
        text.push('\n');
    }
    if !result.recommended_verification.is_empty() {
        text.push_str("## Recommended verification\n\n");
        for verification in &result.recommended_verification {
            text.push_str(&format!(
                "- {}\n  Records: {}. Evidence: {}\n",
                display_text(&verification.action),
                verification.record_ids.join(", "),
                verification.evidence_ids.join(", ")
            ));
        }
        text.push('\n');
    }
    if !result.imported_evidence.is_empty() {
        text.push_str("## Imported evidence snapshots\n\n");
        for evidence in &result.imported_evidence {
            text.push_str(&format!(
                "- [{}] {} / {} (snapshot {}; {})\n  Content hash: {}\n",
                evidence.id,
                display_text(&evidence.import_id),
                display_text(&evidence.native_id),
                evidence.snapshot_id,
                if evidence.current {
                    "latest imported record"
                } else {
                    "historical imported record"
                },
                evidence.content_hash
            ));
            for locator in &evidence.locators {
                text.push_str(&format!(
                    "  {}{}{}\n",
                    display_text(&locator.locator),
                    locator
                        .revision
                        .as_ref()
                        .map_or(String::new(), |revision| format!(
                            "; upstream revision: {}",
                            display_text(revision)
                        )),
                    locator
                        .field
                        .as_ref()
                        .map_or(String::new(), |field| format!(
                            "; native field: {}",
                            display_text(field)
                        ))
                ));
            }
            text.push_str(&format!(
                "  Captured: {}{}\n",
                display_text(&evidence.captured_at),
                evidence
                    .observed_at
                    .as_ref()
                    .map_or(String::new(), |at| format!(
                        "; upstream observation: {}",
                        display_text(at)
                    ))
            ));
        }
        text.push_str("Resolve complete native records with: lore evidence <evidence-id>\n\n");
    }
}

#[cfg(test)]
mod tests {
    use super::mentions_identifier;

    #[test]
    fn identifiers_are_complete_and_unicode_safe() {
        assert!(mentions_identifier("Implement PAY-17, then test", "PAY-17"));
        assert!(mentions_identifier("Inspect (pay-17)", "PAY-17"));
        assert!(!mentions_identifier("Implement PAY-170", "PAY-17"));
        assert!(!mentions_identifier("Implement MY-PAY-17", "PAY-17"));
        assert!(mentions_identifier("Endre øvelse-17 nå", "øvelse-17"));
        assert!(!mentions_identifier("Endre øvelse-170 nå", "øvelse-17"));
    }
}
