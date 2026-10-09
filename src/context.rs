//! Deterministic, read-only task context from the existing knowledge registry.
//! Selection never changes knowledge, generates claims, or invokes a model.
pub mod retrieval;

use crate::{
    domain::{KnowledgeView, SourceMaterial, documentary_basis},
    reviews, storage, util,
};
use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const CONTEXT_SCHEMA_VERSION: u32 = 1;
pub const DEFAULT_MAX_TOKENS: usize = 3_000;
pub const MIN_MAX_TOKENS: usize = 256;
pub const MAX_MAX_TOKENS: usize = 100_000;

#[derive(Debug, Clone)]
pub struct ContextOptions {
    pub task: String,
    pub paths: Vec<String>,
    pub max_tokens: usize,
}

#[derive(Debug)]
pub struct ContextError {
    pub code: &'static str,
    pub message: String,
}
impl std::fmt::Display for ContextError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for ContextError {}
fn failure(code: &'static str, message: impl Into<String>) -> anyhow::Error {
    ContextError {
        code,
        message: message.into(),
    }
    .into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextBudget {
    pub max_tokens: usize,
    /// A bound on both complete output formats, including their own metadata.
    pub used_tokens: usize,
    pub tokenizer: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContextSections {
    pub constraints: Vec<ContextItem>,
    pub decisions: Vec<ContextItem>,
    pub current_designs: Vec<ContextItem>,
    pub historical: Vec<ContextItem>,
    pub needs_verification: Vec<ContextItem>,
    pub proposals: Vec<ContextItem>,
    pub relevant_knowledge: Vec<ContextItem>,
}
impl ContextSections {
    pub fn items(&self) -> impl Iterator<Item = &ContextItem> {
        self.constraints
            .iter()
            .chain(&self.decisions)
            .chain(&self.current_designs)
            .chain(&self.historical)
            .chain(&self.needs_verification)
            .chain(&self.proposals)
            .chain(&self.relevant_knowledge)
    }
    fn push(&mut self, item: ContextItem, section: Section) {
        match section {
            Section::Constraints => &mut self.constraints,
            Section::Decisions => &mut self.decisions,
            Section::Designs => &mut self.current_designs,
            Section::Historical => &mut self.historical,
            Section::Verify => &mut self.needs_verification,
            Section::Proposals => &mut self.proposals,
            Section::Other => &mut self.relevant_knowledge,
        }
        .push(item);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextItem {
    pub id: String,
    pub statement: String,
    pub kind: String,
    pub lifecycle: String,
    pub support_state: String,
    pub topic: String,
    pub subject: String,
    pub scope: String,
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub effective_at: String,
    pub documentary_basis: String,
    pub evidence_ids: Vec<String>,
    pub qualifications: Vec<String>,
    pub relevance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextEvidence {
    pub id: String,
    /// Root-qualified path as observed at capture time, not a guessed live path.
    pub source: String,
    pub source_revision_id: String,
    pub material: SourceMaterial,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub origin: Option<String>,
    pub provenance_recorded: bool,
    pub current: bool,
    pub excerpt: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub line_start: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub line_end: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextRelation {
    pub id: String,
    pub from: String,
    pub to: String,
    pub kind: String,
    pub current: bool,
    pub evidence_id: String,
    pub material: SourceMaterial,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextReview {
    pub id: String,
    pub reason: String,
    pub knowledge_ids: Vec<String>,
    pub evidence_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct InspectionPath {
    pub root_id: String,
    pub path: String,
    /// `source_record` or `mentioned_in_evidence`; neither asserts file existence.
    pub basis: String,
    pub evidence_id: String,
    pub historical: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContextOmissions {
    pub knowledge_units: usize,
    pub critical_groups: usize,
    pub unsupported_units: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextResult {
    pub schema_version: u32,
    pub task: String,
    pub paths: Vec<String>,
    pub empty: bool,
    pub model_calls: u32,
    pub retrieval_truncated: bool,
    pub budget: ContextBudget,
    pub sections: ContextSections,
    pub relations: Vec<ContextRelation>,
    pub reviews: Vec<ContextReview>,
    pub evidence: Vec<ContextEvidence>,
    pub suggested_inspection: Vec<InspectionPath>,
    pub omissions: ContextOmissions,
    pub warnings: Vec<String>,
}

#[derive(Clone, Copy)]
enum Section {
    Constraints,
    Decisions,
    Designs,
    Historical,
    Verify,
    Proposals,
    Other,
}

struct Group {
    ids: BTreeSet<String>,
    priority: u8,
    score: f64,
    critical: bool,
}

/// Budgets use a named, embedded tokenizer, requiring no network or model.
/// Literal special-token spellings in source material are counted as text.
pub fn count_tokens(text: &str) -> usize {
    tiktoken_rs::cl100k_base_singleton().count_ordinary(text)
}

pub fn build_context(conn: &Connection, options: &ContextOptions) -> Result<ContextResult> {
    validate_options(options)?;
    let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if !(4..=storage::SCHEMA_VERSION).contains(&version) {
        return Err(failure(
            "incompatible_schema",
            "This registry requires a compatible Lore update before context can be read; no migration was performed.",
        ));
    }
    // Hold a single SQLite read snapshot for all queries, even when the caller
    // did not open an explicit transaction. SAVEPOINT makes this composable.
    conn.execute_batch("SAVEPOINT lore_context_read")?;
    let result = assemble(conn, options);
    let released = conn.execute_batch("RELEASE lore_context_read");
    match result {
        Ok(result) => {
            released?;
            Ok(result)
        }
        Err(error) => {
            let _ = released;
            Err(error)
        }
    }
}

fn validate_options(options: &ContextOptions) -> Result<()> {
    if options.task.trim().is_empty()
        || options.task.len() > 8_000
        || options.task.chars().any(char::is_control)
        || !options.task.chars().any(char::is_alphanumeric)
    {
        return Err(failure(
            "invalid_query",
            "Task must contain text, have no control characters, and be at most 8000 UTF-8 bytes.",
        ));
    }
    if !(MIN_MAX_TOKENS..=MAX_MAX_TOKENS).contains(&options.max_tokens) {
        return Err(failure(
            "invalid_budget",
            format!("--max-tokens must be {MIN_MAX_TOKENS}..{MAX_MAX_TOKENS}."),
        ));
    }
    if options.paths.len() > 32
        || options
            .paths
            .iter()
            .any(|p| p.trim().is_empty() || p.len() > 4_096 || p.chars().any(char::is_control))
    {
        return Err(failure(
            "invalid_query",
            "Supply at most 32 nonempty path hints, each at most 4096 bytes and without control characters.",
        ));
    }
    Ok(())
}

fn critical(kind: &str) -> bool {
    matches!(
        kind,
        "supersedes" | "contradicts" | "suspected_conflict" | "reaffirms"
    )
}
fn conflict(kind: &str) -> bool {
    matches!(kind, "contradicts" | "suspected_conflict")
}

fn assemble(conn: &Connection, options: &ContextOptions) -> Result<ContextResult> {
    let retrieval = retrieval::retrieve_with_report(conn, options.task.trim(), &options.paths)?;
    let hits = retrieval.hits;
    let views: BTreeMap<_, _> = storage::views(conn)?
        .into_iter()
        .map(|v| (v.id.clone(), v))
        .collect();
    let facts = storage::relation_facts(conn)?;
    let pending = reviews::list(conn, false)?;
    let mut reasons: BTreeMap<String, Vec<String>> = hits
        .iter()
        .map(|h| (h.knowledge.id.clone(), h.reasons.clone()))
        .collect();
    let scores: BTreeMap<_, _> = hits
        .iter()
        .map(|h| (h.knowledge.id.clone(), h.score))
        .collect();
    let mut adjacency: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for fact in facts.iter().filter(|f| critical(&f.kind)) {
        adjacency
            .entry(fact.from.clone())
            .or_default()
            .insert(fact.to.clone());
        adjacency
            .entry(fact.to.clone())
            .or_default()
            .insert(fact.from.clone());
    }
    // Reviews are associated by stored IDs only. A prose resemblance cannot
    // bind a review to a task or silently close a recorded disagreement.
    let mut review_bindings = BTreeMap::new();
    for review in &pending {
        let mut ids = BTreeSet::new();
        if let Some(id) = &review.target_unit_id {
            ids.insert(id.clone());
        }
        if let Some(assertion) = &review.assertion_revision_id {
            if let Some(id) = storage::assigned(conn, assertion)? {
                ids.insert(id);
            }
        }
        for id in &ids {
            adjacency
                .entry(id.clone())
                .or_default()
                .extend(ids.iter().filter(|other| *other != id).cloned());
        }
        review_bindings.insert(review.id.clone(), ids);
    }
    let mut seen = BTreeSet::new();
    let mut groups = Vec::new();
    for hit in &hits {
        if seen.contains(&hit.knowledge.id) {
            continue;
        }
        let mut stack = vec![hit.knowledge.id.clone()];
        let mut ids = BTreeSet::new();
        while let Some(id) = stack.pop() {
            if !ids.insert(id.clone()) {
                continue;
            }
            if let Some(neighbors) = adjacency.get(&id) {
                stack.extend(neighbors.iter().cloned());
            }
        }
        for id in &ids {
            if !views.contains_key(id) {
                return Err(failure(
                    "invalid_registry",
                    "A relevant recorded relationship refers to missing knowledge; run lore audit.",
                ));
            }
            reasons.entry(id.clone()).or_insert_with(|| {
                vec!["Required by a recorded decision, conflict, or review relationship.".into()]
            });
        }
        seen.extend(ids.iter().cloned());
        let has_conflict = facts
            .iter()
            .any(|f| conflict(&f.kind) && (ids.contains(&f.from) || ids.contains(&f.to)))
            || review_bindings
                .values()
                .any(|bound| !bound.is_disjoint(&ids));
        let current_guidance = |id: &String| {
            let view = &views[id];
            matches!(view.lifecycle.as_str(), "active" | "accepted")
                && view.support_state == "current_documentary_support"
                && view
                    .evidence
                    .iter()
                    .any(|evidence| evidence.active && evidence.material == SourceMaterial::Primary)
        };
        let priority = if has_conflict {
            0
        } else if ids
            .iter()
            .any(|id| views[id].kind == "constraint" && current_guidance(id))
        {
            1
        } else if ids
            .iter()
            .any(|id| views[id].kind == "decision" && current_guidance(id))
        {
            2
        } else {
            3
        };
        let score = ids
            .iter()
            .filter_map(|id| scores.get(id))
            .copied()
            .fold(0.0, f64::max);
        let is_critical = ids.len() > 1 || has_conflict;
        groups.push(Group {
            ids,
            priority,
            score,
            critical: is_critical,
        });
    }
    // Relevance remains the main signal. A weak topic-only constraint must
    // not displace strongly task-matched knowledge solely because of its kind.
    // Only currently supported, adopted guidance receives an authority bonus.
    let ranked_score = |group: &Group| {
        group.score
            + match group.priority {
                0 => 2.0,
                1 => 1.5,
                2 => 0.75,
                _ => 0.0,
            }
    };
    groups.sort_by(|a, b| {
        ranked_score(b)
            .total_cmp(&ranked_score(a))
            .then_with(|| a.priority.cmp(&b.priority))
            .then_with(|| a.ids.cmp(&b.ids))
    });
    let mut candidates = BTreeSet::new();
    for group in &groups {
        candidates.extend(group.ids.iter().cloned());
    }
    let unsupported: BTreeSet<_> = candidates
        .iter()
        .filter(|id| {
            let view = &views[*id];
            view.support_state == "unsupported" || view.evidence.is_empty()
        })
        .cloned()
        .collect();
    let assembly = Assembly {
        conn,
        options,
        views: &views,
        facts: &facts,
        pending: &pending,
        review_bindings: &review_bindings,
        reasons: &reasons,
        groups: &groups,
        candidates: &candidates,
        unsupported: &unsupported,
        retrieval_truncated: retrieval.truncated,
    };
    let mut selected = BTreeSet::new();
    let mut result = assembly.result(&selected)?;
    measure(&mut result)?;
    if result.budget.used_tokens > options.max_tokens {
        return Err(failure(
            "invalid_budget",
            format!(
                "Task, path hints and context metadata require at least {} tokens; shorten the task or increase --max-tokens.",
                result.budget.used_tokens
            ),
        ));
    }
    for group in &groups {
        if !group.ids.is_disjoint(&unsupported) {
            continue;
        }
        let mut trial_ids = selected.clone();
        trial_ids.extend(group.ids.iter().cloned());
        let mut trial = assembly.result(&trial_ids)?;
        measure(&mut trial)?;
        if trial.budget.used_tokens <= options.max_tokens {
            selected = trial_ids;
            result = trial;
        }
    }
    Ok(result)
}

struct Assembly<'a> {
    conn: &'a Connection,
    options: &'a ContextOptions,
    views: &'a BTreeMap<String, KnowledgeView>,
    facts: &'a [storage::RelationFact],
    pending: &'a [reviews::ReviewItem],
    review_bindings: &'a BTreeMap<String, BTreeSet<String>>,
    reasons: &'a BTreeMap<String, Vec<String>>,
    groups: &'a [Group],
    candidates: &'a BTreeSet<String>,
    unsupported: &'a BTreeSet<String>,
    retrieval_truncated: bool,
}

impl Assembly<'_> {
    fn result(&self, selected: &BTreeSet<String>) -> Result<ContextResult> {
        let mut paths = self
            .options
            .paths
            .iter()
            .map(|p| p.trim().replace('\\', "/"))
            .collect::<Vec<_>>();
        paths.sort();
        paths.dedup();
        let mut result = ContextResult {
            schema_version: CONTEXT_SCHEMA_VERSION,
            task: self.options.task.trim().into(),
            paths,
            empty: selected.is_empty(),
            model_calls: 0,
            retrieval_truncated: self.retrieval_truncated,
            budget: ContextBudget {
                max_tokens: self.options.max_tokens,
                used_tokens: 0,
                tokenizer: "cl100k_base".into(),
            },
            sections: ContextSections::default(),
            relations: vec![],
            reviews: vec![],
            evidence: vec![],
            suggested_inspection: vec![],
            omissions: ContextOmissions {
                knowledge_units: self.candidates.len() - selected.len(),
                critical_groups: self
                    .groups
                    .iter()
                    .filter(|g| g.critical && g.ids.is_disjoint(selected))
                    .count(),
                unsupported_units: self.unsupported.len(),
            },
            warnings: vec![],
        };
        let mut evidence_active: BTreeMap<String, bool> = BTreeMap::new();
        for group in self.groups {
            for id in &group.ids {
                if !selected.contains(id) {
                    continue;
                }
                let view = &self.views[id];
                for evidence in &view.evidence {
                    *evidence_active.entry(evidence.id.clone()).or_default() |= evidence.active;
                }
                let edges: Vec<_> = self
                    .facts
                    .iter()
                    .filter(|f| f.from == *id || f.to == *id)
                    .collect();
                let has_review = self
                    .review_bindings
                    .values()
                    .any(|bound| bound.contains(id));
                let (item, section) = context_item(view, &edges, has_review, &self.reasons[id]);
                result.sections.push(item, section);
            }
        }
        let included_facts: Vec<_> = self
            .facts
            .iter()
            .filter(|f| selected.contains(&f.from) && selected.contains(&f.to))
            .collect();
        for fact in &included_facts {
            if fact.evidence_id.is_empty() {
                return Err(failure(
                    "invalid_registry",
                    "A selected relationship has no evidence; run lore audit.",
                ));
            }
            *evidence_active.entry(fact.evidence_id.clone()).or_default() |= fact.active;
        }
        let mut inspections = BTreeSet::new();
        let mut materials = BTreeMap::new();
        for (id, current) in evidence_active {
            let snapshot = storage::evidence_snapshot(self.conn, &id)
                .with_context(|| format!("resolve selected evidence {id}"))?;
            if util::digest(&snapshot.excerpt) != snapshot.digest || snapshot.excerpt.is_empty() {
                return Err(failure(
                    "invalid_registry",
                    "A selected evidence snapshot failed its integrity check; run lore audit.",
                ));
            }
            materials.insert(id.clone(), snapshot.material);
            let live: Option<(String, String)> = self.conn.query_row(
                "SELECT s.root_id,s.relative_path FROM sources s JOIN source_current c ON c.source_id=s.id WHERE s.id=?1 AND s.removed_at IS NULL",
                [&snapshot.source_id], |row| Ok((row.get(0)?, row.get(1)?)),
            ).optional()?;
            let historical = !current || live.is_none();
            let (inspection_root, inspection_path) =
                live.unwrap_or_else(|| (snapshot.root_id.clone(), snapshot.path.clone()));
            inspections.insert(InspectionPath {
                root_id: inspection_root,
                path: inspection_path,
                basis: "source_record".into(),
                evidence_id: id.clone(),
                historical,
            });
            for path in mentioned_paths(&snapshot.excerpt) {
                inspections.insert(InspectionPath {
                    root_id: snapshot.root_id.clone(),
                    path,
                    basis: "mentioned_in_evidence".into(),
                    evidence_id: id.clone(),
                    historical: !current,
                });
            }
            result.evidence.push(ContextEvidence {
                id,
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
        result.suggested_inspection = inspections.into_iter().collect();
        for fact in included_facts {
            result.relations.push(ContextRelation {
                id: fact.id.clone(),
                from: fact.from.clone(),
                to: fact.to.clone(),
                kind: fact.kind.clone(),
                current: fact.active,
                evidence_id: fact.evidence_id.clone(),
                material: materials[&fact.evidence_id],
            });
        }
        result.relations.sort_by(|a, b| {
            (&a.from, &a.to, &a.kind, &a.id).cmp(&(&b.from, &b.to, &b.kind, &b.id))
        });
        result.relations.dedup_by(|a, b| a.id == b.id);
        for review in self.pending {
            let bound = &self.review_bindings[&review.id];
            if bound.is_empty() || bound.is_disjoint(selected) {
                continue;
            }
            let knowledge_ids: Vec<_> = bound
                .iter()
                .filter(|id| selected.contains(*id))
                .cloned()
                .collect();
            let evidence_ids = knowledge_ids
                .iter()
                .flat_map(|id| self.views[id].evidence.iter().map(|e| e.id.clone()))
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            result.reviews.push(ContextReview {
                id: review.id.clone(),
                reason: review.reason.clone(),
                knowledge_ids,
                evidence_ids,
            });
        }
        if result.omissions.knowledge_units > 0 {
            result.warnings.push("Some relevant knowledge was omitted. Increase --max-tokens or narrow the task; complete records and critical relationship groups are kept together.".into());
        }
        if result.retrieval_truncated {
            result.warnings.push("Retrieval reached a search or expansion bound. Omission counts cover retrieved candidates only; narrow the task or add a specific --path to inspect other knowledge.".into());
        }
        if result.omissions.critical_groups > 0 {
            result.warnings.push("Relevant conflict, review, or decision-history groups could not be included. This context is incomplete for a decision.".into());
        }
        if result.omissions.unsupported_units > 0 {
            result.warnings.push(
                "Unsupported records were excluded; inspect the registry with lore audit.".into(),
            );
        }
        if result.evidence.iter().any(|e| !e.provenance_recorded) {
            result.warnings.push("Legacy evidence predates provenance capture; primary is a compatibility default, and original material/origin were not recorded.".into());
        }
        if selected.is_empty() && self.candidates.is_empty() {
            result.warnings.push("No relevant stored knowledge found. This does not establish that no constraints exist.".into());
        }
        Ok(result)
    }
}

fn context_item(
    view: &KnowledgeView,
    edges: &[&storage::RelationFact],
    has_review: bool,
    reasons: &[String],
) -> (ContextItem, Section) {
    let mut qualifications = Vec::new();
    let current = view.evidence.iter().any(|e| e.active);
    let primary = view
        .evidence
        .iter()
        .any(|e| e.active && e.material == SourceMaterial::Primary && e.root_path.is_some());
    let derived_only = view
        .evidence
        .iter()
        .filter(|e| e.active)
        .all(|e| e.material == SourceMaterial::Derived);
    let disputed = edges.iter().any(|e| conflict(&e.kind));
    let withdrawn = edges
        .iter()
        .any(|e| e.to == view.id && e.kind == "supersedes" && !e.active);
    if view.lifecycle == "superseded" {
        qualifications
            .push("Superseded documentary decision; do not apply as current guidance.".into());
    }
    if !current || view.support_state == "historical_only" {
        qualifications
            .push("Historical evidence only; current applicability is unverified.".into());
    }
    if current && !primary {
        qualifications.push(if derived_only {
            "Current support is derived material only; verify the original source before applying this as authoritative guidance."
        } else {
            "Original source provenance was not captured; verify its primary or derived status before applying this as authoritative guidance."
        }.into());
    }
    if disputed {
        qualifications.push("Recorded disagreement remains relevant, including retained historical evidence; inspect the linked records.".into());
    }
    if withdrawn {
        qualifications.push(
            "Prior replacement evidence was withdrawn; this does not revive the earlier decision."
                .into(),
        );
    }
    if has_review {
        qualifications.push("An unresolved, evidence-bound review applies to this record.".into());
    }
    if matches!(view.kind.as_str(), "reported_outcome" | "observation") {
        qualifications.push(
            "This is a source report or observation, not independent verification of the outcome."
                .into(),
        );
    }
    if matches!(view.lifecycle.as_str(), "proposed" | "unknown" | "rejected") {
        qualifications.push(format!(
            "Lifecycle is {}; adoption must not be inferred.",
            view.lifecycle
        ));
    }
    let section = if matches!(view.lifecycle.as_str(), "superseded" | "rejected")
        || !current
        || view.support_state == "historical_only"
    {
        Section::Historical
    } else if disputed
        || withdrawn
        || has_review
        || !primary
        || view.support_state == "needs_review"
        || matches!(view.kind.as_str(), "risk" | "question")
    {
        Section::Verify
    } else if matches!(view.kind.as_str(), "plan" | "proposal") || view.lifecycle == "proposed" {
        Section::Proposals
    } else if matches!(view.kind.as_str(), "reported_outcome" | "observation") {
        Section::Historical
    } else if matches!(view.lifecycle.as_str(), "active" | "accepted") {
        match view.kind.as_str() {
            "constraint" => Section::Constraints,
            "decision" => Section::Decisions,
            "design" | "procedure" => Section::Designs,
            _ => Section::Other,
        }
    } else if matches!(view.kind.as_str(), "constraint" | "decision") {
        Section::Verify
    } else {
        Section::Other
    };
    let evidence_ids = view
        .evidence
        .iter()
        .map(|e| e.id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let mut relevance = reasons.to_vec();
    relevance.sort();
    relevance.dedup();
    (
        ContextItem {
            id: view.id.clone(),
            statement: view.statement.clone(),
            kind: view.kind.clone(),
            lifecycle: view.lifecycle.clone(),
            support_state: view.support_state.clone(),
            topic: view.topic.clone(),
            subject: view.subject.clone(),
            scope: view.scope.clone(),
            effective_at: view.effective_at.clone(),
            documentary_basis: documentary_basis(&view.kind).into(),
            evidence_ids,
            qualifications,
            relevance,
        },
        section,
    )
}

/// A source path suggestion must occur literally in an evidence excerpt. Do
/// not invent paths from a topic, task, basename, or a model's statement.
fn mentioned_paths(excerpt: &str) -> BTreeSet<String> {
    excerpt
        .split(|c: char| {
            c.is_whitespace()
                || matches!(
                    c,
                    '`' | '"' | '\'' | '(' | ')' | '[' | ']' | '<' | '>' | ',' | ';'
                )
        })
        .map(|s| s.trim_end_matches(['.', ':', '!']).replace('\\', "/"))
        .filter(|s| {
            s.contains('/')
                && s.rsplit('/').next().is_some_and(|last| last.contains('.'))
                && !s.contains("://")
                && !s.starts_with('/')
                && !s.split('/').any(|part| part == "..")
                && s.len() <= 1_024
                && s.chars().all(|c| c.is_alphanumeric() || "/._-".contains(c))
        })
        .collect()
}

fn measure(result: &mut ContextResult) -> Result<()> {
    result.budget.used_tokens = 0;
    // Numerical metadata can change its own token count at a digit boundary.
    // Monotonic convergence keeps a safe bound even if an exact fixed point
    // does not exist. The finite input budget bounds the possible digit count.
    for _ in 0..16 {
        let json = serde_json::to_string(result)? + "\n";
        let used = count_tokens(&json).max(count_tokens(&render_context(result)));
        if used <= result.budget.used_tokens {
            return Ok(());
        }
        result.budget.used_tokens = used;
    }
    Err(failure(
        "invalid_budget",
        "Could not stabilize context budget metadata.",
    ))
}

fn display_text(value: &str) -> String {
    util::markdown_text(
        &value
            .chars()
            .flat_map(|c| {
                if c.is_control() {
                    c.escape_default().collect::<Vec<_>>()
                } else {
                    vec![c]
                }
            })
            .collect::<String>(),
    )
}

/// Human and JSON forms contain the same stored records and qualifiers.
pub fn render_context(result: &ContextResult) -> String {
    let mut text = format!("Task: {}\n\n", display_text(&result.task));
    if !result.paths.is_empty() {
        text.push_str(&format!(
            "Path hints: {}\n\n",
            result
                .paths
                .iter()
                .map(|p| display_text(p))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    text.push_str("Documentary context; source reports are not independent verification of implementation.\n\n");
    for (title, items) in [
        ("Constraints", &result.sections.constraints),
        ("Relevant decisions", &result.sections.decisions),
        (
            "Current designs and procedures",
            &result.sections.current_designs,
        ),
        (
            "Historical lessons and decisions",
            &result.sections.historical,
        ),
        ("Needs verification", &result.sections.needs_verification),
        ("Proposals and plans", &result.sections.proposals),
        (
            "Other relevant knowledge",
            &result.sections.relevant_knowledge,
        ),
    ] {
        if items.is_empty() {
            continue;
        }
        text.push_str(&format!("## {title}\n\n"));
        for item in items {
            text.push_str(&format!(
                "- {} ({}, {}, {}; scope: {}) [{}]\n",
                display_text(&item.statement),
                item.kind,
                item.lifecycle,
                item.support_state,
                display_text(&item.scope),
                item.id
            ));
            if !item.effective_at.is_empty() {
                text.push_str(&format!(
                    "  Effective: {}\n",
                    display_text(&item.effective_at)
                ));
            }
            for qualification in &item.qualifications {
                text.push_str(&format!("  {}\n", display_text(qualification)));
            }
            text.push_str(&format!("  Evidence: {}\n", item.evidence_ids.join(", ")));
        }
        text.push('\n');
    }
    if !result.relations.is_empty() {
        text.push_str("## Recorded relationships\n\n");
        for relation in &result.relations {
            text.push_str(&format!(
                "- {} {} {} ({}; {} material). Evidence: {}\n",
                relation.from,
                relation.kind,
                relation.to,
                if relation.current {
                    "current documentary evidence"
                } else {
                    "historical evidence only"
                },
                relation.material.as_str(),
                relation.evidence_id
            ));
        }
        text.push('\n');
    }
    if !result.reviews.is_empty() {
        text.push_str("## Unresolved reviews\n\n");
        for review in &result.reviews {
            text.push_str(&format!(
                "- {} [{}]. Evidence: {}\n",
                display_text(&review.reason),
                review.id,
                review.evidence_ids.join(", ")
            ));
        }
        text.push('\n');
    }
    if !result.evidence.is_empty() {
        text.push_str("## Evidence\n\n");
        for evidence in &result.evidence {
            let lines = match (evidence.line_start, evidence.line_end) {
                (Some(a), Some(b)) => format!(":{a}-{b}"),
                _ => String::new(),
            };
            text.push_str(&format!(
                "- [{}] {}{} ({}; {} material).\n  {}\n",
                evidence.id,
                display_text(&evidence.source),
                lines,
                if evidence.current {
                    "current documentary evidence"
                } else {
                    "historical evidence only"
                },
                evidence.material.as_str(),
                display_text(&evidence.excerpt)
            ));
            if let Some(origin) = &evidence.origin {
                text.push_str(&format!("  Declared origin: {}\n", display_text(origin)));
            }
        }
        text.push_str("Resolve full snapshots with: lore evidence <evidence-id>\n\n");
    }
    if !result.suggested_inspection.is_empty() {
        text.push_str("## Suggested inspection\n\n");
        for path in &result.suggested_inspection {
            text.push_str(&format!(
                "- {}:{} ({}{}; evidence: {})\n",
                display_text(&path.root_id),
                display_text(&path.path),
                path.basis,
                if path.historical {
                    ", historical reference"
                } else {
                    ""
                },
                path.evidence_id
            ));
        }
        text.push('\n');
    }
    if result.empty {
        text.push_str("No knowledge records included.\n\n");
    }
    for warning in &result.warnings {
        text.push_str(&format!("Warning: {warning}\n"));
    }
    text.push_str(&format!("\nBudget: {}/{} {} tokens (both output formats). Omitted: {} knowledge units, {} critical groups; {} unsupported units. Model calls: 0.\n",
        result.budget.used_tokens, result.budget.max_tokens, result.budget.tokenizer,
        result.omissions.knowledge_units, result.omissions.critical_groups, result.omissions.unsupported_units));
    text
}
