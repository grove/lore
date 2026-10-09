//! Versioned native observations. Upstream systems retain ownership of their
//! records; Lore stores immutable evidence and its separately qualified links.
pub mod adapters;
pub mod relationships;
pub mod render;
pub mod storage;

use crate::{config::ResolvedConfig, domain::ImportKind, util};
use adapters::{AdapterRecord, ImportBatch};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub use storage::{evidence, views, warnings};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportedObservation {
    /// Stable identity within one configured native source, across revisions.
    pub id: String,
    pub evidence_id: String,
    pub snapshot_id: String,
    pub import_id: String,
    pub origin: ImportKind,
    pub source_path: String,
    pub format: String,
    /// BLAKE3 of canonical parsed native JSON, not a fabricated Markdown quote.
    pub content_hash: String,
    pub captured_at: String,
    /// Current in the last successfully imported snapshot. Does not assert
    /// that code, a remote service, or an upstream database is current.
    pub current: bool,
    pub record: AdapterRecord,
}

#[derive(Debug, Clone)]
pub struct SourceBatch {
    pub id: String,
    pub kind: ImportKind,
    pub path: PathBuf,
    pub batch: ImportBatch,
    pub digest: String,
}

#[derive(Debug, Clone, Default)]
pub struct Inventory {
    pub sources: Vec<SourceBatch>,
    pub digest: String,
    pub warnings: Vec<String>,
}
impl Inventory {
    pub fn records(&self) -> usize {
        self.sources.iter().map(|s| s.batch.records.len()).sum()
    }
}

pub fn scan(config: &ResolvedConfig) -> Result<Inventory> {
    let mut sources = Vec::new();
    let mut warnings = Vec::new();
    for (id, path) in &config.imports {
        let source = config
            .config
            .imports
            .iter()
            .find(|s| &s.id == id)
            .context("resolved native import has no configuration")?;
        let batch = adapters::read(source, path, config.config.processing.max_file_bytes)?;
        for record in &batch.records {
            validate_record(record).with_context(|| {
                format!("invalid native observation {}:{}", id, record.native_id)
            })?;
        }
        let digest = util::json_digest(&("native-import-v1", source, path, &batch))?;
        warnings.extend(batch.warnings.iter().map(|w| format!("Import {id}: {w}")));
        sources.push(SourceBatch {
            id: id.clone(),
            kind: source.kind,
            path: path.clone(),
            batch,
            digest,
        });
    }
    sources.sort_by(|a, b| a.id.cmp(&b.id));
    let digest = util::json_digest(
        &sources
            .iter()
            .map(|s| (&s.id, &s.digest))
            .collect::<Vec<_>>(),
    )?;
    Ok(Inventory {
        sources,
        digest,
        warnings,
    })
}

/// Structured evidence has its own validator: JSON pointers resolve against
/// the retained native record, while opaque code references are merely retained.
pub fn validate_record(record: &AdapterRecord) -> Result<()> {
    ensure!(
        !record.native_id.trim().is_empty()
            && record.native_id.len() <= 2_048
            && !record.native_id.chars().any(char::is_control),
        "invalid native identity"
    );
    ensure!(
        !record.statement.trim().is_empty(),
        "empty native observation"
    );
    ensure!(
        !record.evidence.is_empty(),
        "native observation lacks an evidence locator"
    );
    for evidence in &record.evidence {
        ensure!(
            !evidence.locator.trim().is_empty()
                && evidence.locator.len() <= 8_192
                && !evidence.locator.chars().any(char::is_control),
            "invalid native evidence locator"
        );
        if let Some(pointer) = &evidence.field {
            ensure!(
                record.native_record.pointer(pointer).is_some(),
                "unresolvable native evidence JSON pointer: {pointer}"
            );
        }
    }
    if record.verification == adapters::ObservationVerification::UpstreamVerifiedAtRevision {
        ensure!(
            record
                .evidence
                .iter()
                .any(|e| e.revision.as_ref().is_some_and(|r| !r.trim().is_empty())),
            "upstream verification lacks an evidence revision"
        );
    }
    Ok(())
}
