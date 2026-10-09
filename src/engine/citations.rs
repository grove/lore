//! Request-scoped citation checks shared by overview and topic synthesis.
//! Diagnostics retain locations and hashes, never rejected prose or raw IDs.
use crate::{config::ResolvedConfig, domain::PageDraft, util};
use anyhow::{Result, bail};
use serde_json::json;
use std::collections::BTreeSet;

pub(super) const CONTRACT_VERSION: &str = "scoped-page-citations-v1";

pub(super) fn validate(
    draft: &PageDraft,
    allowed: &BTreeSet<String>,
    task: &str,
    config: &ResolvedConfig,
    run: &str,
) -> Result<()> {
    let mut issues = Vec::new();
    let mut invalid_count = 0;
    for (section, s) in draft.sections.iter().enumerate() {
        for (paragraph, p) in s.paragraphs.iter().enumerate() {
            let mut seen = BTreeSet::new();
            for (citation, id) in p.knowledge_ids.iter().enumerate() {
                let reason = if !allowed.contains(id) {
                    Some("outside_request_scope")
                } else if !seen.insert(id) {
                    Some("duplicate_in_paragraph")
                } else {
                    None
                };
                if let Some(reason) = reason {
                    invalid_count += 1;
                    if issues.len() < 128 {
                        issues.push(json!({
                            "section": section + 1, "paragraph": paragraph + 1,
                            "citation": citation + 1, "reason": reason,
                            "id_digest": util::digest(id.as_bytes()), "id_bytes": id.len()
                        }));
                    }
                }
            }
        }
    }
    if invalid_count == 0 {
        return Ok(());
    }
    let record = json!({
        "schema_version": 1, "event": "rejected_citations",
        "contract": CONTRACT_VERSION, "task": task, "run": run,
        "allowed_knowledge_ids": allowed, "invalid_count": invalid_count,
        "issues": issues, "truncated": invalid_count > issues.len(),
        "draft_digest": util::json_digest(draft)?,
        "note": "A rejected attempt, not a run outcome. Repair may subsequently succeed. No raw rejected IDs or prose retained."
    });
    let directory = config.state.join("citation-diagnostics");
    util::private_dir(&directory)?;
    let digest = util::json_digest(&record)?;
    let file = directory.join(format!("{}.json", &digest[7..]));
    util::atomic_write(&file, &serde_json::to_vec_pretty(&record)?)?;
    let first = &issues[0];
    bail!(
        "{task} cited unknown or duplicate knowledge at section {}, paragraph {}, citation {}. Copy knowledge_ids exactly from knowledge[].id using the schema enum, not evidence/assertion IDs or document names. Safe rejection metadata: {}",
        first["section"],
        first["paragraph"],
        first["citation"],
        file.display()
    )
}
