//! Schema 4 orchestration. Inspection and inference are bounded separately;
//! their results and caches never become accepted registry knowledge.

use super::*;
use crate::{
    config::ResolvedConfig,
    context::{
        self, ContextBudget, ContextOmissions, ContextOptions,
        inspection::{InspectionReport, InspectionSession, InspectionSettings},
        intelligence::{self, BriefEvidence},
        investigation::{
            self, Budget, InvestigationCandidate, InvestigationReport, InvestigationStep,
        },
    },
    http::HttpModel,
    inference::{
        EgressPolicy, EmbeddingModel, EmbeddingRequest, EmbeddingResponse, ExecutionLocation,
        GenerationRequest, GenerativeModel, ModelDescriptor, ModelError, ModelFuture, Provider,
    },
    util,
};
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU32, Ordering},
    time::{Duration, Instant},
};

const CACHE_VERSION: u32 = 2;
const MAX_CACHE_BYTES: usize = 1_000_000;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RunOptions {
    pub inspect: bool,
    pub investigate: bool,
    pub no_inspect: bool,
    pub no_cache: bool,
    pub allow_checkout_egress: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckoutEgress {
    pub allowed: bool,
    /// Actual checkout-bearing calls in this invocation, not cached history.
    pub model_received_checkout: bool,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DecisionContextResult {
    Brief(DecisionResult),
    FastFallback(DecisionFallback),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionResult {
    pub schema_version: u32,
    pub task: String,
    pub paths: Vec<String>,
    pub mode: String,
    pub model_calls: u32,
    pub model: String,
    pub cache_status: String,
    pub revision_key: String,
    pub budget: ContextBudget,
    pub brief: DecisionBrief,
    pub evidence: Vec<BriefEvidence>,
    pub inspection: InspectionReport,
    pub investigation: InvestigationReport,
    pub checkout_egress: CheckoutEgress,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_reason: Option<String>,
    pub retrieval_truncated: bool,
    pub omissions: ContextOmissions,
    pub brief_items_omitted: usize,
    pub warnings: Vec<String>,
}

/// Very small output budgets retain the established retrieval shape and an
/// explicit schema 4 fallback, rather than dropping a required qualification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionFallback {
    #[serde(flatten)]
    pub context: ContextResult,
    pub mode: String,
    pub fallback_reason: String,
    pub cache_status: String,
    pub inspection_status: String,
    pub investigation_status: String,
    pub checkout_egress: CheckoutEgress,
    pub model_call_limit: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CachedDecision {
    version: u32,
    question: String,
    created_at: String,
    registry_revision: String,
    permission_key: String,
    key: String,
    revision_key: String,
    content_hash: String,
    draft: DraftDecision,
    verification: DecisionVerification,
    inspection: InspectionReport,
    investigation: InvestigationReport,
}

fn configured(
    config: &ResolvedConfig,
    options: &ContextOptions,
    run: &RunOptions,
) -> ResolvedConfig {
    let mut config = config.clone();
    config.config.context.cache &= !run.no_cache;
    config.config.context.inspection.enabled = !run.no_inspect
        && (run.inspect || run.investigate || config.config.context.inspection.enabled);
    // Bound excerpt overhead before inference, without changing file hashes.
    config.config.context.inspection.max_excerpt_lines = config
        .config
        .context
        .inspection
        .max_excerpt_lines
        .min((options.max_tokens / 200).clamp(5, 80));
    config.config.privacy.allow_checkout_egress |= run.allow_checkout_egress;
    // The query budget counts actual attempts; hidden transport retries could
    // otherwise multiply its model-call allowance.
    config.config.processing.retry_attempts = 0;
    config
}

fn policy(config: &ResolvedConfig) -> EgressPolicy {
    if config.config.privacy.local_only {
        EgressPolicy::LocalOnly
    } else {
        EgressPolicy::ExplicitHosted
    }
}

fn authorize_model(config: &ResolvedConfig, descriptor: &ModelDescriptor) -> Result<()> {
    policy(config)
        .authorize(descriptor)
        .map_err(|_| anyhow::anyhow!("provider egress is disabled"))?;
    if config.config.privacy.local_only {
        ensure!(
            descriptor.location == ExecutionLocation::Local
                && descriptor.provider == Provider::Ollama
                && !descriptor.model.contains(":cloud")
                && !descriptor.model.ends_with("-cloud"),
            "local-only policy forbids this inference model"
        );
    }
    Ok(())
}

fn checkout_permission(
    config: &ResolvedConfig,
    model: Option<&dyn GenerativeModel>,
) -> CheckoutEgress {
    let hosted = model.is_some_and(|model| {
        let descriptor = model.descriptor();
        descriptor.provider != Provider::Ollama
            || descriptor.location == ExecutionLocation::Hosted
            || descriptor.model.contains(":cloud")
            || descriptor.model.ends_with("-cloud")
    });
    let allowed = model.is_some()
        && (!hosted || config.config.privacy.allow_checkout_egress)
        && model.is_some_and(|model| authorize_model(config, model.descriptor()).is_ok());
    CheckoutEgress {
        allowed,
        model_received_checkout: false,
        reason: if model.is_none() {
            "no_model"
        } else if allowed && hosted {
            "explicit_checkout_permission"
        } else if allowed {
            "local_model"
        } else {
            "checkout_content_withheld"
        }
        .into(),
    }
}

/// CLI entry point: bounded hybrid retrieval, then schema 4 reasoning. The
/// savepoint supplies one consistent registry snapshot without writing it.
pub async fn run(
    config: &ResolvedConfig,
    conn: &Connection,
    options: &ContextOptions,
    run: &RunOptions,
) -> Result<DecisionContextResult> {
    context::validate_options(options)?;
    let started = Instant::now();
    let config = configured(config, options, run);
    config.config.context.investigation.validate()?;
    let model = HttpModel::new(&config, &config.config.models.generative).ok();
    conn.execute_batch("SAVEPOINT lore_decision_context")?;
    let result = async {
        let selected = select_evidence(conn, &config, options, model.is_some(), started).await?;
        let budget = Budget::with_usage(
            config.config.context.investigation,
            started,
            selected.model_calls,
        )?;
        build(
            conn,
            &config,
            options,
            selected,
            model.as_ref().map(|m| m as &dyn GenerativeModel),
            run,
            budget,
        )
        .await
    }
    .await;
    let released = conn.execute_batch("RELEASE lore_decision_context");
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

/// Provider-neutral entry point for embedders and deterministic integration
/// tests. Selected evidence receives the same validation as the CLI path.
pub async fn build_decision_context(
    conn: &Connection,
    config: &ResolvedConfig,
    options: &ContextOptions,
    selected: ContextResult,
    model: Option<&dyn GenerativeModel>,
    run: &RunOptions,
) -> Result<DecisionContextResult> {
    context::validate_options(options)?;
    let config = configured(config, options, run);
    let budget = Budget::with_usage(
        config.config.context.investigation,
        Instant::now(),
        selected.model_calls,
    )?;
    build(conn, &config, options, selected, model, run, budget).await
}

struct LimitedEmbedding<'a> {
    model: &'a dyn EmbeddingModel,
    calls: AtomicU32,
    limit: u32,
}
impl EmbeddingModel for LimitedEmbedding<'_> {
    fn descriptor(&self) -> &ModelDescriptor {
        self.model.descriptor()
    }
    fn cache_identity(&self) -> String {
        self.model.cache_identity()
    }
    #[allow(deprecated)] // fetch_update also supports pre-1.99 stable toolchains.
    fn embed<'a>(&'a self, request: &'a EmbeddingRequest) -> ModelFuture<'a, EmbeddingResponse> {
        Box::pin(async move {
            self.calls
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| {
                    (n < self.limit).then_some(n + 1)
                })
                .map_err(|_| {
                    ModelError::Unavailable("query embedding call budget exhausted".into())
                })?;
            self.model.embed(request).await
        })
    }
}

async fn select_evidence(
    conn: &Connection,
    config: &ResolvedConfig,
    options: &ContextOptions,
    enabled: bool,
    started: Instant,
) -> Result<ContextResult> {
    let mut warnings = Vec::new();
    let mut calls = 0;
    let mut semantic = None;
    let input_bytes = config
        .config
        .processing
        .max_context_bytes
        .saturating_sub(16_000);
    if enabled && input_bytes >= 2048 {
        if let Some(role) = config
            .config
            .models
            .embedding
            .as_ref()
            .filter(|r| r.enabled)
        {
            match HttpModel::new(config, role) {
                Ok(embedding) => {
                    let limited = LimitedEmbedding { model: &embedding, calls: AtomicU32::new(0),
                        limit: config.config.context.investigation.max_model_calls.saturating_sub(2).min(2) };
                    let cache = if config.config.context.cache {
                        context::semantic::open_cache(&config.state.join("semantic.sqlite3")).or_else(|_| {
                            warnings.push("Semantic cache unavailable; using a temporary index.".into());
                            Connection::open_in_memory().map_err(anyhow::Error::from)
                        })?
                    } else { Connection::open_in_memory()? };
                    let remaining = Duration::from_secs(config.config.context.investigation.timeout_seconds)
                        .saturating_sub(started.elapsed()).min(Duration::from_secs(10));
                    let outcome = tokio::time::timeout(remaining, context::semantic::search(conn, &cache,
                        &limited, policy(config), &options.task, &options.paths,
                        config.config.processing.max_context_bytes)).await;
                    calls = limited.calls.load(Ordering::SeqCst);
                    match outcome {
                        Ok(Ok(report)) => { warnings.extend(report.warnings.clone()); semantic = Some(report); },
                        _ => warnings.push("Semantic retrieval unavailable or its query budget was exhausted; using lexical and recorded relationship retrieval.".into()),
                    }
                },
                Err(_) => warnings.push("Embedding configuration unavailable or disallowed; using lexical and recorded relationship retrieval.".into()),
            }
        }
    }
    let mut input_options = options.clone();
    if enabled {
        input_options.max_tokens = (input_bytes / 4).clamp(context::MIN_MAX_TOKENS, 12_000);
    }
    let mut selected = loop {
        let candidate = match &semantic {
            Some(report) => context::build_context_with_semantic(conn, &input_options, report),
            None => context::build_context(conn, &input_options),
        };
        let candidate = match candidate {
            Ok(candidate) => candidate,
            Err(error)
                if error
                    .downcast_ref::<context::ContextError>()
                    .is_some_and(|e| e.code == "invalid_budget") =>
            {
                break context::build_context(conn, options)?;
            }
            Err(error) => return Err(error),
        };
        if !enabled
            || serde_json::to_vec(&candidate)?.len() <= input_bytes
            || input_options.max_tokens <= 512
        {
            break candidate;
        }
        input_options.max_tokens = (input_options.max_tokens * 3 / 4).max(512);
    };
    selected.model_calls = calls;
    selected.warnings.extend(warnings);
    Ok(selected)
}

fn discover(
    config: &ResolvedConfig,
    options: &ContextOptions,
    selected: &ContextResult,
) -> Result<InspectionSession> {
    match InspectionSession::discover(
        config,
        &options.task,
        &selected.suggested_inspection,
        &options.paths,
        &config.config.context.inspection,
    ) {
        Ok(session) => Ok(session),
        Err(_) => InspectionSession::discover(
            config,
            &options.task,
            &[],
            &[],
            &InspectionSettings::default(),
        ),
    }
}

fn inspection_report(session: &InspectionSession, config: &ResolvedConfig) -> InspectionReport {
    let mut report = session.report();
    if config.config.context.inspection.enabled && report.status == "disabled" {
        report.status = "unavailable".into();
        report.warnings.push("Checkout inspection is unavailable or its settings failed validation; using retained knowledge.".into());
    }
    report
}

fn validate_plan_text(
    plan: &investigation::InvestigationPlan,
    retained: &EvidenceCatalog,
    observations: &[CodeObservation],
    candidates: &[InvestigationCandidate],
) -> Result<()> {
    let mut catalog = DecisionCatalog::new(retained, observations)?;
    catalog
        .paths
        .extend(candidates.iter().map(|candidate| candidate.path.clone()));
    for text in [
        &plan.uncertainty,
        &plan.hypothesis,
        &plan.expected_discriminator,
    ] {
        if !text.is_empty() {
            catalog.text(text)?;
        }
    }
    Ok(())
}

fn validate_cached_trace(
    report: &InvestigationReport,
    retained: &EvidenceCatalog,
    observations: &[CodeObservation],
) -> Result<()> {
    let catalog = DecisionCatalog::new(retained, observations)?;
    ensure!(
        report.steps.len() <= 2,
        "cached investigation exceeds round limit"
    );
    for step in &report.steps {
        ensure!(
            InvestigationCandidate::new(step.path.clone())?.id == step.candidate_id,
            "cached investigation candidate identity changed"
        );
        ensure!(
            !step.observation_ids.is_empty()
                && step.observation_ids.iter().all(|id| observations
                    .iter()
                    .any(|observation| observation.id == *id && observation.path == step.path)),
            "cached investigation lost its inspected evidence"
        );
        for text in [
            &step.uncertainty,
            &step.initial_hypothesis,
            &step.discriminating_check,
            &step.previous_recommendation,
            &step.revised_recommendation,
        ] {
            catalog.text(text)?;
        }
        for item in &step.counterevidence {
            catalog.text(&item.hypothesis)?;
            catalog.text(&item.explanation)?;
            catalog.references(&item.evidence_ids, &item.observation_ids, true)?;
        }
    }
    for uncertainty in &report.remaining_uncertainty {
        catalog.text(uncertainty)?;
    }
    Ok(())
}

fn trace_references(report: &InvestigationReport) -> Result<(BTreeSet<String>, BTreeSet<String>)> {
    let text = serde_json::to_string(&(&report.steps, &report.remaining_uncertainty))?;
    let mut evidence = BTreeSet::new();
    let mut observations = BTreeSet::new();
    for word in text.split(|c: char| !c.is_alphanumeric() && c != '_') {
        if word.starts_with("ev_") || word.starts_with("ne_") {
            evidence.insert(word.into());
        }
        if word.starts_with("co_") {
            observations.insert(word.into());
        }
    }
    Ok((evidence, observations))
}

fn validate_trace_verification(
    report: &InvestigationReport,
    verification: &DecisionVerification,
) -> Result<()> {
    let (evidence, observations) = trace_references(report)?;
    ensure!(
        evidence
            .iter()
            .all(|id| verification.checked_evidence_ids.contains(id))
            && observations
                .iter()
                .all(|id| verification.checked_observation_ids.contains(id)),
        "support check omitted investigation evidence"
    );
    Ok(())
}

fn revision_key(key: &str, observations: &[CodeObservation]) -> Result<String> {
    util::json_digest(&(key, observations))
}

fn cache_hash(entry: &CachedDecision) -> Result<String> {
    util::json_digest(&(
        &entry.question,
        &entry.created_at,
        &entry.registry_revision,
        &entry.permission_key,
        &entry.key,
        &entry.revision_key,
        &entry.draft,
        &entry.verification,
        &entry.inspection,
        &entry.investigation,
    ))
}

fn cache_path(config: &ResolvedConfig, key: &str) -> PathBuf {
    config
        .state
        .join("context-cache")
        .join(format!("decision-{}.json", &key[7..]))
}

async fn generate(
    model: &dyn GenerativeModel,
    config: &ResolvedConfig,
    request: GenerationRequest,
    budget: &mut Budget,
    egress: &mut CheckoutEgress,
    has_checkout: bool,
) -> Result<String> {
    authorize_model(config, model.descriptor())?;
    ensure!(
        !has_checkout || egress.allowed,
        "checkout egress is disabled"
    );
    let bytes = request
        .input
        .len()
        .saturating_add(request.instructions.len())
        .saturating_add(request.schema.as_ref().map_or(0, |s| s.to_string().len()));
    ensure!(
        bytes <= config.config.processing.max_context_bytes,
        "decision input budget exhausted"
    );
    budget.before_model_call()?;
    egress.model_received_checkout |= has_checkout;
    let response = tokio::time::timeout(budget.remaining_time(), model.generate(&request))
        .await
        .map_err(|_| anyhow::anyhow!("decision deadline exhausted"))?
        .map_err(|_| anyhow::anyhow!("decision model unavailable or invalid"))?;
    budget.ensure_time()?;
    let expected = &model.descriptor().model;
    ensure!(
        response.model == *expected
            || model.descriptor().provider == Provider::Ollama
                && (response.model.strip_suffix(":latest") == Some(expected.as_str())
                    || expected.strip_suffix(":latest") == Some(response.model.as_str())),
        "decision response changed models"
    );
    ensure!(
        response.text.len() <= 128_000,
        "oversized decision response"
    );
    Ok(response.text)
}

async fn synthesize(
    model: &dyn GenerativeModel,
    config: &ResolvedConfig,
    options: &ContextOptions,
    retained: &EvidenceCatalog,
    sources: &Value,
    observations: &[CodeObservation],
    previous: Option<&DecisionBrief>,
    plan: Option<&investigation::InvestigationPlan>,
    budget: &mut Budget,
    egress: &mut CheckoutEgress,
    key: &str,
) -> Result<(DraftDecision, DecisionBrief)> {
    let catalog = DecisionCatalog::new(retained, observations)?;
    let mut input = request_input(
        &options.task,
        sources.clone(),
        observations,
        options.max_tokens,
        previous,
    );
    if let Some(plan) = plan {
        input["inspection_question"] = serde_json::to_value(plan)?;
    }
    let request = GenerationRequest {
        instructions: SYNTHESIS_INSTRUCTIONS.into(),
        input: serde_json::to_string(&input)?,
        schema: Some(synthesis_schema(&catalog)),
        reasoning_effort: config.config.models.reasoning.for_task("context_synthesis"),
    };
    let raw = generate(
        model,
        config,
        request,
        budget,
        egress,
        !observations.is_empty(),
    )
    .await?;
    let draft = parse_draft(&raw)?;
    validate_draft(&catalog, &draft)?;
    let brief = finish_brief(&catalog, &draft, &revision_key(key, observations)?);
    Ok((draft, brief))
}

async fn build(
    conn: &Connection,
    config: &ResolvedConfig,
    options: &ContextOptions,
    selected: ContextResult,
    model: Option<&dyn GenerativeModel>,
    run: &RunOptions,
    mut budget: Budget,
) -> Result<DecisionContextResult> {
    let model = model.filter(|_| config.config.models.generative.enabled);
    let mut egress = checkout_permission(config, model);
    let retained = match EvidenceCatalog::validate(conn, &selected) {
        Ok(catalog) => catalog,
        Err(_) => {
            return small_fallback(
                conn,
                options,
                &selected,
                &budget,
                &egress,
                "unavailable",
                "evidence_changed",
                "Retained evidence changed or failed validation; retrieve refreshed context before relying on generated advice.",
            );
        }
    };
    let sources = intelligence::evidence_input(&selected, &retained);
    let mut session = discover(config, options, &selected)?;
    let mut inspection = inspection_report(&session, config);
    let mut investigation = InvestigationReport::new(run.investigate && !run.no_inspect, &budget);
    let identity = model.map(|m| m.cache_identity()).unwrap_or_default();
    let registry_revision = crate::storage::registry_revision(conn)?;
    let permission_key = context::memory::permission_key(config)?;
    let key = util::json_digest(&(
        DECISION_PROMPT_VERSION,
        investigation::INVESTIGATION_PROMPT_VERSION,
        &registry_revision,
        &options.task,
        &options.paths,
        options.max_tokens,
        &sources,
        identity,
        &config.config,
        &config.base,
        &inspection.root,
        &inspection.index_digest,
        run,
    ))?;
    let cache = cache_path(config, &key);
    let can_cache = config.config.context.cache
        && (!config.config.context.inspection.enabled || inspection.index_complete);
    // Privacy authorization precedes cache reads as well as all provider calls.
    if let Some(model) = model.filter(|m| authorize_model(config, m.descriptor()).is_ok()) {
        if can_cache {
            let cached = util::read_limited(&cache, MAX_CACHE_BYTES)
                .ok()
                .and_then(|raw| serde_json::from_str::<CachedDecision>(&raw).ok())
                .filter(|entry| {
                    entry.version == CACHE_VERSION
                        && entry.key == key
                        && entry.registry_revision == registry_revision
                        && entry.permission_key == permission_key
                        && context::memory::fresh(&entry.created_at)
                        && cache_hash(entry).ok().as_ref() == Some(&entry.content_hash)
                });
            if let Some(entry) = cached {
                let files_valid = entry.inspection.observations.is_empty()
                    || session
                        .revalidate(&entry.inspection.observations)
                        .unwrap_or(false);
                let exposed = if egress.allowed {
                    entry.inspection.observations.as_slice()
                } else {
                    &[]
                };
                let validated = DecisionCatalog::new(&retained, exposed).and_then(|catalog| {
                    validate_cached_trace(&entry.investigation, &retained, exposed)?;
                    validate_draft(&catalog, &entry.draft)?;
                    ensure!(
                        revision_key(&key, exposed)? == entry.revision_key,
                        "cache revision changed"
                    );
                    let brief = finish_brief(&catalog, &entry.draft, &entry.revision_key);
                    parse_validate_verification(
                        &serde_json::to_string(&entry.verification)?,
                        &catalog,
                        &brief,
                    )?;
                    validate_trace_verification(&entry.investigation, &entry.verification)?;
                    Ok(brief)
                });
                if files_valid && budget.ensure_time().is_ok() {
                    if let Ok(brief) = validated {
                        let mut current_inspection = inspection_report(&session, config);
                        current_inspection.observations = entry.inspection.observations;
                        let mut report = entry.investigation;
                        report.budget = budget.report();
                        report.stop_reason = "cache_reused".into();
                        let result = assemble(
                            options,
                            &selected,
                            &retained,
                            model,
                            brief,
                            current_inspection,
                            report,
                            &budget,
                            egress.clone(),
                            "hit",
                            None,
                        );
                        if let Some(result) = fit(result)? {
                            if budget.ensure_time().is_ok() {
                                return Ok(DecisionContextResult::Brief(result));
                            }
                        }
                    }
                }
                // A stale cache probe may have read changed files. A fresh
                // discovery preserves their consumed I/O budget via the session;
                // it does not reset limits to make more reads possible.
            }
        }
    }
    if budget.ensure_time().is_err() {
        investigation.finish("time_budget_exhausted", &budget);
        return fallback(
            conn,
            options,
            &selected,
            &retained,
            inspection_report(&session, config),
            investigation,
            &budget,
            egress,
            "The time budget was exhausted before fresh reasoning; this briefing contains deterministic guidance.",
        );
    }
    if config.config.context.inspection.enabled {
        session.inspect_initial();
        inspection = inspection_report(&session, config);
    }
    let mut observations = if egress.allowed {
        inspection.observations.clone()
    } else {
        Vec::new()
    };
    if !egress.allowed && !inspection.observations.is_empty() {
        inspection.warnings.push("Local observations are returned to you; checkout content and discovered filenames were withheld from the configured hosted model. Use explicit checkout egress permission to include them in reasoning.".into());
    }
    let Some(model) = model.filter(|m| authorize_model(config, m.descriptor()).is_ok()) else {
        investigation.finish("inference_unavailable", &budget);
        return fallback(
            conn,
            options,
            &selected,
            &retained,
            inspection,
            investigation,
            &budget,
            egress,
            "The configured model is unavailable, disabled, or disallowed; this briefing contains deterministic guidance without fresh reasoning.",
        );
    };
    if retained.known.is_empty() && observations.is_empty() {
        investigation.finish("no_evidence", &budget);
        return fallback(
            conn,
            options,
            &selected,
            &retained,
            inspection,
            investigation,
            &budget,
            egress,
            "No relevant retained knowledge or inspectable source fits this task; add a precise path or project source.",
        );
    }
    let (mut draft, mut brief) = match synthesize(
        model,
        config,
        options,
        &retained,
        &sources,
        &observations,
        None,
        None,
        &mut budget,
        &mut egress,
        &key,
    )
    .await
    {
        Ok(value) => value,
        Err(_) => {
            investigation.finish("inference_or_validation_failed", &budget);
            return fallback(
                conn,
                options,
                &selected,
                &retained,
                inspection,
                investigation,
                &budget,
                egress,
                "Decision inference was unavailable, exceeded its budget, or failed evidence/readiness validation; using deterministic guidance.",
            );
        }
    };
    if investigation.enabled {
        investigation.stop_reason = if !egress.allowed {
            "checkout_egress_withheld"
        } else if inspection.root.is_none() {
            "inspection_unavailable"
        } else {
            "round_limit"
        }
        .into();
        if egress.allowed && inspection.root.is_some() {
            loop {
                // One planning call and one revision call, plus a final
                // independent support check, must remain before starting.
                if budget.model_calls().saturating_add(3)
                    > config.config.context.investigation.max_model_calls
                {
                    investigation.stop_reason = "model_call_limit".into();
                    break;
                }
                if budget.rounds() >= config.config.context.investigation.max_rounds {
                    break;
                }
                let seen: BTreeSet<_> = inspection
                    .observations
                    .iter()
                    .map(|o| o.path.as_str())
                    .collect();
                let candidates = session
                    .candidates()
                    .iter()
                    .filter(|c| !seen.contains(c.path.as_str()))
                    .filter_map(|c| InvestigationCandidate::new(c.path.clone()).ok())
                    .take(128)
                    .collect::<Vec<_>>();
                if candidates.is_empty() {
                    investigation.stop_reason = "no_candidates".into();
                    break;
                }
                if let Err(reason) = budget.consume_round() {
                    investigation.stop_reason = reason.to_string();
                    break;
                }
                let payload =
                    json!({"retained":sources,"local_observations":observations,"brief":brief});
                let request = match investigation::plan_request(
                    &options.task,
                    &payload,
                    &brief.preferred_approach.text,
                    &candidates,
                    config.config.models.reasoning.for_task("context_synthesis"),
                    config.config.processing.max_context_bytes,
                ) {
                    Ok(request) => request,
                    Err(_) => {
                        investigation.stop_reason = "input_budget".into();
                        break;
                    }
                };
                let plan = match generate(model, config, request, &mut budget, &mut egress, true)
                    .await
                    .and_then(|raw| {
                        let plan = investigation::parse_plan(&raw, &candidates)?;
                        validate_plan_text(&plan, &retained, &observations, &candidates)?;
                        Ok(plan)
                    }) {
                    Ok(plan) => plan,
                    Err(_) => {
                        investigation.stop_reason = "invalid_or_unavailable_plan".into();
                        break;
                    }
                };
                let candidate = investigation::selected_candidate(&plan, &candidates)?;
                let Some(candidate) = candidate else {
                    investigation.stop_reason = "no_useful_inspection".into();
                    if !plan.uncertainty.is_empty() {
                        investigation.remaining_uncertainty.push(plan.uncertainty);
                    }
                    break;
                };
                let observation = match session.inspect(&candidate.path) {
                    Ok(observation) => observation,
                    Err(_) => {
                        investigation.stop_reason = "inspection_budget_or_unavailable".into();
                        break;
                    }
                };
                inspection = inspection_report(&session, config);
                observations = inspection.observations.clone();
                let (next_draft, next_brief) = match synthesize(
                    model,
                    config,
                    options,
                    &retained,
                    &sources,
                    &observations,
                    Some(&brief),
                    Some(&plan),
                    &mut budget,
                    &mut egress,
                    &key,
                )
                .await
                {
                    Ok(value) => value,
                    Err(_) => {
                        investigation.finish("revision_unavailable", &budget);
                        return fallback(
                            conn,
                            options,
                            &selected,
                            &retained,
                            inspection,
                            investigation,
                            &budget,
                            egress,
                            "New inspection evidence could not receive a validated revision; the earlier recommendation was discarded.",
                        );
                    }
                };
                let revision = match validate_revision(&brief, &next_brief) {
                    Ok(revision) => revision,
                    Err(_) => {
                        investigation.finish("counterevidence_not_addressed", &budget);
                        return fallback(
                            conn,
                            options,
                            &selected,
                            &retained,
                            inspection,
                            investigation,
                            &budget,
                            egress,
                            "Material counterevidence did not produce a valid recommendation revision; the earlier recommendation was discarded.",
                        );
                    }
                };
                investigation.steps.push(InvestigationStep {
                    uncertainty: plan.uncertainty,
                    initial_hypothesis: plan.hypothesis,
                    discriminating_check: plan.expected_discriminator,
                    candidate_id: candidate.id.clone(),
                    path: candidate.path.clone(),
                    observation_ids: vec![observation.id],
                    counterevidence: revision.counterevidence,
                    previous_recommendation: revision.previous_preferred_approach,
                    revised_recommendation: revision.revised_preferred_approach,
                    recommendation_changed: revision.recommendation_changed,
                });
                draft = next_draft;
                brief = next_brief;
            }
        }
    }
    let catalog = DecisionCatalog::new(&retained, &observations)?;
    let mut verification_input =
        verification_request(&options.task, sources, &observations, &brief);
    verification_input["investigation_steps"] = serde_json::to_value(&investigation.steps)?;
    verification_input["investigation_remaining_uncertainty"] =
        serde_json::to_value(&investigation.remaining_uncertainty)?;
    let request = GenerationRequest {
        instructions: VERIFICATION_INSTRUCTIONS.into(),
        input: serde_json::to_string(&verification_input)?,
        schema: Some(verification_schema(&catalog)),
        reasoning_effort: config
            .config
            .models
            .reasoning
            .for_task("context_verification"),
    };
    let verification = match generate(
        model,
        config,
        request,
        &mut budget,
        &mut egress,
        !observations.is_empty(),
    )
    .await
    .and_then(|raw| {
        let verification = parse_validate_verification(&raw, &catalog, &brief)?;
        validate_trace_verification(&investigation, &verification)?;
        Ok(verification)
    }) {
        Ok(verification) => verification,
        Err(_) => {
            investigation.finish("support_check_failed", &budget);
            return fallback(
                conn,
                options,
                &selected,
                &retained,
                inspection,
                investigation,
                &budget,
                egress,
                "The decision did not pass its independent support and readiness check within the query budget; using deterministic guidance.",
            );
        }
    };
    // Recheck all consumed files after inference, including deeper-only files.
    // Incomplete inspection cannot certify freshness and disables cache reuse.
    let unchanged = inspection.observations.is_empty()
        || session
            .revalidate(&inspection.observations)
            .unwrap_or(false);
    if EvidenceCatalog::validate(conn, &selected).is_err() {
        return small_fallback(
            conn,
            options,
            &selected,
            &budget,
            &egress,
            "discarded",
            "evidence_changed",
            "Retained evidence changed during reasoning; refreshed deterministic context replaces the discarded decision.",
        );
    }
    if !unchanged || budget.ensure_time().is_err() {
        investigation.finish("evidence_changed", &budget);
        let mut current = inspection_report(&session, config);
        current.observations.clear();
        current.warnings.push("Inspected evidence changed or could not be revalidated within its I/O budget; generated interpretations were discarded.".into());
        return fallback(
            conn,
            options,
            &selected,
            &retained,
            current,
            investigation,
            &budget,
            egress,
            "Evidence changed during reasoning or could not be revalidated; generated advice was discarded.",
        );
    }
    inspection = inspection_report(&session, config);
    investigation
        .remaining_uncertainty
        .extend(brief.remaining_uncertainty.iter().map(|a| a.text.clone()));
    investigation.budget = budget.report();
    let cache_status = if !config.config.context.cache {
        "disabled"
    } else if !can_cache {
        "unavailable"
    } else {
        "miss"
    };
    let result = assemble(
        options,
        &selected,
        &retained,
        model,
        brief.clone(),
        inspection.clone(),
        investigation.clone(),
        &budget,
        egress.clone(),
        cache_status,
        None,
    );
    let Some(mut result) = fit(result)? else {
        return small_fallback(
            conn,
            options,
            &selected,
            &budget,
            &egress,
            &inspection.status,
            "output_budget",
            "The complete decision, required constraints, and exact observations do not fit this output budget; increase --max-tokens.",
        );
    };
    if can_cache {
        let mut entry = CachedDecision {
            version: CACHE_VERSION,
            question: options.task.clone(),
            created_at: util::now(),
            registry_revision,
            permission_key,
            key,
            revision_key: brief.revision_key,
            content_hash: String::new(),
            draft,
            verification,
            inspection,
            investigation,
        };
        entry.content_hash = cache_hash(&entry)?;
        let saved = (|| -> Result<()> {
            let bytes = serde_json::to_vec(&entry)?;
            ensure!(
                bytes.len() <= MAX_CACHE_BYTES,
                "decision cache exceeds its size bound"
            );
            util::private_dir(cache.parent().context("cache has no directory")?)?;
            context::memory::prepare_write(
                cache.parent().context("cache has no directory")?,
                &cache,
            )?;
            util::atomic_write(&cache, &bytes)
        })();
        if saved.is_err() {
            result.cache_status = "unavailable".into();
            result.warnings.push(
                "Local decision cache unavailable; this validated briefing remains usable.".into(),
            );
            if let Some(result) = fit(result)? {
                return Ok(DecisionContextResult::Brief(result));
            }
            return small_fallback(
                conn,
                options,
                &selected,
                &budget,
                &egress,
                "complete",
                "output_budget",
                "Cache metadata and complete guidance exceed the output budget.",
            );
        }
    }
    Ok(DecisionContextResult::Brief(result))
}

#[allow(clippy::too_many_arguments)]
fn assemble(
    options: &ContextOptions,
    selected: &ContextResult,
    retained: &EvidenceCatalog,
    model: &dyn GenerativeModel,
    brief: DecisionBrief,
    inspection: InspectionReport,
    investigation: InvestigationReport,
    budget: &Budget,
    egress: CheckoutEgress,
    cache_status: &str,
    fallback_reason: Option<String>,
) -> DecisionResult {
    DecisionResult {
        schema_version: DECISION_SCHEMA_VERSION,
        task: options.task.clone(),
        paths: options.paths.clone(),
        mode: if fallback_reason.is_some() {
            "fast_fallback"
        } else {
            "intelligent"
        }
        .into(),
        model_calls: budget.model_calls(),
        model: model.descriptor().model.clone(),
        cache_status: cache_status.into(),
        revision_key: brief.revision_key.clone(),
        budget: ContextBudget {
            max_tokens: options.max_tokens,
            used_tokens: 0,
            tokenizer: "cl100k_base".into(),
        },
        brief,
        evidence: retained.evidence.values().cloned().collect(),
        inspection,
        investigation,
        checkout_egress: egress,
        fallback_reason,
        retrieval_truncated: selected.retrieval_truncated,
        omissions: selected.omissions.clone(),
        brief_items_omitted: 0,
        warnings: selected.warnings.clone(),
    }
}

#[allow(clippy::too_many_arguments)]
fn fallback(
    conn: &Connection,
    options: &ContextOptions,
    selected: &ContextResult,
    retained: &EvidenceCatalog,
    inspection: InspectionReport,
    mut investigation: InvestigationReport,
    budget: &Budget,
    egress: CheckoutEgress,
    reason: &str,
) -> Result<DecisionContextResult> {
    // A failed assessment must not retain unverified generated claims or
    // dangling references from a previous investigation round.
    investigation.steps.clear();
    investigation.remaining_uncertainty.clear();
    investigation.budget = budget.report();
    let catalog = DecisionCatalog::new(retained, &inspection.observations)?;
    let brief = deterministic_fallback(&options.task, &catalog, reason);
    let result = DecisionResult {
        schema_version: DECISION_SCHEMA_VERSION,
        task: options.task.clone(),
        paths: options.paths.clone(),
        mode: "fast_fallback".into(),
        model_calls: budget.model_calls(),
        model: String::new(),
        cache_status: "unused".into(),
        revision_key: String::new(),
        budget: ContextBudget {
            max_tokens: options.max_tokens,
            used_tokens: 0,
            tokenizer: "cl100k_base".into(),
        },
        brief,
        evidence: retained.evidence.values().cloned().collect(),
        inspection: inspection.clone(),
        investigation: investigation.clone(),
        checkout_egress: egress.clone(),
        fallback_reason: Some(reason.into()),
        retrieval_truncated: selected.retrieval_truncated,
        omissions: selected.omissions.clone(),
        brief_items_omitted: 0,
        warnings: selected.warnings.clone(),
    };
    if let Some(result) = fit(result)? {
        return Ok(DecisionContextResult::Brief(result));
    }
    small_fallback(
        conn,
        options,
        selected,
        budget,
        &egress,
        &inspection.status,
        &investigation.stop_reason,
        reason,
    )
}

#[allow(clippy::too_many_arguments)]
fn small_fallback(
    conn: &Connection,
    options: &ContextOptions,
    selected: &ContextResult,
    budget: &Budget,
    egress: &CheckoutEgress,
    inspection: &str,
    investigation: &str,
    reason: &str,
) -> Result<DecisionContextResult> {
    let mut target = options.max_tokens;
    for _ in 0..12 {
        let mut context = context::build_context(
            conn,
            &ContextOptions {
                max_tokens: target,
                ..options.clone()
            },
        )?;
        context.schema_version = DECISION_SCHEMA_VERSION;
        context.model_calls = budget.model_calls();
        context.budget.max_tokens = options.max_tokens;
        for warning in &selected.warnings {
            if !context.warnings.contains(warning) {
                context.warnings.push(warning.clone());
            }
        }
        let mut result = DecisionContextResult::FastFallback(DecisionFallback {
            context,
            mode: "fast_fallback".into(),
            fallback_reason: reason.into(),
            cache_status: "unused".into(),
            inspection_status: inspection.into(),
            investigation_status: investigation.into(),
            checkout_egress: egress.clone(),
            model_call_limit: budget.report().max_model_calls,
        });
        measure(&mut result)?;
        let used = match &result {
            DecisionContextResult::FastFallback(r) => r.context.budget.used_tokens,
            _ => unreachable!(),
        };
        if used <= options.max_tokens {
            return Ok(result);
        }
        target = target.saturating_sub(used - options.max_tokens + 16);
        if target < context::MIN_MAX_TOKENS {
            break;
        }
    }
    Err(context::failure(
        "invalid_budget",
        "Task, paths and explicit schema 4 fallback metadata exceed --max-tokens; increase the budget.",
    ))
}

pub(crate) fn measure(result: &mut DecisionContextResult) -> Result<()> {
    let set = |r: &mut DecisionContextResult, n| match r {
        DecisionContextResult::Brief(r) => r.budget.used_tokens = n,
        DecisionContextResult::FastFallback(r) => r.context.budget.used_tokens = n,
    };
    set(result, 0);
    let mut previous = 0;
    for _ in 0..16 {
        let used = context::count_tokens(&(serde_json::to_string(result)? + "\n"))
            .max(context::count_tokens(&render(result)));
        if used <= previous {
            return Ok(());
        }
        set(result, used);
        previous = used;
    }
    Err(context::failure(
        "invalid_budget",
        "Could not stabilize schema 4 output budget metadata.",
    ))
}

fn fit(mut result: DecisionResult) -> Result<Option<DecisionResult>> {
    loop {
        let (mut evidence, mut observations) = referenced_ids(&result.brief);
        let (trace_evidence, trace_observations) = trace_references(&result.investigation)?;
        evidence.extend(trace_evidence);
        observations.extend(trace_observations);
        for step in &result.investigation.steps {
            observations.extend(step.observation_ids.iter().cloned());
            for item in &step.counterevidence {
                evidence.extend(item.evidence_ids.iter().cloned());
                observations.extend(item.observation_ids.iter().cloned());
            }
        }
        result.evidence.retain(|item| evidence.contains(&item.id));
        // Unreferenced inspections are useful to a caller if they fit. Remove
        // these whole observations before any core decision content.
        let mut measured = DecisionContextResult::Brief(result);
        measure(&mut measured)?;
        let DecisionContextResult::Brief(mut next) = measured else {
            unreachable!()
        };
        if next.budget.used_tokens <= next.budget.max_tokens {
            return Ok(Some(next));
        }
        if let Some(i) = next
            .inspection
            .observations
            .iter()
            .rposition(|o| !observations.contains(&o.id))
        {
            next.inspection.observations.remove(i);
        } else if let Some(i) = next
            .brief
            .checks
            .iter()
            .rposition(|c| c.priority == CheckPriority::OptionalFollowUp)
        {
            next.brief.checks.remove(i);
        } else if !next.brief.heuristics.is_empty() {
            next.brief.heuristics.pop();
        } else if let Some(i) = next.brief.facts.iter().rposition(|fact| {
            !next
                .brief
                .constraints
                .iter()
                .any(|c| c.knowledge_id == fact.record.record_id)
        }) {
            next.brief.facts.remove(i);
        } else if next.brief.completion_criteria.len() > 1 {
            next.brief.completion_criteria.pop();
        } else {
            return Ok(None);
        }
        next.brief_items_omitted += 1;
        result = next;
    }
}

pub fn render(result: &DecisionContextResult) -> String {
    match result {
        DecisionContextResult::FastFallback(result) => format!(
            "# Decision context\n\n{}\n\nInspection: {}. Investigation: {}.\n\n{}",
            display_text(&result.fallback_reason),
            display_text(&result.inspection_status),
            display_text(&result.investigation_status),
            context::render_context(&result.context)
        ),
        DecisionContextResult::Brief(result) => {
            let mut text = render_brief(&result.brief);
            if let Some(reason) = &result.fallback_reason {
                text.push_str(&format!("\nFallback: {}\n", display_text(reason)));
            }
            for observation in &result.inspection.observations {
                let fence = "`".repeat(
                    observation
                        .excerpt
                        .split(|ch| ch != '`')
                        .map(str::len)
                        .max()
                        .unwrap_or(0)
                        .saturating_add(1)
                        .max(3),
                );
                text.push_str(&format!(
                    "\n## Static observation {}\n\n{}:{}–{} ({})\n\n{fence}\n{}\n{fence}\n\n{}\n",
                    display_text(&observation.id),
                    display_text(&observation.path),
                    observation.start_line,
                    observation.end_line,
                    display_text(&observation.content_hash),
                    observation.excerpt.replace('\r', "\\r"),
                    display_text(&observation.qualification)
                ));
            }
            for step in &result.investigation.steps {
                text.push_str(&format!("\nInvestigation: {}\nHypothesis: {}\nInspection: {}\nPrevious action: {}\nRevised action: {}\n",
                    display_text(&step.uncertainty), display_text(&step.initial_hypothesis), display_text(&step.discriminating_check),
                    display_text(&step.previous_recommendation), display_text(&step.revised_recommendation)));
            }
            text.push_str(&format!("\nInspection: {}. Investigation: {}. Cache: {}. Model calls: {} / {}. Output: {} / {} tokens.\n",
                result.inspection.status, result.investigation.stop_reason, result.cache_status, result.model_calls,
                result.investigation.budget.max_model_calls, result.budget.used_tokens, result.budget.max_tokens));
            for warning in result.warnings.iter().chain(&result.inspection.warnings) {
                text.push_str(&format!("\n{}\n", display_text(warning)));
            }
            for evidence in &result.evidence {
                text.push_str(&format!(
                    "\n[{}] {} — {} ({}, current: {})\n",
                    display_text(&evidence.id),
                    display_text(&evidence.source),
                    display_text(&evidence.revision_id),
                    display_text(&evidence.basis),
                    evidence.current
                ));
            }
            text
        }
    }
}
