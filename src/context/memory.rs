//! Bounded housekeeping for the decision runtime's existing disposable findings.
//! No second knowledge store: the cached decision already contains hypotheses,
//! counterevidence, inspected observations, checks and the recommendation trace.

use crate::{config::ResolvedConfig, util};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

pub const MAX_FINDINGS: usize = 64;
pub const MAX_FINDING_BYTES: usize = 1_000_000;
pub const RETENTION_DAYS: u64 = 30;
const MAX_DIRECTORY_ENTRIES: usize = 512;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionReport {
    pub removed: usize,
    pub retained: usize,
    pub max_findings: usize,
    pub retention_days: u64,
}

fn finding_name(name: &str) -> bool {
    name.strip_prefix("decision-")
        .and_then(|v| v.strip_suffix(".json"))
        .is_some_and(|hash| hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// All policy-relevant source scopes and egress restrictions are inherited.
/// Query text and budgets are separately part of the runtime's full cache key.
pub fn permission_key(config: &ResolvedConfig) -> Result<String> {
    util::json_digest(&(
        "finding-permissions-v1",
        &config.project_id,
        &config.base,
        &config.roots,
        &config.imports,
        &config.config.sources,
        &config.config.privacy,
        config.config.context.inspection.enabled,
        &config.config.context.inspection.root,
    ))
}

fn files(directory: &Path) -> Result<Vec<(PathBuf, SystemTime, u64)>> {
    util::reject_symlinks(directory)?;
    if !directory.exists() {
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    for (index, entry) in fs::read_dir(directory)?.enumerate() {
        ensure!(
            index < MAX_DIRECTORY_ENTRIES,
            "finding directory exceeds its bounded inventory"
        );
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if !finding_name(name) {
            continue;
        }
        let metadata = entry.file_type()?;
        ensure!(
            !metadata.is_symlink(),
            "finding cache cannot contain symlinks"
        );
        if !metadata.is_file() {
            continue;
        }
        let metadata = entry.metadata()?;
        files.push((entry.path(), metadata.modified()?, metadata.len()));
    }
    Ok(files)
}

/// Evict expired/oversized findings and reserve one slot before an atomic write.
/// Call only when persistence is enabled: --no-cache never performs maintenance.
pub fn prepare_write(directory: &Path, destination: &Path) -> Result<RetentionReport> {
    let mut entries = files(directory)?;
    let now = SystemTime::now();
    let retention = Duration::from_secs(RETENTION_DAYS * 86_400);
    let mut removed = 0;
    let mut retained = Vec::new();
    for (path, modified, bytes) in entries.drain(..) {
        if bytes > MAX_FINDING_BYTES as u64
            || now.duration_since(modified).unwrap_or_default() > retention
        {
            fs::remove_file(&path).with_context(|| format!("evict finding {}", path.display()))?;
            removed += 1;
        } else if path != destination {
            retained.push((path, modified));
        }
    }
    retained.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
    let excess = retained.len().saturating_sub(MAX_FINDINGS - 1);
    for (path, _) in retained.iter().take(excess) {
        fs::remove_file(path)?;
        removed += 1;
    }
    Ok(RetentionReport {
        removed,
        retained: retained.len() - excess,
        max_findings: MAX_FINDINGS,
        retention_days: RETENTION_DAYS,
    })
}

pub fn fresh(created_at: &str) -> bool {
    chrono::DateTime::parse_from_rfc3339(created_at).is_ok_and(|created| {
        let age = chrono::Utc::now().signed_duration_since(created);
        age >= chrono::Duration::zero() && age <= chrono::Duration::days(RETENTION_DAYS as i64)
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FindingSummary {
    pub id: String,
    pub question: String,
    pub created_at: String,
    pub registry_revision: String,
    pub inspected_files: usize,
    pub investigation_steps: usize,
    /// Listing a finding does not rehash code or validate its recommendation.
    pub applicability: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryReport {
    pub schema_version: u32,
    pub findings: Vec<FindingSummary>,
    pub withheld: usize,
    pub max_findings: usize,
    pub retention_days: u64,
}

/// Metadata-only discovery of current-policy findings. A later task request
/// must still revalidate every dependency before using a recommendation.
pub fn list(config: &ResolvedConfig, registry_revision: &str) -> Result<MemoryReport> {
    let policy = permission_key(config)?;
    let mut result = MemoryReport {
        schema_version: 1,
        findings: Vec::new(),
        withheld: 0,
        max_findings: MAX_FINDINGS,
        retention_days: RETENTION_DAYS,
    };
    for (path, _, _) in files(&config.state.join("context-cache"))? {
        let record = util::read_limited(&path, MAX_FINDING_BYTES)
            .ok()
            .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok());
        let Some(record) = record.filter(|record| {
            record["version"] == 2
                && record["permission_key"] == policy
                && record["registry_revision"] == registry_revision
                && record["created_at"].as_str().is_some_and(fresh)
        }) else {
            result.withheld += 1;
            continue;
        };
        let Some(question) = record["question"]
            .as_str()
            .filter(|question| question.len() <= 16_384 && !question.chars().any(char::is_control))
        else {
            result.withheld += 1;
            continue;
        };
        result.findings.push(FindingSummary {
            id: path
                .file_stem()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .into(),
            question: question.into(),
            created_at: record["created_at"].as_str().unwrap_or_default().into(),
            registry_revision: registry_revision.into(),
            inspected_files: record["inspection"]["observations"]
                .as_array()
                .map_or(0, Vec::len),
            investigation_steps: record["investigation"]["steps"]
                .as_array()
                .map_or(0, Vec::len),
            applicability: "revalidation_required".into(),
        });
    }
    result.findings.sort_by(|a, b| {
        b.created_at
            .cmp(&a.created_at)
            .then_with(|| a.id.cmp(&b.id))
    });
    result.findings.truncate(MAX_FINDINGS);
    Ok(result)
}

pub fn clear(config: &ResolvedConfig) -> Result<usize> {
    let entries = files(&config.state.join("context-cache"))?;
    let count = entries.len();
    for (path, _, _) in entries {
        fs::remove_file(path)?;
    }
    Ok(count)
}
