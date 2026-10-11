//! Read-only, receipt-bound proposals. Submitted captures never authenticate themselves.
use crate::{
    config::ResolvedConfig, context::adaptive::SnapshotManifest, engine, sources, storage, util,
};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::Write,
    path::{Component, Path},
};

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    contract: String,
    schema_version: u32,
    task_id: String,
    scope: Vec<String>,
    snapshot: SnapshotManifest,
    source_files: BTreeMap<String, String>,
    evidence_ids: Vec<String>,
    changes: Vec<Change>,
    checks: Vec<Check>,
    started_at: String,
    finished_at: Value,
    usage: Usage,
    notes: Vec<Note>,
    actor: Actor,
    grants: Grants,
    attestations: Vec<Attestation>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Change {
    path: String,
    before: Value,
    after: Value,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Check {
    event_id: String,
    checker_id: String,
    argv_digest: String,
    exit_status: Value,
    log_digest: Value,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Usage {
    elapsed_ms: Value,
    model_calls: Value,
    input_tokens: Value,
    output_tokens: Value,
    cost_usd: Value,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Note {
    kind: String,
    text: String,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Actor {
    kind: String,
    id: String,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Grants {
    inspected_files: Vec<String>,
    network: bool,
    execution: bool,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Attestation {
    kind: String,
    actor_id: String,
    capture_digest: String,
}

fn identifier(value: &str) -> Result<()> {
    ensure!(
        !value.is_empty()
            && value.len() <= 120
            && value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b)),
        "invalid opaque identifier"
    );
    Ok(())
}
fn digest(value: &str) -> Result<()> {
    let hex = value
        .strip_prefix("blake3:")
        .or_else(|| value.strip_prefix("sha256:"))
        .context("digest must name blake3 or sha256")?;
    ensure!(
        hex.len() == 64
            && hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "invalid content digest"
    );
    Ok(())
}
fn nullable_digest(value: &Value) -> Result<()> {
    if !value.is_null() {
        digest(
            value
                .as_str()
                .context("digest must be string or explicit null")?,
        )?;
    }
    Ok(())
}
fn relative(value: &str) -> Result<()> {
    ensure!(
        value
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != ".."),
        "ambiguous relative path"
    );
    ensure!(
        !value.is_empty()
            && value.len() <= 512
            && !value.contains(['\\', ':'])
            && !value.chars().any(char::is_control)
            && Path::new(value)
                .components()
                .all(|c| matches!(c, Component::Normal(_))),
        "invalid relative project path"
    );
    ensure!(
        !value.split('/').any(|part| part.starts_with('.')
            || matches!(
                part.to_ascii_lowercase().as_str(),
                "secrets" | "credentials" | "answer_keys" | "checker_private"
            )),
        "private or hidden paths cannot enter feedback"
    );
    Ok(())
}
fn unique<'a>(items: impl IntoIterator<Item = &'a str>) -> Result<()> {
    let mut seen = BTreeSet::new();
    for item in items {
        ensure!(seen.insert(item), "duplicate receipt entry");
    }
    Ok(())
}
fn byte_digest(path: &Path, expected: &str) -> Result<String> {
    use std::io::Read;
    util::reject_symlinks(path)?;
    ensure!(
        fs::symlink_metadata(path)?.is_file(),
        "expected a regular file"
    );
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // A concurrent replacement must not turn hashing into a blocking pipe read.
        options.custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW);
    }
    let file = options.open(path)?;
    ensure!(file.metadata()?.is_file(), "expected a regular file");
    let mut bytes = Vec::new();
    file.take(8_000_001).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 8_000_000, "file exceeds feedback limit");
    if expected.starts_with("sha256:") {
        use sha2::{Digest, Sha256};
        Ok(format!("sha256:{:x}", Sha256::digest(&bytes)))
    } else {
        Ok(util::digest(bytes))
    }
}

pub fn draft(
    config: &ResolvedConfig,
    receipt: &Path,
    allow_files: &[String],
    output: Option<&Path>,
    write: bool,
) -> Result<Value> {
    ensure!(
        write == output.is_some(),
        "--output and --write must be used together"
    );
    let receipt_path = util::absolute(&std::env::current_dir()?, receipt)?;
    ensure!(
        fs::symlink_metadata(&receipt_path)?.is_file(),
        "receipt must be a regular file"
    );
    let text = util::read_limited(&receipt_path, 512_000)?;
    let raw: Value = serde_json::from_str(&text)?;
    ensure!(
        raw["snapshot"].as_object().is_some_and(|o| o.len() == 2),
        "snapshot must contain only project and registry identity"
    );
    let receipt: Receipt = serde_json::from_value(raw)?;
    ensure!(
        receipt.contract == "lore.task_outcome" && receipt.schema_version == 1,
        "unsupported task outcome contract"
    );
    identifier(&receipt.task_id)?;
    identifier(&receipt.actor.id)?;
    ensure!(
        matches!(
            receipt.actor.kind.as_str(),
            "user" | "agent" | "external_runner"
        ),
        "invalid actor kind"
    );
    digest(&receipt.snapshot.registry_revision)?;
    ensure!(
        !receipt.scope.is_empty()
            && receipt.scope.len() <= 64
            && receipt.source_files.len() <= 1024
            && receipt.evidence_ids.len() <= 128
            && receipt.changes.len() <= 128
            && receipt.checks.len() <= 128
            && receipt.notes.len() <= 32
            && receipt.attestations.len() <= 32
            && receipt.grants.inspected_files.len() <= 128,
        "receipt collection limit exceeded"
    );
    for path in &receipt.scope {
        relative(path)?;
    }
    unique(receipt.scope.iter().map(String::as_str))?;
    for (path, hash) in &receipt.source_files {
        let (root, path) = path
            .split_once(':')
            .context("source path must be ROOT:PATH")?;
        identifier(root)?;
        relative(path)?;
        digest(hash)?;
    }
    unique(receipt.evidence_ids.iter().map(String::as_str))?;
    unique(receipt.changes.iter().map(|c| c.path.as_str()))?;
    unique(receipt.checks.iter().map(|c| c.event_id.as_str()))?;
    unique(receipt.grants.inspected_files.iter().map(String::as_str))?;
    for path in &receipt.grants.inspected_files {
        relative(path)?;
    }
    for change in &receipt.changes {
        relative(&change.path)?;
        nullable_digest(&change.before)?;
        nullable_digest(&change.after)?;
        ensure!(
            !change.before.is_null() || !change.after.is_null(),
            "change needs a before or after hash"
        );
    }
    for check in &receipt.checks {
        identifier(&check.event_id)?;
        identifier(&check.checker_id)?;
        digest(&check.argv_digest)?;
        nullable_digest(&check.log_digest)?;
        ensure!(
            check.exit_status.is_null()
                || check
                    .exit_status
                    .as_i64()
                    .is_some_and(|s| (-255..=255).contains(&s)),
            "exit status must be integer or explicit null"
        );
    }
    let start = chrono::DateTime::parse_from_rfc3339(&receipt.started_at)?;
    if !receipt.finished_at.is_null() {
        let end = chrono::DateTime::parse_from_rfc3339(
            receipt
                .finished_at
                .as_str()
                .context("finish time must be timestamp or null")?,
        )?;
        ensure!(end >= start, "finish precedes start");
    }
    for value in [
        &receipt.usage.elapsed_ms,
        &receipt.usage.model_calls,
        &receipt.usage.input_tokens,
        &receipt.usage.output_tokens,
    ] {
        ensure!(
            value.is_null() || value.as_u64().is_some(),
            "usage must be nonnegative integer or explicit null"
        );
    }
    ensure!(
        receipt.usage.cost_usd.is_null()
            || receipt
                .usage
                .cost_usd
                .as_f64()
                .is_some_and(|n| n.is_finite() && n >= 0.0),
        "cost must be nonnegative or explicit null"
    );
    for note in &receipt.notes {
        ensure!(
            matches!(note.kind.as_str(), "failure" | "repair" | "observation")
                && !note.text.trim().is_empty()
                && note.text.len() <= 4000,
            "invalid bounded observation note"
        );
    }
    for a in &receipt.attestations {
        ensure!(
            matches!(a.kind.as_str(), "external_check" | "independent_review"),
            "invalid attestation kind"
        );
        identifier(&a.actor_id)?;
        digest(&a.capture_digest)?;
    }
    unique(
        receipt
            .attestations
            .iter()
            .map(|a| a.capture_digest.as_str()),
    )?;
    unique(allow_files.iter().map(String::as_str))?;
    ensure!(allow_files.len() <= 128, "too many file grants");
    let mut verified_changes = Vec::new();
    for path in allow_files {
        relative(path)?;
        let change = receipt
            .changes
            .iter()
            .find(|c| &c.path == path)
            .context("file grant must name a changed file in this receipt")?;
        let target = util::absolute(&config.base, Path::new(path))?;
        ensure!(
            target.starts_with(&config.base)
                && !target.starts_with(&config.state)
                && !target.starts_with(&config.wiki),
            "file outside allowed source scope"
        );
        if change.after.is_null() {
            ensure!(
                !target.try_exists()?,
                "reported deletion does not match current file"
            );
        } else {
            let expected = change.after.as_str().unwrap();
            ensure!(
                byte_digest(&target, expected)? == expected,
                "changed file does not match reported after hash"
            );
        }
        verified_changes.push(json!({"path_digest":util::digest(path),"after":change.after,"verification":"current_bytes_only_not_check_execution"}));
    }
    let status = engine::status(config)?;
    ensure!(
        status.initialized,
        "task feedback requires an initialized project; no initialization was attempted"
    );
    let conn = storage::read_only(&config.state.join("state.db"))?;
    conn.execute_batch("BEGIN")?;
    let snapshot = SnapshotManifest::capture(config, &conn)?;
    ensure!(
        snapshot.project_id == receipt.snapshot.project_id,
        "receipt belongs to another project"
    );
    let sources = sources::scan(config)?;
    let current: BTreeMap<_, _> = sources
        .documents
        .iter()
        .map(|d| {
            (
                format!("{}:{}", d.root_id, d.relative_path),
                d.digest.clone(),
            )
        })
        .collect();
    let stale =
        status.needs_update || receipt.snapshot != snapshot || receipt.source_files != current;
    let active_evidence: BTreeSet<_> = storage::views(&conn)?
        .into_iter()
        .flat_map(|view| view.evidence.into_iter().filter(|e| e.active).map(|e| e.id))
        .collect();
    let mut evidence = Vec::new();
    for id in &receipt.evidence_ids {
        identifier(id)?;
        if id.starts_with("ne_") {
            let retained = crate::imports::evidence(&conn, id)?;
            evidence.push(json!({"id":id,"basis":"retained_native_report_not_current_implementation","content_digest":retained.content_hash,"active":retained.current}));
        } else {
            let retained = storage::evidence_snapshot(&conn, id)?;
            ensure!(
                util::digest(&retained.excerpt) == retained.digest,
                "evidence integrity failure"
            );
            evidence.push(json!({"id":id,"basis":retained.material.qualification(),"content_digest":retained.digest,"active":active_evidence.contains(id)}));
        }
    }
    let reported_pass = receipt
        .checks
        .iter()
        .filter(|c| c.exit_status == json!(0))
        .count();
    let reported_fail = receipt
        .checks
        .iter()
        .filter(|c| c.exit_status.as_i64().is_some_and(|s| s != 0))
        .count();
    let notes: Vec<_> = receipt
        .notes
        .iter()
        .map(|n| json!({"kind":n.kind,"text_digest":util::digest(&n.text)}))
        .collect();
    let identity = util::json_digest(&receipt)?;
    let state = if stale {
        "STALE: investigate against current sources; do not assert a current fact"
    } else {
        "Self-reported task outcome; check execution and independence are unverified"
    };
    // Receipt prose, command arguments, logs and checker captures stay private, including answer keys.
    let mut markdown = format!(
        "# Task outcome investigation proposal\n\n{state}.\n\nReceipt: `{identity}`. Registry: `{}`.\n\nReported checks: {reported_pass} passing, {reported_fail} failing, {} unknown. Recorded failure notes: {}; repair notes: {}. Unknown usage and cost remain unknown.\n\nREPORT feedback: A submitted task outcome reports {reported_fail} failed checks and {reported_pass} passing checks. Revalidate this scoped result before reusing the approach. This is a report, not proof of execution or an accepted project decision.\n\nSource evidence:\n",
        receipt.snapshot.registry_revision,
        receipt.checks.len() - reported_pass - reported_fail,
        receipt.notes.iter().filter(|n| n.kind == "failure").count(),
        receipt.notes.iter().filter(|n| n.kind == "repair").count()
    );
    for item in &evidence {
        markdown.push_str(&format!(
            "- `{}` (resolve with `lore evidence`).\n",
            item["id"].as_str().unwrap()
        ));
    }
    markdown.push_str("\nSuggested destination: a reviewed task investigation note in a configured Markdown source. Add only a disclosure-reviewed lesson; original free-form notes, paths, task text, checker identities and logs are intentionally omitted. Approve and import through ordinary lore update. Removing the source withdraws current support while retaining history.\n");
    conn.execute_batch("ROLLBACK")?;
    let final_conn = storage::read_only(&config.state.join("state.db"))?;
    ensure!(
        SnapshotManifest::capture(config, &final_conn)? == snapshot
            && sources::scan(config)?.digest == sources.digest,
        "project changed during feedback drafting; retry against a fresh snapshot"
    );
    let mut result = json!({"contract":"lore.feedback_draft","schema_version":1,"receipt_digest":identity,"snapshot":receipt.snapshot,"stale":stale,"verification":"self_reported","authenticated_external_results":0,"independently_reviewed_conclusions":0,"attestations_submitted":receipt.attestations.len(),"attestations_verified":false,"evidence":evidence,"notes":notes,"usage":receipt.usage,"reported_checks":{"passing":reported_pass,"failing":reported_fail,"unknown":receipt.checks.len()-reported_pass-reported_fail},"verified_changes":verified_changes,"unverified_changes":receipt.changes.len()-verified_changes.len(),"scope_digest":util::json_digest(&receipt.scope)?,"actor_kind":receipt.actor.kind,"actor_id_digest":util::digest(&receipt.actor.id),"actor_authenticated":false,"declared_grants_are_authority":false,"source_write":false,"model_calls":0,"network_calls":0,"markdown":markdown});
    if let Some(path) = output {
        let destination = util::absolute(&std::env::current_dir()?, path)?;
        ensure!(
            destination.extension().is_some_and(|e| e == "md")
                && !destination.starts_with(&config.state)
                && !destination.starts_with(&config.wiki)
                && destination != config.config_path,
            "output must be a new Markdown proposal outside generated/state paths"
        );
        let name = destination
            .file_name()
            .and_then(|s| s.to_str())
            .context("invalid destination")?;
        ensure!(
            !matches!(
                name.to_ascii_lowercase().as_str(),
                "agents.md" | "claude.md"
            ) && !name.starts_with('.'),
            "cannot write agent instructions or hidden files"
        );
        let mut opts = OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let parent = destination.parent().context("output has no parent")?;
        let temporary = parent.join(format!(".lore-feedback-{}", util::id("draft")));
        let publish = (|| -> Result<()> {
            let mut file = opts.open(&temporary)?;
            file.write_all(markdown.as_bytes())?;
            file.sync_all()?;
            drop(file);
            fs::hard_link(&temporary, &destination)
                .context("create new proposal; existing files are never replaced")?;
            Ok(())
        })();
        let _ = fs::remove_file(&temporary);
        publish?;
        result["source_write"] = json!(true);
        result["output"] = json!(destination);
    }
    Ok(result)
}
