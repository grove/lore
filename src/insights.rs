//! Deterministic decision lenses and non-executing worked cases.
//!
//! These are presentations of the existing registry, not new accepted facts.
//! Source headings organize exact retained passages; missing rationale or
//! outcomes remain missing rather than being invented from engineering norms.

mod evidence;
mod render;

pub use render::{render_cases, render_decisions};

use crate::{
    context::{self, ContextBudget, ContextItem, ContextOptions, ContextRelation, ContextResult},
    imports::adapters::ObservationKind,
    storage::{self, EvidenceSnapshot},
    util,
};
use anyhow::Result;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const INSIGHTS_SCHEMA_VERSION: u32 = 1;
const MAX_LENSES: usize = 12;
const MAX_CASES: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FacetKind {
    StartingConditions,
    Rationale,
    Alternatives,
    TradeOffs,
    Assumptions,
    Constraints,
    Conditions,
    Exceptions,
    ExpectedBehavior,
    ReportedOutcome,
    Consequences,
    ReconsiderationTriggers,
    RejectedApproaches,
    FailureModes,
}

impl FacetKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::StartingConditions => "Starting conditions",
            Self::Rationale => "Documented rationale",
            Self::Alternatives => "Documented alternatives",
            Self::TradeOffs => "Documented trade-offs",
            Self::Assumptions => "Documented assumptions",
            Self::Constraints => "Documented constraints",
            Self::Conditions => "Applicability conditions",
            Self::Exceptions => "Important exceptions",
            Self::ExpectedBehavior => "Documented expected behavior",
            Self::ReportedOutcome => "Source-reported outcome",
            Self::Consequences => "Documented consequences",
            Self::ReconsiderationTriggers => "Reconsideration triggers",
            Self::RejectedApproaches => "Rejected approaches",
            Self::FailureModes => "Documented failure modes",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FacetRecord {
    pub knowledge_id: String,
    pub kind: String,
    pub lifecycle: String,
    pub original_lifecycle: String,
    pub scope: String,
    pub support_state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentedFacet {
    pub kind: FacetKind,
    /// `captured_source_heading` or an explicit marker in the original quote.
    /// This organizes source text; it does not assert a newly inferred reason.
    pub classification_basis: String,
    pub heading_path: Vec<String>,
    /// Exact source-owned classifications; a proposed sibling passage does not
    /// acquire the lifecycle of the decision being explained.
    pub records: Vec<FacetRecord>,
    pub evidence_id: String,
    /// Current retained support, not independently verified runtime behavior.
    pub current: bool,
    /// The complete retained quote and its bounded original surrounding text.
    /// The surrounding text is not a complete source document or a new ID.
    pub evidence: EvidenceSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionLens {
    pub record: ContextItem,
    /// Kept separately from an effective `superseded` lifecycle.
    pub original_lifecycle: String,
    pub facets: Vec<DocumentedFacet>,
    /// Original source-backed relationships, including historical witnesses.
    pub history: Vec<ContextRelation>,
    pub negative_knowledge_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NegativeKnowledge {
    pub knowledge_id: String,
    /// Derived only from a retained rejected lifecycle or documented risk.
    pub basis: String,
    pub evidence_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsightCoverage {
    pub selected_records: usize,
    pub omitted_complete_items: usize,
    pub candidate_limit_reached: bool,
    pub retrieval_truncated: bool,
    pub complete_source_documents: bool,
    pub qualification: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionLenses {
    pub schema_version: u32,
    pub query: String,
    pub registry_revision: String,
    pub generation_basis: String,
    pub model_calls: u32,
    pub lenses: Vec<DecisionLens>,
    pub negative_knowledge: Vec<NegativeKnowledge>,
    /// The unmodified, independently versioned retrieval contract retains
    /// critical groups, source authority, exceptions and relation endpoints.
    pub context: ContextResult,
    pub coverage: InsightCoverage,
    pub budget: ContextBudget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaseKind {
    RejectedApproach,
    DocumentedRisk,
    DocumentedProcedure,
    SourceReportedOutcome,
    SourceReportedObservation,
    WorkHistory,
    ImportedReport,
    BoundaryCase,
}

impl CaseKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::RejectedApproach => "Rejected approach",
            Self::DocumentedRisk => "Documented risk",
            Self::DocumentedProcedure => "Documented procedure",
            Self::SourceReportedOutcome => "Source-reported outcome",
            Self::SourceReportedObservation => "Source-reported observation",
            Self::WorkHistory => "Source-owned work history",
            Self::ImportedReport => "Imported report",
            Self::BoundaryCase => "Documented boundary case",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkedCase {
    pub id: String,
    pub title: String,
    pub kind: CaseKind,
    pub record: Option<ContextItem>,
    pub imported_record: Option<context::imports::ContextObservation>,
    pub facets: Vec<DocumentedFacet>,
    pub evidence_ids: Vec<String>,
    pub qualification: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseResult {
    pub schema_version: u32,
    pub query: String,
    pub registry_revision: String,
    pub generation_basis: String,
    pub model_calls: u32,
    pub runtime_execution: bool,
    pub tests_run: u32,
    pub cases: Vec<WorkedCase>,
    pub context: ContextResult,
    pub coverage: InsightCoverage,
    pub budget: ContextBudget,
}

fn invalid_budget() -> anyhow::Error {
    context::ContextError {
        code: "invalid_budget",
        message: "Decision/case views need room for complete evidence and conditions; use --max-tokens between 512 and 100000.".into(),
    }
    .into()
}

fn selected(conn: &Connection, query: &str, max_tokens: usize) -> Result<ContextResult> {
    if !(512..=context::MAX_MAX_TOKENS).contains(&max_tokens) {
        return Err(invalid_budget());
    }
    context::build_context(
        conn,
        &ContextOptions {
            task: query.to_owned(),
            paths: Vec::new(),
            max_tokens: (max_tokens / 2).max(context::MIN_MAX_TOKENS),
        },
    )
}

fn budget(max_tokens: usize) -> ContextBudget {
    ContextBudget {
        max_tokens,
        used_tokens: 0,
        tokenizer: "cl100k_base".into(),
    }
}

fn coverage(context: &ContextResult) -> InsightCoverage {
    InsightCoverage {
        selected_records: context.sections.items().count() + context.imported_observations.len(),
        omitted_complete_items: 0,
        candidate_limit_reached: false,
        retrieval_truncated: context.retrieval_truncated,
        complete_source_documents: false,
        qualification: "Fields contain exact retained evidence where structurally identified. Missing fields do not establish that a source lacks the information. Surrounding context is bounded; no source document, runtime behavior or test result is reconstructed or independently verified.".into(),
    }
}

fn negative(context: &ContextResult) -> Vec<NegativeKnowledge> {
    context
        .sections
        .items()
        .filter_map(|record| {
            let basis = if record.lifecycle == "rejected" {
                "retained_rejected_lifecycle"
            } else if record.kind == "risk" {
                "documented_risk_not_an_observed_failure"
            } else {
                return None;
            };
            Some(NegativeKnowledge {
                knowledge_id: record.id.clone(),
                basis: basis.into(),
                evidence_ids: record.evidence_ids.clone(),
            })
        })
        .collect()
}

fn with_snapshot<T>(conn: &Connection, operation: impl FnOnce() -> Result<T>) -> Result<T> {
    conn.execute_batch("SAVEPOINT lore_insights")?;
    let result = operation();
    let released = conn.execute_batch("RELEASE lore_insights");
    match result {
        Ok(value) => {
            released?;
            Ok(value)
        }
        Err(error) => {
            let _ = released;
            Err(error)
        }
    }
}

/// Build practical, source-owned decision views without inference, live file
/// reads, registry writes, or any command execution.
pub fn decisions(conn: &Connection, query: &str, max_tokens: usize) -> Result<DecisionLenses> {
    with_snapshot(conn, || {
        let context = selected(conn, query, max_tokens)?;
        let mut coverage = coverage(&context);
        let negative_knowledge = negative(&context);
        let mut details = evidence::Details::new(conn)?;
        let mut lenses = Vec::new();
        for record in context
            .sections
            .items()
            .filter(|record| matches!(record.kind.as_str(), "decision" | "proposal" | "plan"))
        {
            if lenses.len() >= MAX_LENSES {
                coverage.omitted_complete_items += 1;
                coverage.candidate_limit_reached = true;
                continue;
            }
            let Some(facets) = details.facets(record)? else {
                coverage.omitted_complete_items += 1;
                coverage.candidate_limit_reached = true;
                continue;
            };
            let history: Vec<_> = context
                .relations
                .iter()
                .filter(|relation| {
                    (relation.from == record.id || relation.to == record.id)
                        && matches!(
                            relation.kind.as_str(),
                            "supersedes" | "reaffirms" | "contradicts" | "suspected_conflict"
                        )
                })
                .cloned()
                .collect();
            let related: BTreeSet<_> = history
                .iter()
                .flat_map(|relation| [&relation.from, &relation.to])
                .collect();
            let negative_ids = negative_knowledge
                .iter()
                .filter(|item| {
                    item.knowledge_id == record.id || related.contains(&item.knowledge_id)
                })
                .map(|item| item.knowledge_id.clone())
                .collect();
            lenses.push(DecisionLens {
                record: record.clone(),
                original_lifecycle: details.original_lifecycle(&record.id),
                facets,
                history,
                negative_knowledge_ids: negative_ids,
            });
        }
        let mut result = DecisionLenses {
            schema_version: INSIGHTS_SCHEMA_VERSION,
            query: query.into(),
            registry_revision: storage::registry_revision(conn)?,
            generation_basis: "deterministic_documentary_view".into(),
            model_calls: 0,
            lenses,
            negative_knowledge,
            context,
            coverage,
            budget: budget(max_tokens),
        };
        fit_decisions(&mut result)?;
        Ok(result)
    })
}

fn record_case(record: &ContextItem, facets: &[DocumentedFacet]) -> Option<CaseKind> {
    if record.lifecycle == "rejected" {
        Some(CaseKind::RejectedApproach)
    } else {
        match record.kind.as_str() {
            "risk" => Some(CaseKind::DocumentedRisk),
            "procedure" => Some(CaseKind::DocumentedProcedure),
            "reported_outcome" => Some(CaseKind::SourceReportedOutcome),
            "observation" => Some(CaseKind::SourceReportedObservation),
            "issue_state" => Some(CaseKind::WorkHistory),
            "constraint"
                if facets.iter().any(|facet| {
                    matches!(facet.kind, FacetKind::Exceptions | FacetKind::FailureModes)
                }) =>
            {
                Some(CaseKind::BoundaryCase)
            }
            _ => None,
        }
    }
}

/// Browse source-reported outcomes, documented procedures and negative cases.
/// A report or closed work item never becomes proof that Lore executed it.
pub fn cases(conn: &Connection, query: &str, max_tokens: usize) -> Result<CaseResult> {
    with_snapshot(conn, || {
        let context = selected(conn, query, max_tokens)?;
        let mut coverage = coverage(&context);
        let mut details = evidence::Details::new(conn)?;
        let mut cases = Vec::new();
        for record in context.sections.items() {
            let Some(facets) = details.facets(record)? else {
                coverage.omitted_complete_items += 1;
                coverage.candidate_limit_reached = true;
                continue;
            };
            let Some(kind) = record_case(record, &facets) else {
                continue;
            };
            if cases.len() >= MAX_CASES {
                coverage.omitted_complete_items += 1;
                coverage.candidate_limit_reached = true;
                continue;
            }
            let id = format!(
                "case_{}",
                &util::json_digest(&("documentary-case-v1", &record.id, &record.evidence_ids))?
                    [7..]
            );
            cases.push(WorkedCase {
                id, title: record.subject.clone(), kind, record: Some(record.clone()),
                imported_record: None, facets, evidence_ids: record.evidence_ids.clone(),
                qualification: "A retained documentary case. Reported outcomes and expected behavior keep their distinct source meanings; Lore has not executed or replayed this case.".into(),
            });
        }
        for record in &context.imported_observations {
            if cases.len() >= MAX_CASES {
                coverage.omitted_complete_items += 1;
                coverage.candidate_limit_reached = true;
                continue;
            }
            let id = format!(
                "case_{}",
                &util::json_digest(&("imported-case-v1", &record.id, &record.evidence_ids))?[7..]
            );
            cases.push(WorkedCase {
                id, title: record.title.clone(),
                kind: if record.kind == ObservationKind::WorkState { CaseKind::WorkHistory } else { CaseKind::ImportedReport },
                record: None, imported_record: Some(record.clone()), facets: Vec::new(),
                evidence_ids: record.evidence_ids.clone(),
                qualification: "Source-owned imported history. Closure, reported verification and implementation claims retain their recorded scope; they do not prove current code behavior, deployment or execution by Lore.".into(),
            });
        }
        let mut result = CaseResult {
            schema_version: INSIGHTS_SCHEMA_VERSION,
            query: query.into(),
            registry_revision: storage::registry_revision(conn)?,
            generation_basis: "deterministic_source_cases".into(),
            model_calls: 0,
            runtime_execution: false,
            tests_run: 0,
            cases,
            context,
            coverage,
            budget: budget(max_tokens),
        };
        fit_cases(&mut result)?;
        Ok(result)
    })
}

fn fit_decisions(result: &mut DecisionLenses) -> Result<()> {
    loop {
        result.budget.used_tokens = 0;
        for _ in 0..8 {
            let used = context::count_tokens(&(serde_json::to_string(result)? + "\n"))
                .max(context::count_tokens(&render_decisions(result)))
                .max(result.budget.used_tokens);
            if used == result.budget.used_tokens {
                break;
            }
            result.budget.used_tokens = used;
        }
        if result.budget.used_tokens <= result.budget.max_tokens {
            return Ok(());
        }
        if result.lenses.pop().is_some() || result.negative_knowledge.pop().is_some() {
            result.coverage.omitted_complete_items += 1;
        } else {
            return Err(invalid_budget());
        }
    }
}

fn fit_cases(result: &mut CaseResult) -> Result<()> {
    loop {
        result.budget.used_tokens = 0;
        for _ in 0..8 {
            let used = context::count_tokens(&(serde_json::to_string(result)? + "\n"))
                .max(context::count_tokens(&render_cases(result)))
                .max(result.budget.used_tokens);
            if used == result.budget.used_tokens {
                break;
            }
            result.budget.used_tokens = used;
        }
        if result.budget.used_tokens <= result.budget.max_tokens {
            return Ok(());
        }
        if result.cases.pop().is_some() {
            result.coverage.omitted_complete_items += 1;
        } else {
            return Err(invalid_budget());
        }
    }
}
