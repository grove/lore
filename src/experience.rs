//! Human explanations, source tours and deliberate learning over the shared
//! intelligence engine. These are disposable presentations, not project facts
//! or a learner profile. No command in this module edits or executes a project.

mod learning;
mod model;
mod render;
mod source;

pub use learning::{
    ActivityKind, Feedback, FeedbackBasis, FeedbackOutcome, LearningActivity, LearningGraph,
    LearningPath, LearningStage, Prerequisite,
};
pub use render::render_markdown;

use crate::{
    config::ResolvedConfig,
    context::{
        self, ContextBudget, ContextOptions,
        adaptive::{self, AdaptiveResult, Capabilities, SnapshotManifest},
        decision::{
            GenerationBasis,
            runtime::{self, DecisionContextResult, RunOptions},
        },
        inspection::InspectionSession,
        investigation::Budget,
    },
    http::HttpModel,
    inference::GenerativeModel,
    storage, util,
};
use anyhow::{Result, ensure};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, time::Instant};

pub const EXPERIENCE_SCHEMA_VERSION: u32 = 1;
pub const DEFAULT_MAX_TOKENS: usize = 6_000;
pub const EXPERIENCE_PROMPT_VERSION: &str = "human-experience-v1";

/// Releasing the read savepoint on drop also covers a caller cancelling or
/// dropping this future while a model is pending. It performs no registry write.
struct ReadSnapshot<'a> {
    conn: &'a Connection,
    active: bool,
}

impl<'a> ReadSnapshot<'a> {
    fn begin(conn: &'a Connection) -> Result<Self> {
        conn.execute_batch("SAVEPOINT lore_human_experience")?;
        Ok(Self { conn, active: true })
    }
    fn finish(mut self) -> Result<()> {
        self.conn.execute_batch("RELEASE lore_human_experience")?;
        self.active = false;
        Ok(())
    }
}

impl Drop for ReadSnapshot<'_> {
    fn drop(&mut self) {
        if self.active {
            let _ = self.conn.execute_batch("RELEASE lore_human_experience");
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExperienceMode {
    #[default]
    Auto,
    Explanation,
    HowTo,
    Tutorial,
    Reference,
}

impl ExperienceMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Explanation => "explanation",
            Self::HowTo => "how-to",
            Self::Tutorial => "tutorial",
            Self::Reference => "reference",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExperienceOptions {
    pub goal: Option<String>,
    pub task: Option<String>,
    pub mode: ExperienceMode,
    pub paths: Vec<String>,
    pub max_tokens: usize,
    pub hint_level: u8,
    pub show_solution: bool,
    pub answer: Option<String>,
    /// Optional source-bound lesson revision from a previous response. The
    /// learner's answer is assessed only against this exact lesson.
    pub lesson: Option<String>,
    pub activity: LearningStage,
    pub run: RunOptions,
}

impl Default for ExperienceOptions {
    fn default() -> Self {
        Self {
            goal: None,
            task: None,
            mode: ExperienceMode::Auto,
            paths: Vec::new(),
            max_tokens: DEFAULT_MAX_TOKENS,
            hint_level: 0,
            show_solution: false,
            answer: None,
            lesson: None,
            activity: LearningStage::Primary,
            run: RunOptions::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimBasis {
    Documentary,
    ImportedReport,
    StaticInference,
    Inference,
    Hypothetical,
}

impl ClaimBasis {
    pub fn label(self) -> &'static str {
        match self {
            Self::Documentary => "documented",
            Self::ImportedReport => "imported report",
            Self::StaticInference => "static inference",
            Self::Inference => "inference",
            Self::Hypothetical => "practice scenario",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Claim {
    pub text: String,
    pub basis: ClaimBasis,
    pub evidence_ids: Vec<String>,
    pub observation_ids: Vec<String>,
    pub qualifications: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Concept {
    pub id: String,
    pub title: String,
    pub description: Claim,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TourRole {
    Trigger,
    Entry,
    Transition,
    Outcome,
    Boundary,
    Failure,
    DocumentedWorkflow,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TourStop {
    pub title: String,
    pub role: TourRole,
    pub description: Claim,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Workflow {
    pub title: String,
    /// True only for a proposed ordering that passed the source-support check.
    /// It never means that Lore observed the workflow executing.
    pub sequence_supported: bool,
    pub stops: Vec<TourStop>,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExploreNext {
    pub label: String,
    pub goal: String,
    pub mode: ExperienceMode,
    pub evidence_ids: Vec<String>,
    pub observation_ids: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Orientation {
    pub purpose: Option<Claim>,
    pub rationale: Option<Claim>,
    pub concepts: Vec<Concept>,
    pub architecture: Vec<Claim>,
    pub workflow: Option<Workflow>,
    /// Full selected rules/decisions and their qualifications are copied from
    /// the shared evidence, never substituted by a model summary.
    pub constraints: Vec<Claim>,
    pub next_exploration: Vec<ExploreNext>,
    pub gaps: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExactReference {
    pub evidence_id: String,
    pub source: String,
    pub revision_id: String,
    pub content_hash: String,
    pub current: bool,
    pub basis: String,
    pub excerpt: String,
    pub line_start: Option<i64>,
    pub line_end: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirstTask {
    pub task: String,
    /// The user supplied the goal; this is not a discovered or assigned issue.
    pub provenance: String,
    pub readiness: String,
    pub approach: String,
    pub completion_checks: Vec<String>,
    pub qualification: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperienceResult {
    pub schema_version: u32,
    pub project: String,
    pub goal: String,
    pub mode: ExperienceMode,
    pub generation_basis: GenerationBasis,
    pub snapshot: SnapshotManifest,
    pub capabilities: Capabilities,
    pub revision_key: String,
    pub orientation: Orientation,
    pub references: Vec<ExactReference>,
    pub tutorial: Option<LearningPath>,
    pub first_task: Option<FirstTask>,
    pub feedback: Option<Feedback>,
    /// The original, independently versioned shared answer and evidence
    /// manifest. Human claims resolve against this exact result.
    pub intelligence: DecisionContextResult,
    /// The same source-owned groups retained by the explicit schema-5 engine.
    #[serde(
        default,
        skip_serializing_if = "adaptive::SourceRelationships::is_empty"
    )]
    pub source_relationships: adaptive::SourceRelationships,
    pub budget: ContextBudget,
    pub model_calls: u32,
    pub presentation_model_calls: u32,
    pub model_call_limit: u32,
    pub presentation_status: String,
    pub presentation_revalidation: PresentationRevalidation,
    pub omitted_items: usize,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresentationRevalidation {
    pub status: String,
    pub files_rehashed: usize,
    pub bytes_read: usize,
    pub elapsed_ms: u64,
}

pub fn resolve_mode(options: &ExperienceOptions) -> ExperienceMode {
    if options.mode != ExperienceMode::Auto {
        return options.mode;
    }
    if options.answer.is_some()
        || options.lesson.is_some()
        || options.hint_level > 0
        || options.show_solution
        || options.activity == LearningStage::Transfer
    {
        return ExperienceMode::Tutorial;
    }
    if options.task.is_some() {
        return ExperienceMode::HowTo;
    }
    let text = options.goal.as_deref().unwrap_or("").to_lowercase();
    if ["tutorial", "teach me", "learn by", "practice", "exercise"]
        .iter()
        .any(|s| text.contains(s))
    {
        ExperienceMode::Tutorial
    } else if [
        "exact ",
        "reference",
        "signature",
        "default value",
        "parameter",
        "what value",
    ]
    .iter()
    .any(|s| text.contains(s))
    {
        ExperienceMode::Reference
    } else if [
        "how to ",
        "implement ",
        "refactor ",
        "fix ",
        "add ",
        "change ",
    ]
    .iter()
    .any(|s| text.starts_with(s))
    {
        ExperienceMode::HowTo
    } else {
        ExperienceMode::Explanation
    }
}

fn validate_options(options: &ExperienceOptions) -> Result<()> {
    ensure!(
        (512..=context::MAX_MAX_TOKENS).contains(&options.max_tokens),
        "onboard --max-tokens must be 512..100000"
    );
    for text in [&options.goal, &options.task].into_iter().flatten() {
        ensure!(
            !text.trim().is_empty() && text.len() <= 4_000 && !text.chars().any(char::is_control),
            "goal and task need 1..4000 UTF-8 bytes without control characters"
        );
    }
    if let Some(answer) = &options.answer {
        ensure!(
            !answer.trim().is_empty()
                && answer.len() <= 8_000
                && !answer
                    .chars()
                    .any(|c| c.is_control() && !matches!(c, '\n' | '\t')),
            "a learning answer needs 1..8000 UTF-8 bytes without unsafe control characters"
        );
    }
    if let Some(lesson) = &options.lesson {
        ensure!(
            lesson
                .strip_prefix("blake3:")
                .is_some_and(|id| id.len() == 64 && id.bytes().all(|b| b.is_ascii_hexdigit())),
            "--lesson must be a returned lesson revision key"
        );
    }
    ensure!(options.hint_level <= 3, "--hint must be 0..3");
    ensure!(
        resolve_mode(options) == ExperienceMode::Tutorial
            || (options.answer.is_none()
                && options.lesson.is_none()
                && options.hint_level == 0
                && !options.show_solution
                && options.activity == LearningStage::Primary),
        "learning answers, hints and solutions require tutorial mode"
    );
    ensure!(
        options.paths.len() <= 32
            && options
                .paths
                .iter()
                .all(|p| !p.is_empty() && p.len() <= 4096 && !p.chars().any(char::is_control)),
        "invalid path hints"
    );
    Ok(())
}

fn query(conn: &Connection, project: &str, options: &ExperienceOptions) -> Result<String> {
    if let Some(task) = &options.task {
        return Ok(task.trim().to_string());
    }
    if let Some(goal) = &options.goal {
        return Ok(goal.trim().to_string());
    }
    // An orientation has no keyword supplied by the newcomer. Seed the
    // existing retriever from a bounded inventory of actual current subjects;
    // do not depend on every project using the same words for its architecture.
    let mut subjects = BTreeSet::new();
    for record in storage::views(conn)? {
        if matches!(
            record.kind.as_str(),
            "design" | "procedure" | "constraint" | "decision"
        ) && !matches!(record.lifecycle.as_str(), "rejected" | "superseded")
            && record.support_state != "historical_only"
        {
            subjects.insert(record.subject);
        }
        if subjects.len() >= 24 {
            break;
        }
    }
    let mut text =
        format!("Understand {project}: purpose architecture workflow concepts constraints");
    for subject in subjects {
        if text.len() + subject.len() + 2 <= 2_000 && !subject.chars().any(char::is_control) {
            text.push_str("; ");
            text.push_str(&subject);
        }
    }
    Ok(text)
}

/// CLI entry. Holds one registry read snapshot through investigation and view
/// support checking. Answers are never included in the shared task/cache key.
pub async fn run(
    config: &ResolvedConfig,
    conn: &Connection,
    options: &ExperienceOptions,
) -> Result<ExperienceResult> {
    validate_options(options)?;
    let started = Instant::now();
    let (mut authorized, _, _) = adaptive::authorized_config(config, &options.run)?;
    authorized.config.processing.retry_attempts = 0;
    let limits = authorized.config.context.investigation;
    let mut engine_config = config.clone();
    engine_config.config.processing.retry_attempts = 0;
    if limits.max_model_calls >= 3 {
        engine_config.config.context.investigation.max_model_calls -= 2;
    }
    let mode = resolve_mode(options);
    let snapshot_guard = ReadSnapshot::begin(conn)?;
    let result = async {
        let context_options = ContextOptions {
            task: query(conn, &authorized.config.project.name, options)?,
            paths: options.paths.clone(),
            max_tokens: (options.max_tokens * 2 / 3).max(context::MIN_MAX_TOKENS),
        };
        let shared = if mode == ExperienceMode::Reference {
            let selected = context::build_context(conn, &context_options)?;
            adaptive::build(
                conn,
                &engine_config,
                &context_options,
                selected,
                None,
                &options.run,
            )
            .await?
        } else {
            adaptive::run(&engine_config, conn, &context_options, &options.run).await?
        };
        let model = if mode == ExperienceMode::Reference {
            None
        } else {
            HttpModel::new(&authorized, &authorized.config.models.generative).ok()
        };
        let mut budget = Budget::with_usage(limits, started, model_calls(&shared.intelligence))?;
        build_with_budget(
            conn,
            &authorized,
            options,
            shared,
            model.as_ref().map(|m| m as &dyn GenerativeModel),
            &mut budget,
        )
        .await
    }
    .await;
    let release = snapshot_guard.finish();
    match result {
        Ok(result) => {
            release?;
            Ok(result)
        }
        Err(error) => {
            let _ = release;
            Err(error)
        }
    }
}

/// Provider-neutral presentation adapter. The supplied intelligence is still
/// resolved against exact retained evidence; provider permission is rechecked.
/// This is useful to embedders and contract tests without a network provider.
pub async fn build(
    conn: &Connection,
    config: &ResolvedConfig,
    options: &ExperienceOptions,
    shared: AdaptiveResult,
    model: Option<&dyn GenerativeModel>,
) -> Result<ExperienceResult> {
    validate_options(options)?;
    let snapshot_guard = ReadSnapshot::begin(conn)?;
    let (authorized, _, capabilities) = adaptive::authorized_config(config, &options.run)?;
    ensure!(
        shared.capabilities == capabilities,
        "human presentation requires the same current capability envelope as its shared intelligence"
    );
    let mut budget = Budget::with_usage(
        authorized.config.context.investigation,
        Instant::now(),
        model_calls(&shared.intelligence),
    )?;
    let result = build_with_budget(conn, &authorized, options, shared, model, &mut budget).await;
    snapshot_guard.finish()?;
    result
}

async fn build_with_budget(
    conn: &Connection,
    config: &ResolvedConfig,
    options: &ExperienceOptions,
    mut shared: AdaptiveResult,
    model: Option<&dyn GenerativeModel>,
    budget: &mut Budget,
) -> Result<ExperienceResult> {
    let initial_calls = budget.model_calls();
    ensure!(
        shared.snapshot == SnapshotManifest::capture(config, conn)?,
        "human presentation requires the same project and registry snapshot as its shared intelligence"
    );
    let shared_task = match &shared.intelligence {
        DecisionContextResult::Brief(shared) => &shared.task,
        DecisionContextResult::FastFallback(shared) => &shared.context.task,
    };
    ensure!(
        shared_task == &query(conn, &config.config.project.name, options)?,
        "human presentation goal differs from the shared intelligence task"
    );
    if let DecisionContextResult::Brief(shared) = &shared.intelligence
        && !shared.inspection.observations.is_empty()
    {
        let expected_root = config
            .config
            .context
            .inspection
            .root
            .as_ref()
            .map(|root| util::absolute(&config.base, root))
            .transpose()?;
        ensure!(
            config.config.context.inspection.enabled
                && expected_root.as_ref().is_some_and(|root| shared
                    .inspection
                    .root
                    .as_ref()
                    .is_some_and(|captured| root == &std::path::PathBuf::from(captured))),
            "shared checkout observations are outside the current human-view inspection grant"
        );
    }
    let mode = resolve_mode(options);
    let mut catalog =
        source::Catalog::load(conn, &shared.intelligence, &shared.source_relationships)?;
    let mut orientation = catalog.orientation();
    let mut tutorial = if mode == ExperienceMode::Tutorial {
        learning::fallback_path(
            &catalog,
            &orientation.concepts,
            &shared.snapshot.registry_revision,
        )
    } else {
        None
    };
    let mut feedback = options.answer.as_ref().and_then(|_| {
        tutorial.as_ref().map(|path| {
            learning::fallback_feedback(path.activity(options.activity), options.hint_level)
        })
    });
    let mut generation_basis = GenerationBasis::DeterministicFallback;
    let mut status = if mode == ExperienceMode::Reference {
        "exact_reference"
    } else {
        "source_fallback"
    }
    .to_string();
    if mode != ExperienceMode::Reference
        && let Some(model) = model
    {
        match model::generate(
            config,
            options,
            &catalog,
            &shared.snapshot.registry_revision,
            model,
            budget,
        )
        .await
        {
            Ok(generated) => {
                orientation = generated.orientation;
                // A draft cannot omit or shorten a selected adopted rule.
                orientation.constraints = catalog.constraints();
                tutorial = generated
                    .tutorial
                    .map(|draft| {
                        learning::build_path(
                            draft,
                            &orientation.concepts,
                            &shared.snapshot.registry_revision,
                        )
                    })
                    .transpose()?;
                feedback = generated.feedback.or_else(|| {
                    options.answer.as_ref().and_then(|_| {
                        tutorial.as_ref().map(|path| {
                            learning::fallback_feedback(
                                path.activity(options.activity),
                                options.hint_level,
                            )
                        })
                    })
                });
                generation_basis = GenerationBasis::ModelAssessed;
                status = generated.status;
            }
            Err(reason) => {
                status = reason;
            }
        }
    }
    let presentation_revalidation = revalidate_checkout(config, &shared.intelligence, budget);
    if presentation_revalidation.status == "changed_or_unavailable"
        || presentation_revalidation.status == "budget_exhausted"
    {
        // No current guidance may retain static premises whose files changed
        // while the human view was being generated. Keep coherent documentary
        // help and the actual attempted model count; never relabel stale code.
        (shared.intelligence, shared.source_relationships) = documentary_fallback(
            conn,
            config,
            &shared.intelligence,
            &shared.source_relationships,
        )
        .await?;
        catalog = source::Catalog::load(conn, &shared.intelligence, &shared.source_relationships)?;
        orientation = catalog.orientation();
        tutorial = if mode == ExperienceMode::Tutorial {
            learning::fallback_path(
                &catalog,
                &orientation.concepts,
                &shared.snapshot.registry_revision,
            )
        } else {
            None
        };
        feedback = options.answer.as_ref().and_then(|_| {
            tutorial.as_ref().map(|path| {
                learning::fallback_feedback(path.activity(options.activity), options.hint_level)
            })
        });
        generation_basis = GenerationBasis::DeterministicFallback;
        status = "documentary_fallback_after_revalidation".into();
    }
    if let Some(path) = &mut tutorial {
        path.reveal(options.activity, options.hint_level, options.show_solution);
    }
    let references = if mode == ExperienceMode::Reference {
        catalog.references(
            options
                .task
                .as_ref()
                .or(options.goal.as_ref())
                .map(String::as_str)
                .unwrap_or(""),
        )
    } else {
        Vec::new()
    };
    let goal = options
        .task
        .as_ref()
        .or(options.goal.as_ref())
        .cloned()
        .unwrap_or_else(|| "Understand the project".into());
    let first_task = (mode == ExperienceMode::HowTo || options.task.is_some())
        .then(|| first_task(&shared.intelligence, &goal, options.task.is_some()));
    let mut result = ExperienceResult {
        schema_version: EXPERIENCE_SCHEMA_VERSION,
        project: config.config.project.name.clone(),
        goal,
        mode,
        generation_basis,
        snapshot: shared.snapshot,
        capabilities: shared.capabilities,
        revision_key: String::new(),
        orientation,
        references,
        tutorial,
        first_task,
        feedback,
        intelligence: shared.intelligence,
        source_relationships: shared.source_relationships,
        budget: ContextBudget {
            max_tokens: options.max_tokens,
            used_tokens: 0,
            tokenizer: "cl100k_base".into(),
        },
        model_calls: budget.model_calls(),
        presentation_model_calls: budget.model_calls().saturating_sub(initial_calls),
        model_call_limit: budget.report().max_model_calls,
        presentation_status: status,
        presentation_revalidation,
        omitted_items: 0,
        warnings: Vec::new(),
    };
    if mode == ExperienceMode::Reference {
        result.orientation = Orientation {
            constraints: result.orientation.constraints,
            ..Default::default()
        };
    }
    if options.answer.is_some() {
        result.warnings.push("Feedback is a fallible source comparison. No answer or learner profile is saved; no test execution, contribution correctness or mastery is established.".into());
    }
    if options.lesson.as_ref().is_some_and(|expected| {
        result
            .tutorial
            .as_ref()
            .is_none_or(|path| &path.revision_key != expected)
    }) {
        result.warnings.push("The requested lesson revision does not match the current source-bound lesson. Earlier work is not assessed against a different exercise; use the current sources and lesson revision.".into());
    } else if options.answer.is_some() && options.lesson.is_none() {
        result.warnings.push("No previous lesson revision was supplied. Feedback is a source comparison; add the returned --lesson revision to assess an answer against that exact activity.".into());
    }
    result.revision_key = util::json_digest(&(
        EXPERIENCE_PROMPT_VERSION,
        &result.snapshot,
        &result.source_relationships,
        &result.orientation,
        &result.tutorial,
        &result.references,
    ))?;
    fit(result)
}

fn revalidate_checkout(
    config: &ResolvedConfig,
    intelligence: &DecisionContextResult,
    budget: &Budget,
) -> PresentationRevalidation {
    let mut result = PresentationRevalidation {
        status: "not_needed".into(),
        files_rehashed: 0,
        bytes_read: 0,
        elapsed_ms: 0,
    };
    let DecisionContextResult::Brief(shared) = intelligence else {
        return result;
    };
    if shared.inspection.observations.is_empty() {
        return result;
    }
    let prior = &shared.inspection.budget;
    let mut settings = config.config.context.inspection.clone();
    let remaining_bytes = settings.max_total_bytes.saturating_sub(prior.bytes_read);
    let remaining_ms = settings
        .max_elapsed_ms
        .saturating_sub(prior.elapsed_ms)
        .min(
            budget
                .remaining_time()
                .as_millis()
                .min(u128::from(u64::MAX)) as u64,
        );
    if remaining_bytes == 0 || remaining_ms == 0 {
        result.status = "budget_exhausted".into();
        return result;
    }
    settings.max_total_bytes = remaining_bytes;
    settings.max_file_bytes = settings.max_file_bytes.min(remaining_bytes);
    settings.max_elapsed_ms = remaining_ms;
    settings.max_excerpt_lines = settings
        .max_excerpt_lines
        .min((shared.budget.max_tokens / 200).clamp(5, 80));
    let session = InspectionSession::discover(config, &shared.task, &[], &shared.paths, &settings);
    let Ok(mut session) = session else {
        result.status = "changed_or_unavailable".into();
        return result;
    };
    let valid = session
        .revalidate(&shared.inspection.observations)
        .unwrap_or(false);
    let report = session.report();
    result.files_rehashed = report.budget.files_read;
    result.bytes_read = report.budget.bytes_read;
    result.elapsed_ms = report.budget.elapsed_ms;
    result.status = if valid && report.index_digest == shared.inspection.index_digest {
        "validated"
    } else if !report.budget.exhausted.is_empty() {
        "budget_exhausted"
    } else {
        "changed_or_unavailable"
    }
    .into();
    result
}

async fn documentary_fallback(
    conn: &Connection,
    config: &ResolvedConfig,
    previous: &DecisionContextResult,
    previous_relationships: &adaptive::SourceRelationships,
) -> Result<(DecisionContextResult, adaptive::SourceRelationships)> {
    let (task, paths, max_tokens) = match previous {
        DecisionContextResult::Brief(shared) => {
            (&shared.task, &shared.paths, shared.budget.max_tokens)
        }
        DecisionContextResult::FastFallback(shared) => (
            &shared.context.task,
            &shared.context.paths,
            shared.context.budget.max_tokens,
        ),
    };
    let options = ContextOptions {
        task: task.clone(),
        paths: paths.clone(),
        max_tokens: max_tokens
            .checked_add(previous_relationships.token_overhead()?)
            .filter(|budget| *budget <= context::MAX_MAX_TOKENS)
            .ok_or_else(|| anyhow::anyhow!("invalid documentary relationship fallback budget"))?,
    };
    let mut selected = context::build_context(conn, &options)?;
    let (options, relationships) = adaptive::prepare_relationships(conn, &options, &selected)?;
    selected.model_calls = model_calls(previous);
    let mut fallback = runtime::build_decision_context(
        conn,
        config,
        &options,
        selected,
        None,
        &RunOptions {
            no_inspect: true,
            no_cache: true,
            ..Default::default()
        },
    )
    .await?;
    let warning = "Checkout observations could not be revalidated after human presentation. Static claims and generated guidance were discarded; this is current retained documentary context.".to_string();
    match &mut fallback {
        DecisionContextResult::Brief(shared) => shared.warnings.push(warning),
        DecisionContextResult::FastFallback(shared) => shared.context.warnings.push(warning),
    }
    Ok((fallback, relationships))
}

fn first_task(intelligence: &DecisionContextResult, task: &str, supplied: bool) -> FirstTask {
    let (readiness, approach, completion_checks) = match intelligence {
        DecisionContextResult::Brief(result) => (
            result.brief.readiness.label().to_string(),
            result.brief.preferred_approach.text.clone(),
            result
                .brief
                .completion_criteria
                .iter()
                .map(|c| c.text.clone())
                .collect(),
        ),
        DecisionContextResult::FastFallback(_) => (
            "evidence_only".into(),
            "Use the exact relevant project conditions below as the scope for this task.".into(),
            Vec::new(),
        ),
    };
    FirstTask {
        task: task.into(),
        provenance: if supplied { "user_supplied_task" } else { "user_supplied_goal" }.into(),
        readiness, approach, completion_checks,
        qualification: "You retain control of the change. Completion checks are future work, not checks Lore executed; a generated plan does not demonstrate that you learned the project.".into(),
    }
}

fn model_calls(result: &DecisionContextResult) -> u32 {
    match result {
        DecisionContextResult::Brief(result) => result.model_calls,
        DecisionContextResult::FastFallback(result) => result.context.model_calls,
    }
}

fn measure(result: &mut ExperienceResult) -> Result<()> {
    result.budget.used_tokens = 0;
    for _ in 0..8 {
        // JSON is printed with a trailing newline by the CLI. Account for the
        // complete wire output as well as portable Markdown, including this
        // report's own token count.
        let used = context::count_tokens(&(serde_json::to_string(result)? + "\n"))
            .max(context::count_tokens(&render_markdown(result)));
        // The tokenizer can use a different number of tokens for adjacent
        // decimal counts. A monotone upper bound avoids oscillating between
        // two self-referential counts and reporting fewer tokens than printed.
        if used <= result.budget.used_tokens {
            break;
        }
        result.budget.used_tokens = used;
    }
    Ok(())
}

fn fit(mut result: ExperienceResult) -> Result<ExperienceResult> {
    loop {
        measure(&mut result)?;
        if result.budget.used_tokens <= result.budget.max_tokens {
            return Ok(result);
        }
        // Remove whole optional presentation items. The shared answer, its
        // constraints, relationships, observations and manifest stay intact.
        let removed = if result.references.len() > 1 {
            result.references.pop().is_some()
        } else if result.orientation.next_exploration.pop().is_some()
            || result.orientation.architecture.pop().is_some()
        {
            true
        } else if result.orientation.concepts.len() > 1 && result.tutorial.is_none() {
            result.orientation.concepts.pop().is_some()
        } else if result.orientation.rationale.take().is_some() || result.feedback.take().is_some()
        {
            true
        } else if result.tutorial.take().is_some() {
            result.warnings.push("The complete learning activity could not fit this output budget; source evidence is retained.".into());
            true
        } else {
            result.orientation.workflow.take().is_some()
        };
        ensure!(
            removed,
            "The complete source-backed onboarding response requires {} tokens; increase --max-tokens (current {}). Required constraints and evidence were not truncated.",
            result.budget.used_tokens,
            result.budget.max_tokens
        );
        result.omitted_items += 1;
    }
}
