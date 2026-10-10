//! Explicit project baselines, consequential change views and advisory review.
//! Baselines are source-bound snapshots, not a second authoritative registry.

mod interpretations;
pub use interpretations::{CurrentInterpretationReview, InterpretationChange, InterpretationState};

use crate::{
    config::ResolvedConfig,
    context::{
        self, ContextBudget, ContextOptions,
        adaptive::{self, AdaptiveResult, SnapshotManifest},
        decision::{
            GenerationBasis,
            runtime::{DecisionContextResult, RunOptions},
        },
        intelligence::{RiskCategory, Severity},
    },
    domain::KnowledgeView,
    imports::{self, ImportedObservation},
    storage, util,
};
use anyhow::{Context, Result, ensure};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::PathBuf,
};

const MAX_BASELINE_BYTES: usize = 8_000_000;
const MAX_BASELINE_RECORDS: usize = 10_000;
const MAX_BASELINES: usize = 16;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RevisionState {
    pub id: String,
    pub revision_id: String,
    pub statement: String,
    pub kind: String,
    pub lifecycle: String,
    pub original_lifecycle: String,
    pub support_state: String,
    pub topic: String,
    pub subject: String,
    pub scope: String,
    pub effective_at: String,
    pub evidence_ids: Vec<String>,
}

impl RevisionState {
    fn capture(conn: &Connection, view: &KnowledgeView) -> Result<Self> {
        Ok(Self {
            id: view.id.clone(),
            revision_id: view.revision_id.clone(),
            statement: view.statement.clone(),
            kind: view.kind.clone(),
            lifecycle: view.lifecycle.clone(),
            original_lifecycle: view.base_lifecycle.clone(),
            support_state: view.support_state.clone(),
            topic: view.topic.clone(),
            subject: view.subject.clone(),
            scope: view.scope.clone(),
            effective_at: view.effective_at.clone(),
            evidence_ids: revision_evidence(conn, &view.revision_id)?,
        })
    }
}

fn revision_evidence(conn: &Connection, revision: &str) -> Result<Vec<String>> {
    Ok(conn.prepare("SELECT DISTINCT e.evidence_id FROM knowledge_support s JOIN assertion_evidence e ON e.assertion_revision_id=s.assertion_revision_id WHERE s.knowledge_revision_id=?1 ORDER BY e.evidence_id")?
        .query_map([revision], |row| row.get(0))?.collect::<rusqlite::Result<_>>()?)
}

fn semantic_key(conn: &Connection, record: &RevisionState) -> Result<String> {
    let mut support = BTreeSet::new();
    for id in &record.evidence_ids {
        let evidence = storage::evidence_snapshot(conn, id)?;
        ensure!(
            evidence.digest == util::digest(&evidence.excerpt),
            "evidence digest does not match retained source text"
        );
        // A new capture/location alone is cosmetic. Exact quoted conditions or
        // primary/derived provenance changes affect the source meaning even if
        // an extracted statement was reconciled to the same knowledge record.
        support.insert(util::json_digest(&(
            &evidence.excerpt,
            &evidence.material,
            &evidence.origin,
        ))?);
    }
    util::json_digest(&(
        &record.statement,
        &record.kind,
        &record.lifecycle,
        &record.original_lifecycle,
        &record.support_state,
        &record.topic,
        &record.subject,
        &record.scope,
        &record.effective_at,
        support,
    ))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ImportedRevision {
    pub id: String,
    pub snapshot_id: String,
    pub evidence_id: String,
    /// Source-current at the selected checkpoint, not current runtime behavior.
    pub current: bool,
}

fn imported_revisions(conn: &Connection) -> Result<BTreeMap<String, ImportedRevision>> {
    let available: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='native_latest')",
        [],
        |row| row.get(0),
    )?;
    if !available {
        return Ok(BTreeMap::new());
    }
    let count: usize =
        conn.query_row("SELECT COUNT(*) FROM native_latest", [], |row| row.get(0))?;
    ensure!(
        count <= MAX_BASELINE_RECORDS,
        "project exceeds the complete imported baseline record limit"
    );
    Ok(conn.prepare("SELECT l.observation_id,l.snapshot_id,s.evidence_id,EXISTS(SELECT 1 FROM native_current c WHERE c.observation_id=l.observation_id AND c.snapshot_id=l.snapshot_id) FROM native_latest l JOIN native_snapshots s ON s.id=l.snapshot_id ORDER BY l.observation_id")?
        .query_map([], |row| {
            let id: String = row.get(0)?;
            Ok((id.clone(), ImportedRevision { id, snapshot_id: row.get(1)?, evidence_id: row.get(2)?, current: row.get(3)? }))
        })?.collect::<rusqlite::Result<_>>()?)
}

fn check_native_bytes(conn: &Connection, ids: &BTreeSet<String>) -> Result<()> {
    let mut bytes = 0usize;
    for id in ids {
        let size: usize = conn.query_row(
            "SELECT length(CAST(record_json AS BLOB)) FROM native_snapshots WHERE evidence_id=?1",
            [id],
            |row| row.get(0),
        )?;
        bytes = bytes.saturating_add(size);
        ensure!(
            bytes <= MAX_BASELINE_BYTES,
            "complete imported source evidence exceeds its byte budget"
        );
    }
    Ok(())
}

fn native_evidence(conn: &Connection, ids: &BTreeSet<String>) -> Result<Vec<ImportedObservation>> {
    check_native_bytes(conn, ids)?;
    ids.iter().map(|id| imports::evidence(conn, id)).collect()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Baseline {
    pub schema_version: u32,
    pub name: String,
    pub captured_at: String,
    pub snapshot: SnapshotManifest,
    pub source_scope: String,
    pub records: BTreeMap<String, RevisionState>,
    pub relationships: BTreeMap<String, storage::RelationFact>,
    pub imported: BTreeMap<String, ImportedRevision>,
    /// Latest source-owned interpretation IDs and immutable content hashes.
    pub cross_source_relationships: BTreeMap<String, String>,
    pub content_hash: String,
}

fn source_scope(config: &ResolvedConfig) -> Result<String> {
    util::json_digest(&(
        &config.project_id,
        &config.roots,
        &config.imports,
        &config.config.sources,
        &config.config.imports,
    ))
}

fn baseline_path(config: &ResolvedConfig, name: &str) -> Result<PathBuf> {
    ensure!(
        !name.is_empty()
            && name.len() <= 64
            && name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
        "baseline name must contain 1..64 letters, digits, hyphens or underscores"
    );
    Ok(config.state.join("baselines").join(format!("{name}.json")))
}

fn baseline_hash(baseline: &Baseline) -> Result<String> {
    util::json_digest(&(
        baseline.schema_version,
        &baseline.name,
        &baseline.captured_at,
        &baseline.snapshot,
        &baseline.source_scope,
        &baseline.records,
        &baseline.relationships,
        &baseline.imported,
        &baseline.cross_source_relationships,
    ))
}

fn relationships(conn: &Connection) -> Result<BTreeMap<String, storage::RelationFact>> {
    let count: usize = conn.query_row("SELECT (SELECT COUNT(*) FROM knowledge_relations) + (SELECT COUNT(*) FROM reaffirmation_links)", [], |row| row.get(0))?;
    ensure!(
        count <= 20_000,
        "project exceeds the complete baseline relationship limit"
    );
    Ok(storage::relation_facts(conn)?
        .into_iter()
        .map(|relation| (relation.id.clone(), relation))
        .collect())
}

fn cross_source_manifest(conn: &Connection, history: bool) -> Result<BTreeMap<String, String>> {
    let available: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='cross_source_relations')", [], |row| row.get(0))?;
    if !available {
        return Ok(BTreeMap::new());
    }
    let (count, bytes): (usize, usize) = conn.query_row("SELECT COUNT(*),COALESCE(sum(length(CAST(payload_json AS BLOB))),0) FROM cross_source_relations", [], |row| Ok((row.get(0)?, row.get(1)?)))?;
    ensure!(
        count <= 20_000 && bytes <= MAX_BASELINE_BYTES,
        "project exceeds the complete cross-source relationship record/byte limit"
    );
    let records = if history {
        imports::relationships::history(conn)?
    } else {
        imports::relationships::relations(conn)?
    };
    records
        .into_iter()
        .map(|record| Ok((record.id.clone(), interpretations::content_hash(&record)?)))
        .collect()
}

// Keep all projections in one SQLite read snapshot, including on early return.
// Dropping an asynchronous guardian request also releases its read transaction.
struct ReadSnapshot<'a>(&'a Connection);
impl<'a> ReadSnapshot<'a> {
    fn begin(conn: &'a Connection) -> Result<Self> {
        conn.execute_batch("SAVEPOINT lore_companion_read")?;
        Ok(Self(conn))
    }
}
impl Drop for ReadSnapshot<'_> {
    fn drop(&mut self) {
        let _ = self.0.execute_batch("RELEASE lore_companion_read");
    }
}

pub fn save_baseline(
    config: &ResolvedConfig,
    conn: &Connection,
    name: &str,
    replace: bool,
) -> Result<Baseline> {
    let _snapshot = ReadSnapshot::begin(conn)?;
    let path = baseline_path(config, name)?;
    util::reject_symlinks(&path)?;
    ensure!(
        replace || !path.exists(),
        "baseline already exists; use --replace to explicitly replace it"
    );
    let parent = path.parent().context("baseline directory unavailable")?;
    util::private_dir(parent)?;
    let entries = fs::read_dir(parent)?
        .take(MAX_BASELINES + 1)
        .collect::<std::io::Result<Vec<_>>>()?;
    ensure!(
        path.exists() || entries.len() < MAX_BASELINES,
        "baseline retention limit reached; remove an obsolete named baseline first"
    );
    check_record_limit(conn)?;
    let records = storage::views(conn)?;
    ensure!(
        records.len() <= MAX_BASELINE_RECORDS,
        "project exceeds the complete baseline record limit"
    );
    let mut baseline = Baseline {
        schema_version: 1,
        name: name.into(),
        captured_at: util::now(),
        snapshot: SnapshotManifest::capture(config, conn)?,
        source_scope: source_scope(config)?,
        records: records
            .iter()
            .map(|record| Ok((record.id.clone(), RevisionState::capture(conn, record)?)))
            .collect::<Result<_>>()?,
        relationships: relationships(conn)?,
        imported: imported_revisions(conn)?,
        cross_source_relationships: cross_source_manifest(conn, false)?,
        content_hash: String::new(),
    };
    native_evidence(
        conn,
        &baseline
            .imported
            .values()
            .map(|record| record.evidence_id.clone())
            .collect(),
    )?;
    baseline.content_hash = baseline_hash(&baseline)?;
    let bytes = serde_json::to_vec(&baseline)?;
    ensure!(
        bytes.len() <= MAX_BASELINE_BYTES,
        "complete baseline exceeds its byte limit"
    );
    util::atomic_write(&path, &bytes)?;
    Ok(baseline)
}

pub fn remove_baseline(config: &ResolvedConfig, name: &str) -> Result<()> {
    let path = baseline_path(config, name)?;
    util::reject_symlinks(&path)?;
    fs::remove_file(path).context("remove named baseline")
}

fn load_baseline(config: &ResolvedConfig, conn: &Connection, name: &str) -> Result<Baseline> {
    let baseline: Baseline = serde_json::from_str(&util::read_limited(
        &baseline_path(config, name)?,
        MAX_BASELINE_BYTES,
    )?)?;
    ensure!(
        baseline.schema_version == 1
            && baseline.name == name
            && baseline.snapshot.project_id == config.project_id
            && baseline.source_scope == source_scope(config)?
            && baseline.records.len() <= MAX_BASELINE_RECORDS
            && baseline.imported.len() <= MAX_BASELINE_RECORDS
            && baseline.relationships.len() <= 20_000
            && baseline.cross_source_relationships.len() <= 20_000
            && baseline.content_hash == baseline_hash(&baseline)?,
        "baseline is corrupt, belongs to another project, or has different source permissions"
    );
    // Validate saved statements against immutable registry revisions; a
    // recomputed file checksum cannot turn an edited baseline into history.
    for (id, record) in &baseline.records {
        let valid: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM knowledge_revisions r JOIN knowledge_details d ON d.knowledge_id=r.knowledge_id JOIN topics t ON t.id=d.topic_id WHERE r.id=?1 AND r.knowledge_id=?2 AND r.statement=?3 AND r.kind=?4 AND r.lifecycle=?5 AND r.support_state=?6 AND t.slug=?7 AND d.subject=?8 AND d.scope=?9 AND d.effective_at=?10 AND d.base_lifecycle=?11)",
            rusqlite::params![record.revision_id, id, record.statement, record.kind, record.lifecycle, record.support_state, record.topic, record.subject, record.scope, record.effective_at, record.original_lifecycle],
            |row| row.get(0),
        )?;
        ensure!(
            record.id == *id && valid,
            "baseline no longer resolves to immutable project history"
        );
        ensure!(
            record.evidence_ids == revision_evidence(conn, &record.revision_id)?,
            "baseline omits or substitutes evidence from its immutable knowledge revision"
        );
        for evidence in &record.evidence_ids {
            let belongs: bool = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM assertion_assignments a JOIN assertion_evidence e ON e.assertion_revision_id=a.assertion_revision_id WHERE a.knowledge_id=?1 AND e.evidence_id=?2)",
                rusqlite::params![id, evidence], |row| row.get(0),
            )?;
            ensure!(
                belongs,
                "baseline contains evidence from another knowledge record"
            );
            let snapshot = storage::evidence_snapshot(conn, evidence)?;
            ensure!(
                snapshot.digest == util::digest(&snapshot.excerpt),
                "baseline evidence digest mismatch"
            );
        }
    }
    let native_ids = baseline
        .imported
        .values()
        .map(|record| record.evidence_id.clone())
        .collect();
    let original_imports = native_evidence(conn, &native_ids)?;
    for (id, recorded) in &baseline.imported {
        let original = original_imports
            .iter()
            .find(|record| record.evidence_id == recorded.evidence_id)
            .context("baseline native evidence is unavailable")?;
        ensure!(
            id == &recorded.id
                && original.id == *id
                && original.snapshot_id == recorded.snapshot_id,
            "baseline imported observation does not resolve to its immutable native snapshot"
        );
    }
    let known_relations = relationships(conn)?;
    for (id, recorded) in &baseline.relationships {
        let current = known_relations
            .get(id)
            .context("baseline relationship no longer resolves to project history")?;
        let mut compared = recorded.clone();
        // Active status is a snapshot projection. All documentary identity and
        // content must still equal the retained immutable relationship.
        compared.active = current.active;
        ensure!(
            id == &recorded.id && util::json_digest(&compared)? == util::json_digest(current)?,
            "baseline contains an altered documentary relationship"
        );
    }
    let known_cross_source = cross_source_manifest(conn, true)?;
    ensure!(
        baseline
            .cross_source_relationships
            .iter()
            .all(|(id, digest)| known_cross_source.get(id) == Some(digest)),
        "baseline cross-source relationship does not resolve to immutable retained interpretation history"
    );
    Ok(baseline)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectChange {
    pub knowledge_id: String,
    pub change_kind: String,
    pub before: Option<RevisionState>,
    pub after: Option<RevisionState>,
    pub affected_knowledge_ids: Vec<String>,
    pub implication: String,
    pub implication_basis: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeReport {
    pub schema_version: u32,
    pub baseline: String,
    pub baseline_snapshot: SnapshotManifest,
    pub snapshot: SnapshotManifest,
    pub task: Option<String>,
    pub registry_changed: bool,
    /// Task filtering uses bounded shared retrieval; absence is not proof that
    /// no relevant change exists when that retrieval omitted candidates.
    pub retrieval_truncated: bool,
    /// Project-wide membership signal, even when task filtering or the output
    /// budget omits some complete historical interpretation changes.
    pub cross_source_relationships_changed: bool,
    pub changes: Vec<ProjectChange>,
    pub relationship_changes: Vec<RelationshipChange>,
    pub imported_changes: Vec<ImportedChange>,
    pub cross_source_changes: Vec<InterpretationChange>,
    /// Exact documentary endpoint revisions used by interpretation history.
    /// Several revisions can share one stable knowledge identity.
    pub cross_source_knowledge: Vec<RevisionState>,
    pub related_knowledge: Vec<RevisionState>,
    pub evidence: Vec<storage::EvidenceSnapshot>,
    pub imported_evidence: Vec<ImportedObservation>,
    pub omitted_changes: usize,
    pub omitted_relationship_changes: usize,
    pub omitted_imported_changes: usize,
    pub omitted_cross_source_changes: usize,
    pub generation_basis: String,
    pub live_checkout_assessed: bool,
    pub budget: ContextBudget,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportedChange {
    pub observation_id: String,
    pub before: Option<ImportedRevision>,
    pub after: Option<ImportedRevision>,
    pub qualification: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationshipChange {
    pub relationship_id: String,
    pub before: Option<storage::RelationFact>,
    pub after: Option<storage::RelationFact>,
}

#[derive(Debug, Clone)]
pub struct ChangeOptions {
    pub since: String,
    pub task: Option<String>,
    pub max_tokens: usize,
}

fn impact(record: &RevisionState) -> String {
    if matches!(
        record.support_state.as_str(),
        "unsupported" | "historical_only"
    ) {
        "Current documentary support was lost; earlier advice that required this record needs revalidation.".into()
    } else if matches!(record.lifecycle.as_str(), "superseded" | "rejected") {
        "Earlier guidance must preserve this historical status rather than reuse the record as current policy.".into()
    } else if record.lifecycle == "accepted"
        && matches!(record.kind.as_str(), "constraint" | "decision")
    {
        "This accepted decision or constraint changes the evidence available for subsequent task guidance; apply its recorded scope and exceptions.".into()
    } else if matches!(record.lifecycle.as_str(), "proposed" | "planned") {
        "This is future intent and does not authorize changing accepted behavior.".into()
    } else {
        "This changes documented project understanding; it is not independent evidence of deployed behavior.".into()
    }
}

pub fn changes(
    config: &ResolvedConfig,
    conn: &Connection,
    options: &ChangeOptions,
) -> Result<ChangeReport> {
    let _snapshot = ReadSnapshot::begin(conn)?;
    ensure!(
        (512..=context::MAX_MAX_TOKENS).contains(&options.max_tokens),
        "changes --max-tokens must be 512..100000"
    );
    check_record_limit(conn)?;
    let baseline = load_baseline(config, conn, &options.since)?;
    let snapshot = SnapshotManifest::capture(config, conn)?;
    let current: BTreeMap<_, _> = storage::views(conn)?
        .iter()
        .map(|record| Ok((record.id.clone(), RevisionState::capture(conn, record)?)))
        .collect::<Result<_>>()?;
    let (relevant, relevant_imported, relevance_truncated) = if let Some(task) = &options.task {
        let selected = context::build_context(
            conn,
            &ContextOptions {
                task: task.clone(),
                paths: vec![],
                max_tokens: 100_000,
            },
        )?;
        (
            Some(
                selected
                    .sections
                    .items()
                    .map(|item| item.id.clone())
                    .collect::<BTreeSet<_>>(),
            ),
            Some(
                selected
                    .imported_observations
                    .iter()
                    .map(|item| item.id.clone())
                    .collect::<BTreeSet<_>>(),
            ),
            selected.retrieval_truncated
                || selected.omissions.knowledge_units > 0
                || selected.omissions.critical_groups > 0
                || selected.omissions.unsupported_units > 0
                || selected.omissions.imported_observations > 0
                || selected.omissions.discrepancies > 0
                || selected.omissions.source_warnings > 0,
        )
    } else {
        (None, None, false)
    };
    let relations = storage::relation_facts(conn)?;
    let mut changes = Vec::new();
    let ids: BTreeSet<_> = baseline
        .records
        .keys()
        .chain(current.keys())
        .cloned()
        .collect();
    for id in ids {
        if relevant.as_ref().is_some_and(|set| !set.contains(&id)) {
            continue;
        }
        let before = baseline.records.get(&id);
        let after = current.get(&id);
        let unchanged = match (before, after) {
            (Some(a), Some(b)) => semantic_key(conn, a)? == semantic_key(conn, b)?,
            _ => false,
        };
        if unchanged {
            continue;
        }
        let state = after.or(before).context("changed knowledge has no state")?;
        let kind = if before.is_none() {
            "added"
        } else if after.is_none() {
            "removed"
        } else if before.map(|v| &v.lifecycle) != after.map(|v| &v.lifecycle) {
            "lifecycle_changed"
        } else {
            "understanding_changed"
        };
        let affected = relations
            .iter()
            .filter(|relation| relation.active && (relation.from == id || relation.to == id))
            .flat_map(|relation| [relation.from.clone(), relation.to.clone()])
            .filter(|other| other != &id)
            .collect::<BTreeSet<_>>();
        changes.push(ProjectChange {
            knowledge_id: id,
            change_kind: kind.into(),
            before: before.cloned(),
            after: after.cloned(),
            affected_knowledge_ids: affected.into_iter().collect(),
            implication: impact(state),
            implication_basis: "inferred_from_documented_change".into(),
        });
    }
    changes.sort_by_key(|change| {
        let state = change.after.as_ref().or(change.before.as_ref()).unwrap();
        (
            !matches!(state.kind.as_str(), "constraint" | "decision"),
            state.topic.clone(),
            change.knowledge_id.clone(),
        )
    });
    let current_relations = relationships(conn)?;
    let mut relationship_changes = Vec::new();
    for id in baseline
        .relationships
        .keys()
        .chain(current_relations.keys())
        .collect::<BTreeSet<_>>()
    {
        let before = baseline.relationships.get(id);
        let after = current_relations.get(id);
        let state = after.or(before).context("missing relationship state")?;
        if relevant
            .as_ref()
            .is_some_and(|set| !set.contains(&state.from) && !set.contains(&state.to))
        {
            continue;
        }
        if util::json_digest(&before)? != util::json_digest(&after)? {
            relationship_changes.push(RelationshipChange {
                relationship_id: id.clone(),
                before: before.cloned(),
                after: after.cloned(),
            });
        }
    }
    let current_imported = imported_revisions(conn)?;
    let mut imported_changes = Vec::new();
    for id in baseline
        .imported
        .keys()
        .chain(current_imported.keys())
        .collect::<BTreeSet<_>>()
    {
        if relevant_imported
            .as_ref()
            .is_some_and(|set| !set.contains(id))
        {
            continue;
        }
        let before = baseline.imported.get(id);
        let after = current_imported.get(id);
        if before != after {
            imported_changes.push(ImportedChange { observation_id: id.clone(),
                before: before.cloned(), after: after.cloned(),
                qualification: "A changed source-owned imported observation. Native lifecycle, verification and scope retain their original meanings; this is not independent evidence of code behavior, deployment or execution by Lore.".into() });
        }
    }
    let current_cross_source = cross_source_manifest(conn, false)?;
    let mut interpretation_history = interpretations::InterpretationHistory::read(
        conn,
        &baseline,
        &current_cross_source,
        &current_imported,
        relevant.as_ref(),
        relevant_imported.as_ref(),
    )?;
    let mut result = ChangeReport {
        schema_version: 1,
        baseline: options.since.clone(),
        registry_changed: baseline.snapshot.registry_revision != snapshot.registry_revision,
        retrieval_truncated: relevance_truncated,
        cross_source_relationships_changed: baseline.cross_source_relationships
            != current_cross_source,
        baseline_snapshot: baseline.snapshot,
        snapshot,
        task: options.task.clone(),
        changes,
        relationship_changes,
        imported_changes,
        cross_source_changes: std::mem::take(&mut interpretation_history.changes),
        cross_source_knowledge: Vec::new(),
        related_knowledge: Vec::new(),
        evidence: Vec::new(),
        imported_evidence: Vec::new(),
        omitted_changes: 0,
        omitted_relationship_changes: 0,
        omitted_imported_changes: 0,
        omitted_cross_source_changes: 0,
        generation_basis: "documentary_comparison".into(),
        live_checkout_assessed: false,
        budget: ContextBudget {
            max_tokens: options.max_tokens,
            used_tokens: 0,
            tokenizer: "cl100k_base".into(),
        },
    };
    loop {
        let mut related_ids: BTreeSet<_> = result
            .relationship_changes
            .iter()
            .flat_map(|change| change.before.iter().chain(change.after.iter()))
            .flat_map(|relation| [relation.from.clone(), relation.to.clone()])
            .collect();
        related_ids.extend(
            result
                .changes
                .iter()
                .flat_map(|change| change.affected_knowledge_ids.iter().cloned()),
        );
        result.related_knowledge = related_ids
            .iter()
            .filter_map(|id| current.get(id).or_else(|| baseline.records.get(id)))
            .cloned()
            .collect();
        let mut evidence_ids: BTreeSet<_> = result
            .changes
            .iter()
            .flat_map(|change| change.before.iter().chain(change.after.iter()))
            .chain(result.related_knowledge.iter())
            .flat_map(|state| state.evidence_ids.iter().cloned())
            .collect();
        evidence_ids.extend(
            result
                .relationship_changes
                .iter()
                .flat_map(|change| change.before.iter().chain(change.after.iter()))
                .map(|relation| relation.evidence_id.clone()),
        );
        result.evidence = evidence_ids
            .iter()
            .map(|id| storage::evidence_snapshot(conn, id))
            .collect::<Result<_>>()?;
        for evidence in &result.evidence {
            ensure!(
                evidence.digest == util::digest(&evidence.excerpt),
                "change evidence digest mismatch"
            );
        }
        let imported_ids: BTreeSet<_> = result
            .imported_changes
            .iter()
            .flat_map(|change| change.before.iter().chain(change.after.iter()))
            .map(|state| state.evidence_id.clone())
            .collect();
        result.imported_evidence = native_evidence(conn, &imported_ids)?;
        interpretation_history.populate(&mut result)?;
        result.budget.used_tokens = 0;
        for _ in 0..8 {
            let used = context::count_tokens(&(serde_json::to_string(&result)? + "\n"))
                .max(context::count_tokens(&render_changes(&result)))
                .max(result.budget.used_tokens);
            if used == result.budget.used_tokens {
                break;
            }
            result.budget.used_tokens = used;
        }
        if result.budget.used_tokens <= options.max_tokens {
            break;
        }
        if !result.cross_source_changes.is_empty() {
            result.omitted_cross_source_changes += trim_complete(&mut result.cross_source_changes);
        } else if !result.imported_changes.is_empty() {
            result.omitted_imported_changes += trim_complete(&mut result.imported_changes);
        } else if !result.changes.is_empty() {
            result.omitted_changes += trim_complete(&mut result.changes);
        } else if !result.relationship_changes.is_empty() {
            result.omitted_relationship_changes += trim_complete(&mut result.relationship_changes);
        } else {
            anyhow::bail!("complete change metadata exceeds --max-tokens");
        }
    }
    Ok(result)
}

/// Large comparisons shrink geometrically before small final adjustments.
/// Rebuilding and tokenizing a 10000-record report once per removed item would
/// turn the output budget into quadratic work. Every removed item stays whole.
fn trim_complete<T>(values: &mut Vec<T>) -> usize {
    let count = if values.len() > 32 {
        values.len() / 2
    } else {
        1
    };
    values.truncate(values.len().saturating_sub(count));
    count
}

fn check_record_limit(conn: &Connection) -> Result<()> {
    let count: usize = conn.query_row("SELECT COUNT(*) FROM knowledge_current", [], |row| {
        row.get(0)
    })?;
    ensure!(
        count <= MAX_BASELINE_RECORDS,
        "project exceeds the complete baseline record limit"
    );
    let bytes: usize = conn.query_row(
        "SELECT COALESCE(sum(length(CAST(e.exact_excerpt AS BLOB))+length(CAST(e.context_before AS BLOB))+length(CAST(e.context_after AS BLOB))),0) FROM knowledge_current c JOIN knowledge_support s ON s.knowledge_revision_id=c.revision_id JOIN assertion_evidence a ON a.assertion_revision_id=s.assertion_revision_id JOIN evidence_snapshots e ON e.id=a.evidence_id",
        [], |row| row.get(0),
    )?;
    ensure!(
        bytes <= MAX_BASELINE_BYTES,
        "project exceeds the complete baseline source-evidence byte limit"
    );
    let relation_count: usize = conn.query_row("SELECT (SELECT COUNT(*) FROM knowledge_relations) + (SELECT COUNT(*) FROM reaffirmation_links)", [], |row| row.get(0))?;
    ensure!(
        relation_count <= 20_000,
        "project exceeds the complete baseline relationship limit"
    );
    let relation_bytes: usize = conn.query_row(
        "SELECT COALESCE(sum(length(CAST(exact_excerpt AS BLOB))+length(CAST(context_before AS BLOB))+length(CAST(context_after AS BLOB))),0) FROM evidence_snapshots WHERE id IN (SELECT evidence_id FROM knowledge_relations UNION SELECT evidence_id FROM reaffirmation_links)",
        [], |row| row.get(0),
    )?;
    ensure!(
        relation_bytes <= MAX_BASELINE_BYTES,
        "project exceeds the complete baseline relationship-evidence byte limit"
    );
    // Bound imported payloads before registry_revision or shared retrieval
    // materializes their full immutable JSON, not after serialization.
    let native = imported_revisions(conn)?;
    check_native_bytes(
        conn,
        &native
            .values()
            .map(|record| record.evidence_id.clone())
            .collect(),
    )?;
    cross_source_manifest(conn, false)?;
    Ok(())
}

fn display(value: &str) -> String {
    util::markdown_text(
        &value
            .chars()
            .flat_map(|character| {
                if character.is_control() {
                    character.escape_default().collect::<Vec<_>>()
                } else {
                    vec![character]
                }
            })
            .collect::<String>(),
    )
}

fn quote(value: &str) -> String {
    value
        .lines()
        .map(|line| format!("> {}\n", display(line)))
        .collect()
}

fn render_revision(label: &str, state: &RevisionState, text: &mut String) {
    text.push_str(&format!(
        "{label}: {}\n\nKind: {}. Lifecycle: {} (original {}). Scope: {}. Support: {}.\n\n",
        display(&state.statement),
        display(&state.kind),
        display(&state.lifecycle),
        display(&state.original_lifecycle),
        display(&state.scope),
        display(&state.support_state)
    ));
    text.push_str(&format!(
        "Basis: {}.\n\n",
        display(crate::domain::documentary_basis(&state.kind))
    ));
    if !state.effective_at.is_empty() {
        text.push_str(&format!(
            "Recorded effective time: {}.\n\n",
            display(&state.effective_at)
        ));
    }
}

pub fn render_changes(result: &ChangeReport) -> String {
    let mut text = format!("# Changes since {}\n\n", display(&result.baseline));
    if result.changes.is_empty()
        && result.relationship_changes.is_empty()
        && result.imported_changes.is_empty()
        && !result.cross_source_relationships_changed
    {
        text.push_str("No consequential retained-knowledge change is included for this scope. Live checkout behavior has not been assessed.\n");
    }
    if result.cross_source_relationships_changed {
        text.push_str("Cross-source interpretation membership changed. Included before-and-after groups resolve to immutable original payloads and endpoint evidence below. Task scope and whole-group omissions limit coverage. This is not an accepted policy change or independent runtime verification.\n\n");
    }
    for change in &result.changes {
        let state = change.after.as_ref().or(change.before.as_ref()).unwrap();
        text.push_str(&format!(
            "## {} — {}\n\n",
            display(&state.subject),
            change.change_kind
        ));
        if let Some(before) = &change.before {
            render_revision("Previously", before, &mut text);
        }
        if let Some(after) = &change.after {
            render_revision("Now", after, &mut text);
        }
        text.push_str(&format!("Inferred implication: {}\n\n", change.implication));
        for id in change
            .before
            .iter()
            .chain(change.after.iter())
            .flat_map(|s| &s.evidence_ids)
            .collect::<BTreeSet<_>>()
        {
            text.push_str(&format!("Evidence: `lore evidence {id}`\n\n"));
        }
    }
    for change in &result.relationship_changes {
        let state = change.after.as_ref().or(change.before.as_ref()).unwrap();
        text.push_str(&format!("## Documented relationship: {}\n\n`{}` {} `{}`. Previously active: {}; currently active: {}.\n\nEvidence: `lore evidence {}`\n\n",
            util::markdown_text(&state.kind), state.from, util::markdown_text(&state.kind), state.to,
            change.before.as_ref().is_some_and(|r| r.active), change.after.as_ref().is_some_and(|r| r.active), state.evidence_id));
    }
    interpretations::render(result, &mut text);
    if !result.related_knowledge.is_empty() {
        text.push_str("## Related original records\n\n");
        for state in &result.related_knowledge {
            render_revision(
                &format!("Record `{}`", display(&state.id)),
                state,
                &mut text,
            );
        }
    }
    for change in &result.imported_changes {
        text.push_str(&format!(
            "## Imported observation `{}`\n\n{}\n\n",
            display(&change.observation_id),
            change.qualification
        ));
        for (label, state) in [("Previously", &change.before), ("Now", &change.after)] {
            let Some(state) = state else {
                continue;
            };
            let Some(original) = result
                .imported_evidence
                .iter()
                .find(|e| e.evidence_id == state.evidence_id)
            else {
                continue;
            };
            text.push_str(&format!("{label}: {}\n\nSource-owned lifecycle: {}. Recorded scope: {}. Source-current at checkpoint: {}. Evidence: `{}`.\n\n",
                display(&original.record.statement), display(&original.record.lifecycle),
                display(&serde_json::to_string(&original.record.scope).unwrap_or_default()),
                state.current, display(&state.evidence_id)));
            text.push_str(&format!(
                "Recorded verification: {}. This does not establish current runtime behavior.\n\n",
                display(&serde_json::to_string(&original.record.verification).unwrap_or_default())
            ));
            text.push_str("Original imported payload and source-owned metadata:\n\n");
            text.push_str(&quote(
                &serde_json::to_string(&original.record).unwrap_or_default(),
            ));
            text.push('\n');
        }
    }
    if !result.evidence.is_empty() {
        text.push_str("## Retained original evidence and conditions\n\n");
    }
    for evidence in &result.evidence {
        text.push_str(&format!(
            "### `{}`\n\nSource: {}:{}. {}\n\n",
            display(&evidence.id),
            display(&evidence.root_id),
            display(&evidence.path),
            evidence.material.qualification()
        ));
        text.push_str(&format!(
            "Captured source revision: `{}`; exact excerpt digest: `{}`.\n\n",
            display(&evidence.source_revision_id),
            display(&evidence.digest)
        ));
        if let Some(origin) = &evidence.origin {
            text.push_str(&format!("Recorded origin: {}.\n\n", display(origin)));
        }
        text.push_str(&quote(&evidence.excerpt));
        if !evidence.context_before.is_empty() || !evidence.context_after.is_empty() {
            text.push_str(
                "\nBounded surrounding context from the same original source revision:\n\n",
            );
            text.push_str(&quote(&format!(
                "{}{}{}",
                evidence.context_before, evidence.excerpt, evidence.context_after
            )));
        }
        text.push('\n');
    }
    if result.omitted_changes > 0 {
        text.push_str(&format!(
            "{} complete changes omitted by the output budget.\n",
            result.omitted_changes
        ));
    }
    if result.omitted_relationship_changes > 0 {
        text.push_str(&format!(
            "{} complete relationship changes omitted by the output budget.\n",
            result.omitted_relationship_changes
        ));
    }
    if result.omitted_imported_changes > 0 {
        text.push_str(&format!(
            "{} complete imported changes omitted by the output budget.\n",
            result.omitted_imported_changes
        ));
    }
    if result.omitted_cross_source_changes > 0 {
        text.push_str(&format!(
            "{} complete cross-source interpretation changes omitted by the output budget.\n",
            result.omitted_cross_source_changes
        ));
    }
    if result.retrieval_truncated {
        text.push_str("Task relevance retrieval was incomplete; an empty included change list does not establish that no relevant change exists.\n");
    }
    text
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Advisory {
    pub severity: String,
    pub explanation: String,
    pub recommended_action: String,
    pub evidence_ids: Vec<String>,
    pub observation_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardianReport {
    pub schema_version: u32,
    pub baseline: String,
    pub snapshot: SnapshotManifest,
    pub assessment_status: String,
    pub advisories: Vec<Advisory>,
    pub intelligence: Option<AdaptiveResult>,
    pub execution: bool,
    pub source_write: bool,
    pub warnings: Vec<String>,
    pub advisories_in_shared_guidance: usize,
    pub budget: ContextBudget,
}

fn cross_source_topics(conn: &Connection) -> Result<Vec<String>> {
    let relations = imports::relationships::relations(conn)?;
    let endpoints: BTreeSet<_> = relations
        .iter()
        .flat_map(|relation| relation.endpoints())
        .map(|endpoint| endpoint.id.as_str())
        .collect();
    let mut topics: BTreeSet<_> = storage::views(conn)?
        .into_iter()
        .filter(|record| endpoints.contains(record.id.as_str()))
        .map(|record| record.topic)
        .collect();
    topics.extend(
        imports::views(conn)?
            .into_iter()
            .filter(|record| endpoints.contains(record.id.as_str()))
            .map(|record| record.record.subject),
    );
    Ok(topics.into_iter().collect())
}

/// Investigate first, then surface only source-reviewed material risks. The
/// advisory never blocks a process or claims that a static test was executed.
pub async fn guard(
    config: &ResolvedConfig,
    conn: &Connection,
    options: &ChangeOptions,
    run: &RunOptions,
) -> Result<GuardianReport> {
    ensure!(
        (1024..=context::MAX_MAX_TOKENS).contains(&options.max_tokens),
        "guardian --max-tokens must be 1024..100000"
    );
    let _snapshot = ReadSnapshot::begin(conn)?;
    let changes = changes(config, conn, options)?;
    let mut result = GuardianReport {
        schema_version: 1,
        baseline: options.since.clone(),
        snapshot: changes.snapshot,
        assessment_status: "no_documented_change".into(),
        advisories: Vec::new(),
        intelligence: None,
        execution: false,
        source_write: false,
        warnings: Vec::new(),
        advisories_in_shared_guidance: 0,
        budget: ContextBudget {
            max_tokens: options.max_tokens,
            used_tokens: 0,
            tokenizer: "cl100k_base".into(),
        },
    };
    let omitted = changes.omitted_changes
        + changes.omitted_relationship_changes
        + changes.omitted_imported_changes
        + changes.omitted_cross_source_changes;
    if changes.changes.is_empty()
        && changes.relationship_changes.is_empty()
        && changes.imported_changes.is_empty()
        && changes.cross_source_changes.is_empty()
        && options.task.is_none()
    {
        if omitted > 0 || changes.retrieval_truncated || changes.cross_source_relationships_changed
        {
            result.assessment_status = "change_budget_exhausted".into();
            result.warnings.push("The output budget could not retain complete change evidence; increase it or specify a task before advisory assessment.".into());
        }
        result.warnings.push("No live checkout assessment was requested; this result describes retained knowledge only.".into());
        return finish_guard(result);
    }
    if omitted > 0 || changes.retrieval_truncated {
        result.warnings.push("Some complete changes exceeded the comparison budget; this advisory covers the selected scope.".into());
    }
    let relation_topics = if changes.cross_source_relationships_changed {
        result.warnings.push(format!("Cross-source interpretations changed. Run `lore changes --since {}` for the included before-and-after source payloads; this advisory assesses current shared guidance. A withdrawn interpretation does not reverse a source claim, establish source agreement or verify runtime behavior.", options.since));
        cross_source_topics(conn)?
    } else {
        Vec::new()
    };
    let task = options.task.clone().unwrap_or_else(|| {
        let topics: BTreeSet<_> = changes.changes.iter().filter_map(|change| change.after.as_ref().or(change.before.as_ref()))
            .chain(changes.related_knowledge.iter())
            .chain(changes.cross_source_knowledge.iter())
            .map(|state| state.topic.clone())
            .chain(changes.imported_evidence.iter().map(|source| source.record.subject.clone()))
            .chain(relation_topics).collect();
        let selected: Vec<_> = topics.iter().filter(|topic| topic.len() <= 512).take(6).cloned().collect();
        if selected.len() < topics.len() {
            result.warnings.push(format!("The bounded advisory topic selection omitted {} other topic labels. Specify a task to assess another scope; this is not a complete project audit.", topics.len()-selected.len()));
        }
        let scope = if selected.is_empty() { "the project's retained source discrepancies".into() }
            else { selected.join(", ") };
        format!("Assess accepted constraints and current implementation for {scope}; recommend a concrete behavior-preserving response to consequential discrepancies")
    });
    // Reserve the complete wrapper, including future status/warnings. Duplicate
    // advisory summaries may be removed; the shared brief and its material
    // qualifications are never shortened to make a warning fit.
    let reserve = context::count_tokens(&serde_json::to_string(&result)?) + 256;
    let inner_budget = options
        .max_tokens
        .checked_sub(reserve)
        .filter(|value| *value >= 512)
        .context("guardian needs more output space for complete shared guidance")?;
    let intelligence = adaptive::run(
        config,
        conn,
        &ContextOptions {
            task,
            paths: Vec::new(),
            max_tokens: inner_budget,
        },
        run,
    )
    .await?;
    match &intelligence.intelligence {
        DecisionContextResult::Brief(brief)
            if brief.brief.generation_basis == GenerationBasis::ModelAssessed =>
        {
            result.assessment_status = "source_reviewed_advisory".into();
            for risk in &brief.brief.risks {
                if risk.severity == Severity::High
                    && matches!(
                        risk.category,
                        RiskCategory::AcceptedConstraint
                            | RiskCategory::Security
                            | RiskCategory::Destructive
                            | RiskCategory::FinancialCorrectness
                    )
                {
                    result.advisories.push(Advisory {
                        severity: "high".into(),
                        explanation: risk.text.clone(),
                        recommended_action: brief.brief.preferred_approach.text.clone(),
                        evidence_ids: risk
                            .evidence_ids
                            .iter()
                            .chain(&brief.brief.preferred_approach.evidence_ids)
                            .cloned()
                            .collect::<BTreeSet<_>>()
                            .into_iter()
                            .collect(),
                        observation_ids: risk
                            .observation_ids
                            .iter()
                            .chain(&brief.brief.preferred_approach.observation_ids)
                            .cloned()
                            .collect::<BTreeSet<_>>()
                            .into_iter()
                            .collect(),
                    });
                }
            }
            result.warnings.push("This bounded source-reviewed advisory is not a complete audit or runtime verification. An empty advisory list does not establish that the checkout is free of material risks.".into());
            for blocker in &brief.brief.material_blockers {
                result.advisories.push(Advisory {
                    severity: "high".into(),
                    explanation: blocker.explanation.clone(),
                    recommended_action: blocker.decision_needed.clone(),
                    evidence_ids: blocker.evidence_ids.clone(),
                    observation_ids: blocker.observation_ids.clone(),
                });
            }
        }
        _ => {
            result.assessment_status = "partial_static_guidance".into();
            result.warnings.push("Fresh model-supported risk assessment was unavailable; the shared evidence and permitted observations remain usable. An empty advisory list is not a clean bill of health.".into());
        }
    }
    result.intelligence = Some(intelligence);
    finish_guard(result)
}

fn finish_guard(mut result: GuardianReport) -> Result<GuardianReport> {
    loop {
        result.budget.used_tokens = 0;
        for _ in 0..8 {
            let used = context::count_tokens(&(serde_json::to_string(&result)? + "\n"))
                .max(context::count_tokens(&render_guard(&result)))
                .max(result.budget.used_tokens);
            if used == result.budget.used_tokens {
                break;
            }
            result.budget.used_tokens = used;
        }
        if result.budget.used_tokens <= result.budget.max_tokens {
            return Ok(result);
        }
        ensure!(
            result.advisories.pop().is_some(),
            "complete guardian guidance exceeds --max-tokens"
        );
        result.advisories_in_shared_guidance += 1;
    }
}

pub fn render_guard(result: &GuardianReport) -> String {
    let mut text = format!(
        "# Project advisory since {}\n\nAssessment: {}\n\n",
        util::markdown_text(&result.baseline),
        result.assessment_status
    );
    for advisory in &result.advisories {
        text.push_str(&format!(
            "## {}\n\n{}\n\n",
            util::markdown_text(&advisory.explanation),
            util::markdown_text(&advisory.recommended_action)
        ));
    }
    if result.advisories_in_shared_guidance > 0 {
        text.push_str(&format!("{} additional advisory summaries are retained in full in the shared guidance below.\n\n",result.advisories_in_shared_guidance));
    }
    if let Some(intelligence) = &result.intelligence {
        text.push_str(&adaptive::render(intelligence));
    }
    for warning in &result.warnings {
        text.push_str(&format!("\n{}\n", util::markdown_text(warning)));
    }
    text
}
