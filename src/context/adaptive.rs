//! Explicitly versioned shared intelligence for people and coding agents.
//!
//! The established decision runtime owns retrieval, static observations,
//! hypothesis revision and support validation. This module adds caller-owned
//! standing grants, automatic initiative within those grants, a whole-registry
//! snapshot identity and complete-envelope budgeting. It never executes code.

use super::{
    ContextBudget, ContextOptions, ContextResult, count_tokens,
    decision::runtime::{
        self, CheckoutEgress, DecisionContextResult, DecisionFallback, RunOptions,
    },
    failure,
};
use crate::{config::ResolvedConfig, inference::GenerativeModel, storage, util};
use anyhow::Result;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const ADAPTIVE_SCHEMA_VERSION: u32 = 5;

/// A cancelled model future must not retain a SQLite read lock in an embedding
/// host. Releasing our savepoint also releases any unfinished nested read
/// savepoints, while preserving a transaction owned by the caller.
struct ReadSnapshot<'a> {
    conn: &'a Connection,
    active: bool,
}

impl<'a> ReadSnapshot<'a> {
    fn begin(conn: &'a Connection) -> Result<Self> {
        conn.execute_batch("SAVEPOINT lore_adaptive_context")?;
        Ok(Self { conn, active: true })
    }

    fn finish(mut self) -> Result<()> {
        self.conn.execute_batch("RELEASE lore_adaptive_context")?;
        self.active = false;
        Ok(())
    }
}

impl Drop for ReadSnapshot<'_> {
    fn drop(&mut self) {
        if self.active {
            let _ = self.conn.execute_batch("RELEASE lore_adaptive_context");
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SnapshotManifest {
    pub project_id: String,
    /// Identity of retained source heads, knowledge, imports and relationships.
    /// This describes the read transaction, not an uninspected live checkout.
    pub registry_revision: String,
}

impl SnapshotManifest {
    pub fn capture(config: &ResolvedConfig, conn: &Connection) -> Result<Self> {
        Ok(Self {
            project_id: config.project_id.clone(),
            registry_revision: storage::registry_revision(conn)?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Capabilities {
    /// `granted`, `not_granted`, `disabled_by_caller`, `outside_grant`, or `unavailable`.
    pub inspection: String,
    pub hosted_egress: bool,
    pub checkout_egress: bool,
    pub execution: bool,
    pub source_write: bool,
}

/// These grants come from the embedding host or the invoking process, never
/// from source text, a model response, or a repository's configuration file.
#[derive(Debug, Clone, Default)]
pub struct HostGrants {
    pub inspection_root: Option<PathBuf>,
    pub hosted_egress: bool,
    pub checkout_egress: bool,
}

impl HostGrants {
    pub fn from_environment() -> Self {
        Self {
            inspection_root: std::env::var_os("LORE_INSPECTION_ROOT")
                .filter(|value| !value.is_empty())
                .map(PathBuf::from),
            hosted_egress: std::env::var("LORE_ALLOW_HOSTED_EGRESS").as_deref() == Ok("1"),
            checkout_egress: std::env::var("LORE_ALLOW_CHECKOUT_EGRESS").as_deref() == Ok("1"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdaptiveResult {
    pub schema_version: u32,
    pub snapshot: SnapshotManifest,
    pub capabilities: Capabilities,
    /// The same evidence, observations, constraints and decision contract used
    /// by human presentations. Its nested schema remains explicitly versioned.
    pub intelligence: DecisionContextResult,
    pub budget: ContextBudget,
}

/// Apply a host grant as a ceiling. Explicit per-invocation flags are caller
/// grants; configuration can select a narrower root but cannot escape it.
pub fn authorize(
    config: &ResolvedConfig,
    run: &RunOptions,
    grants: &HostGrants,
) -> Result<(ResolvedConfig, RunOptions, Capabilities)> {
    let mut config = config.clone();
    let mut run = run.clone();
    let root_grant = grants
        .inspection_root
        .clone()
        .or_else(|| (run.inspect || run.investigate).then(|| config.base.clone()));
    let mut inspection = "not_granted";
    config.config.context.inspection.enabled = false;
    if run.no_inspect {
        inspection = "disabled_by_caller";
    } else if let Some(root) = root_grant {
        let paths = (|| -> Result<_> {
            let granted = util::absolute(&config.base, &root)?;
            let requested = config
                .config
                .context
                .inspection
                .root
                .as_ref()
                .map(|root| util::absolute(&config.base, root))
                .transpose()?
                .unwrap_or_else(|| granted.clone());
            Ok((granted, requested))
        })();
        match paths {
            Ok((granted, requested)) if requested.starts_with(&granted) => {
                if let Some(relative) = pathdiff::diff_paths(&requested, &config.base) {
                    config.config.context.inspection.root =
                        Some(if relative.as_os_str().is_empty() {
                            PathBuf::from(".")
                        } else {
                            relative
                        });
                    config.config.context.inspection.enabled = true;
                    inspection = "granted";
                } else {
                    inspection = "unavailable";
                }
            }
            Ok(_) => inspection = "outside_grant",
            Err(_) => inspection = "unavailable",
        }
    }
    // Automatic investigation is a choice of work inside a granted read scope.
    // In particular, a repository setting cannot grant a new checkout root.
    run.inspect = config.config.context.inspection.enabled;
    run.investigate = run.inspect;
    run.no_inspect = !run.inspect;
    let hosted_egress =
        !config.config.privacy.local_only && (grants.hosted_egress || run.allow_checkout_egress);
    let checkout_egress = hosted_egress
        && (run.allow_checkout_egress
            || grants.checkout_egress && config.config.privacy.allow_checkout_egress);
    config.config.privacy.local_only |= !hosted_egress;
    config.config.privacy.allow_checkout_egress = checkout_egress;
    run.allow_checkout_egress = checkout_egress;
    config.config.context.cache &= !run.no_cache;
    Ok((
        config,
        run,
        Capabilities {
            inspection: inspection.into(),
            hosted_egress,
            checkout_egress,
            execution: false,
            source_write: false,
        },
    ))
}

pub fn authorized_config(
    config: &ResolvedConfig,
    run: &RunOptions,
) -> Result<(ResolvedConfig, RunOptions, Capabilities)> {
    authorize(config, run, &HostGrants::from_environment())
}

fn inner_options(
    options: &ContextOptions,
    snapshot: &SnapshotManifest,
    capabilities: &Capabilities,
) -> Result<ContextOptions> {
    super::validate_options(options)?;
    let overhead = count_tokens(&serde_json::to_string(&(snapshot, capabilities))?) + 192;
    let mut inner = options.clone();
    inner.max_tokens = options.max_tokens.checked_sub(overhead)
        .filter(|remaining| *remaining >= super::MIN_MAX_TOKENS)
        .ok_or_else(|| failure("invalid_budget", "Adaptive context needs room for its snapshot, permission status and complete evidence; increase --max-tokens."))?;
    Ok(inner)
}

// Exact symbolic lookup is deliberately narrow. General "what/why/how" requests
// still use reasoning; a named constant can bypass it only when retained current
// evidence supplies that symbol and no selected disagreement needs investigation.
fn exact_symbol(task: &str) -> Option<&str> {
    let lower = task.to_lowercase();
    let prefix = [
        "what is the value of ",
        "what is ",
        "what's ",
        "show ",
        "value of ",
        "lookup ",
    ]
    .into_iter()
    .find(|prefix| lower.starts_with(prefix))?;
    let symbol = task[prefix.len()..]
        .trim()
        .trim_matches(|c: char| "?`'\"".contains(c));
    (symbol.contains('_')
        && symbol.len() > 2
        && symbol
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_'))
    .then_some(symbol)
}

fn exact_reference(
    options: &ContextOptions,
    selected: &ContextResult,
) -> Result<Option<DecisionContextResult>> {
    let Some(symbol) = exact_symbol(&options.task) else {
        return Ok(None);
    };
    if selected.retrieval_truncated
        || !selected.discrepancies.is_empty()
        || !selected.reviews.is_empty()
        || !selected.sections.needs_verification.is_empty()
        || selected
            .relations
            .iter()
            .any(|relation| relation.kind == "contradicts")
        || !selected.sections.items().any(|item| {
            item.statement.contains(symbol)
                && !item.evidence_ids.is_empty()
                && item.support_state == "current_documentary_support"
                && item.evidence_ids.iter().all(|id| {
                    selected
                        .evidence
                        .iter()
                        .any(|evidence| evidence.id == *id && evidence.current)
                })
        })
    {
        return Ok(None);
    }
    let mut context = selected.clone();
    context.schema_version = 4;
    context.model_calls = 0;
    context.budget.max_tokens = options.max_tokens;
    let mut result = DecisionContextResult::FastFallback(DecisionFallback {
        context,
        mode: "reference".into(),
        fallback_reason: "Exact retained symbolic reference; decision inference is unnecessary."
            .into(),
        cache_status: "unused".into(),
        inspection_status: "not_needed".into(),
        investigation_status: "decision_sufficient".into(),
        checkout_egress: CheckoutEgress {
            allowed: false,
            model_received_checkout: false,
            reason: "not_needed".into(),
        },
        model_call_limit: 0,
    });
    runtime::measure(&mut result)?;
    Ok(Some(result))
}

pub async fn run(
    config: &ResolvedConfig,
    conn: &Connection,
    options: &ContextOptions,
    run: &RunOptions,
) -> Result<AdaptiveResult> {
    let (config, run, capabilities) = authorized_config(config, run)?;
    let snapshot_guard = ReadSnapshot::begin(conn)?;
    let result = async {
        let snapshot = SnapshotManifest::capture(&config, conn)?;
        let inner = inner_options(options, &snapshot, &capabilities)?;
        if exact_symbol(&options.task).is_some() {
            let reference_options = ContextOptions {
                max_tokens: inner
                    .max_tokens
                    .saturating_sub(192)
                    .max(super::MIN_MAX_TOKENS),
                ..inner.clone()
            };
            let selected = super::build_context(conn, &reference_options)?;
            if let Some(reference) = exact_reference(&inner, &selected)? {
                return finish(options, snapshot, capabilities, reference);
            }
        }
        let intelligence = runtime::run(&config, conn, &inner, &run).await?;
        finish(options, snapshot, capabilities, intelligence)
    }
    .await?;
    snapshot_guard.finish()?;
    Ok(result)
}

/// Provider-neutral entry point for embedders and deterministic contract tests.
/// Preselection must be the unmodified result of `build_context` for this task,
/// paths and registry. Recreate it in the shared read snapshot before accepting
/// it; checking only cited records would miss newly added counterevidence.
/// The CLI's hybrid retrieval is selected separately within `run`'s snapshot.
pub async fn build(
    conn: &Connection,
    config: &ResolvedConfig,
    options: &ContextOptions,
    selected: ContextResult,
    model: Option<&dyn GenerativeModel>,
    run: &RunOptions,
) -> Result<AdaptiveResult> {
    let (config, run, capabilities) = authorized_config(config, run)?;
    let snapshot_guard = ReadSnapshot::begin(conn)?;
    let snapshot = SnapshotManifest::capture(&config, conn)?;
    let inner = inner_options(options, &snapshot, &capabilities)?;
    let selected_options = ContextOptions {
        max_tokens: selected.budget.max_tokens,
        ..options.clone()
    };
    let current = super::build_context(conn, &selected_options)?;
    if serde_json::to_value(&selected)? != serde_json::to_value(&current)? {
        return Err(failure(
            "invalid_selection",
            "Adaptive preselection no longer matches this registry, task, paths and input budget; rebuild context before requesting shared intelligence.",
        ));
    }
    if let Some(reference) = exact_reference(&inner, &selected)? {
        let result = finish(options, snapshot, capabilities, reference)?;
        snapshot_guard.finish()?;
        return Ok(result);
    }
    let intelligence =
        runtime::build_decision_context(conn, &config, &inner, selected, model, &run).await?;
    let result = finish(options, snapshot, capabilities, intelligence)?;
    snapshot_guard.finish()?;
    Ok(result)
}

fn finish(
    options: &ContextOptions,
    snapshot: SnapshotManifest,
    capabilities: Capabilities,
    intelligence: DecisionContextResult,
) -> Result<AdaptiveResult> {
    let mut result = AdaptiveResult {
        schema_version: ADAPTIVE_SCHEMA_VERSION,
        snapshot,
        capabilities,
        intelligence,
        budget: ContextBudget {
            max_tokens: options.max_tokens,
            used_tokens: 0,
            tokenizer: "cl100k_base".into(),
        },
    };
    for _ in 0..16 {
        let used = count_tokens(&(serde_json::to_string(&result)? + "\n"))
            .max(count_tokens(&render(&result)));
        // Decimal token counts can oscillate between adjacent values. Retain
        // a monotone upper bound for both complete output representations.
        if used <= result.budget.used_tokens {
            return if result.budget.used_tokens <= result.budget.max_tokens {
                Ok(result)
            } else {
                Err(failure(
                    "invalid_budget",
                    "The complete adaptive evidence package exceeds --max-tokens.",
                ))
            };
        }
        result.budget.used_tokens = used;
    }
    Err(failure(
        "invalid_budget",
        "Could not stabilize the complete adaptive output budget metadata.",
    ))
}

pub fn render(result: &AdaptiveResult) -> String {
    let mut text = runtime::render(&result.intelligence);
    text.push_str(&format!(
        "\nShared project snapshot: `{}`\n\nInspection: {}. Execution: unavailable.\n",
        result.snapshot.registry_revision, result.capabilities.inspection,
    ));
    text
}
