//! Read-only adapters for explicitly supported upstream interchange formats.
//!
//! Native payloads are retained separately from Lore's interpretation. In
//! particular, a workflow status and an upstream verification event never grant
//! documentary authority to an issue or an agent recollection.
//!
//! Contracts checked against their upstream implementations:
//! - OpenWiki `src/claims/brains/code/{types,store}.ts`, `src/okf/frontmatter.ts`:
//!   <https://github.com/langchain-ai/openwiki/tree/0b1f07cc622156ac0efe8e28437029bc9427e826/src>
//! - Engram `internal/store/store.go` (`ExportData`, `BackupRelation`):
//!   <https://github.com/Gentleman-Programming/engram/blob/a36d4e150cc5d3a17877aedfd81ea5eaf5cc6f94/internal/store/store.go>
//! - Beads `cmd/bd/export.go`, `internal/types/types.go` (unversioned JSONL):
//!   <https://github.com/gastownhall/beads/blob/f21e3a80f7ed3b3b572b329e6f847971e78890ba/cmd/bd/export.go>

use crate::{config::ImportSource, domain::ImportKind, util};
use anyhow::{Context, Result, bail, ensure};
use pulldown_cmark::{Event, Options, Parser, Tag};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationKind {
    Implementation,
    Recollection,
    WorkState,
    Documentation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationVerification {
    Documentary,
    Reported,
    UpstreamVerifiedAtRevision,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservationScope {
    pub repository: Option<String>,
    pub component: Option<String>,
    pub environment: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeEvidence {
    pub locator: String,
    /// Opaque upstream token. It is not necessarily a Git revision.
    pub revision: Option<String>,
    /// RFC 6901 pointer into `native_record`, when the evidence is structured.
    pub field: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NativeRelationship {
    pub native_id: Option<String>,
    pub target_native_id: String,
    /// The upstream relation vocabulary, without silently changing direction.
    pub kind: String,
    pub upstream_status: Option<String>,
    /// False for pending, ignored, orphaned, or superseded upstream judgments.
    pub active: bool,
    pub native_record: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdapterRecord {
    pub native_id: String,
    pub kind: ObservationKind,
    pub title: String,
    pub subject: String,
    pub statement: String,
    pub scope: ObservationScope,
    /// Source-native lifecycle; "closed" continues to mean only closed work.
    pub lifecycle: String,
    pub evidence: Vec<NativeEvidence>,
    pub verification: ObservationVerification,
    pub observed_at: Option<String>,
    pub tags: Vec<String>,
    pub links: Vec<String>,
    pub relationships: Vec<NativeRelationship>,
    /// Exact parsed native record, including unrecognized extension fields.
    pub native_record: Value,
    /// Source-owned context outside the record, such as page verification.
    pub metadata: Value,
}

impl AdapterRecord {
    /// Construct a conservative record; adapters explicitly add qualifications.
    pub fn new(
        native_id: impl Into<String>,
        statement: impl Into<String>,
        native_record: Value,
    ) -> Self {
        let native_id = native_id.into();
        let statement = statement.into();
        Self {
            title: native_id.clone(),
            subject: native_id.clone(),
            native_id,
            kind: ObservationKind::Recollection,
            statement,
            scope: ObservationScope::default(),
            lifecycle: "reported".into(),
            evidence: Vec::new(),
            verification: ObservationVerification::Reported,
            observed_at: None,
            tags: Vec::new(),
            links: Vec::new(),
            relationships: Vec::new(),
            native_record,
            metadata: json!({}),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ImportBatch {
    /// Adapter contract, not a fabricated upstream application version.
    pub format: String,
    pub records: Vec<AdapterRecord>,
    pub warnings: Vec<String>,
}

pub fn read(source: &ImportSource, path: &Path, max_file_bytes: usize) -> Result<ImportBatch> {
    util::reject_symlinks(path)?;
    let mut batch = match source.kind {
        ImportKind::Openwiki => read_openwiki(source, path, max_file_bytes),
        ImportKind::Engram => read_engram(source, path, max_file_bytes),
        ImportKind::Beads => read_beads(source, path, max_file_bytes),
    }
    .with_context(|| format!("import {} from {}", source.id, path.display()))?;
    batch.records.sort_by(|a, b| a.native_id.cmp(&b.native_id));
    for pair in batch.records.windows(2) {
        ensure!(
            pair[0].native_id != pair[1].native_id,
            "duplicate native record id {} in import {}",
            pair[0].native_id,
            source.id
        );
    }
    batch.warnings.sort();
    batch.warnings.dedup();
    Ok(batch)
}

fn read_openwiki(source: &ImportSource, root: &Path, limit: usize) -> Result<ImportBatch> {
    ensure!(
        root.is_dir(),
        "OpenWiki root is missing or not a directory (not treated as deletion)"
    );
    let index = util::read_limited(&root.join("index.md"), limit)
        .context("OpenWiki native import requires the OKF root index.md")?;
    let (index_fields, _) = frontmatter(&index).context("read OpenWiki root index frontmatter")?;
    let version = string(&index_fields, "okf_version")?;
    ensure!(
        version == "0.2",
        "unsupported OpenWiki OKF version {version:?}; supported version: 0.2 (older Markdown wikis can be configured as source roots)"
    );
    let files = openwiki_files(root)?;
    let mut pages = BTreeMap::new();
    let mut sidecars = BTreeMap::new();
    for (relative, path) in files {
        if let Some(claim_relative) = relative.strip_prefix(".claims/") {
            if let Some(stem) = claim_relative.strip_suffix(".json") {
                let page = format!("{stem}.md");
                ensure!(
                    is_concept_page(&page),
                    "Claims sidecar targets a reserved page: {relative}"
                );
                sidecars.insert(page, (relative, path));
            }
        } else if is_concept_page(&relative) {
            pages.insert(relative, path);
        }
    }
    for page in sidecars.keys() {
        ensure!(
            pages.contains_key(page),
            "OpenWiki Claims sidecar has no corresponding Markdown page: {page} (not treated as deletion)"
        );
    }
    let mut batch = ImportBatch {
        format: "openwiki-okf/0.2+claims/1".into(),
        ..ImportBatch::default()
    };
    for (relative, path) in pages {
        let markdown = util::read_limited(&path, limit)?;
        let (fields, body) =
            frontmatter(&markdown).with_context(|| format!("parse OKF page {relative}"))?;
        validate_okf_page(&fields).with_context(|| format!("validate OKF page {relative}"))?;
        let title = optional_string(&fields, "title")?.unwrap_or(&relative);
        let subject = optional_string(&fields, "subject")?
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| relative.strip_suffix(".md").unwrap_or(&relative));
        let lifecycle = optional_string(&fields, "status")?.unwrap_or("stable");
        let scope_fields = fields
            .get("scope")
            .filter(|value| value.is_object())
            .unwrap_or(&fields);
        let scope = ObservationScope {
            repository: source
                .project
                .clone()
                .or(optional_string(scope_fields, "repository")?.map(str::to_owned)),
            component: optional_string(scope_fields, "component")?.map(str::to_owned),
            environment: optional_string(scope_fields, "environment")?.map(str::to_owned),
        };
        let tags = string_array(&fields, "tags")?;
        let mut links = markdown_links(body);
        for source in optional_array(&fields, "sources")? {
            links.push(nonempty_string(source, "resource")?.to_owned());
        }
        links.sort();
        links.dedup();
        if let Some((sidecar_relative, sidecar_path)) = sidecars.get(&relative) {
            let sidecar_text = util::read_limited(sidecar_path, limit)?;
            let sidecar: Value = serde_json::from_str(&sidecar_text)
                .with_context(|| format!("invalid OpenWiki Claims JSON: {sidecar_relative}"))?;
            check_keys(
                &sidecar,
                &["schemaVersion", "pageVersion", "claims", "verification"],
                "OpenWiki Claims sidecar",
            )?;
            ensure!(
                sidecar.get("schemaVersion").and_then(Value::as_u64) == Some(1),
                "unsupported OpenWiki Claims schemaVersion in {sidecar_relative}; supported version: 1"
            );
            let page_version = string(&sidecar, "pageVersion")?;
            ensure!(
                valid_sha256(page_version),
                "invalid OpenWiki pageVersion in {sidecar_relative}; expected sha256:<64 lowercase hexadecimal digits>"
            );
            // OpenWiki treats page drift as informational. Lore applies a
            // stricter snapshot boundary because page scope and lifecycle are
            // being joined to the claims: changed page metadata must not qualify
            // previously verified claims from another page revision.
            let observed_page_version = format!("sha256:{:x}", Sha256::digest(markdown.as_bytes()));
            ensure!(
                page_version == observed_page_version,
                "OpenWiki pageVersion mismatch for {relative}; regenerate a consistent page/Claims snapshot before importing (not treated as deletion)"
            );
            let verification = sidecar.get("verification");
            if let Some(event) = verification {
                check_keys(event, &["by", "at"], "OpenWiki Claims verification")?;
                identifier(event, "by")?;
                let at = identifier(event, "at")?;
                chrono::DateTime::parse_from_rfc3339(at).context("OpenWiki Claims verification.at must be an ISO 8601 timestamp with an explicit offset")?;
            }
            let claims = sidecar
                .get("claims")
                .and_then(Value::as_array)
                .context("OpenWiki Claims claims must be an array")?;
            if !claims.is_empty() && verification.is_none() {
                batch.warnings.push(format!("OpenWiki page {relative} has no durable Claims verification event; its implementation observations remain reported."));
            }
            for native in claims {
                check_keys(native, &["id", "statement", "evidence"], "OpenWiki claim")?;
                let id = identifier(native, "id")?;
                let statement = nonempty_string(native, "statement")?;
                ensure!(
                    statement == statement.trim(),
                    "OpenWiki claim {id} statement must be canonical (no surrounding whitespace)"
                );
                let evidence = native
                    .get("evidence")
                    .and_then(Value::as_array)
                    .filter(|e| !e.is_empty())
                    .context("OpenWiki claim requires at least one evidence record")?;
                let mut record = AdapterRecord::new(id, statement, native.clone());
                record.kind = ObservationKind::Implementation;
                record.title = title.into();
                record.subject = subject.into();
                record.scope = scope.clone();
                record.lifecycle = lifecycle.into();
                record.tags = tags.clone();
                record.links = links.clone();
                record.verification = if verification.is_some() {
                    ObservationVerification::UpstreamVerifiedAtRevision
                } else {
                    ObservationVerification::Reported
                };
                record.observed_at = verification
                    .and_then(|v| v.get("at"))
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                record.evidence.push(NativeEvidence {
                    locator: native_locator("openwiki", id),
                    revision: None,
                    field: Some("/statement".into()),
                });
                let mut resources = BTreeSet::new();
                for (index, item) in evidence.iter().enumerate() {
                    check_keys(item, &["resource", "version"], "OpenWiki claim evidence")?;
                    let locator = identifier(item, "resource")?;
                    let revision = identifier(item, "version")?;
                    ensure!(
                        resources.insert(locator),
                        "duplicate evidence resource on OpenWiki claim {id}"
                    );
                    record.evidence.push(NativeEvidence {
                        locator: locator.into(),
                        revision: Some(revision.into()),
                        field: Some(format!("/evidence/{index}")),
                    });
                }
                record.metadata = json!({
                    "claims_schema_version": 1,
                    "page": relative,
                    "page_version": page_version,
                    "page_metadata": fields,
                    "verification": verification,
                    "source_sidecar": sidecar_relative,
                    "current_checkout_verified": false,
                });
                batch.records.push(record);
            }
        } else if !body.trim().is_empty() {
            let id = format!("page:{relative}");
            let native = json!({ "path": relative, "markdown": markdown });
            let mut record = AdapterRecord::new(&id, body.trim(), native);
            record.kind = ObservationKind::Documentation;
            record.title = title.into();
            record.subject = subject.into();
            record.scope = scope;
            record.lifecycle = lifecycle.into();
            record.verification = ObservationVerification::Documentary;
            record.observed_at = fields
                .get("generated")
                .and_then(|v| v.get("at"))
                .and_then(Value::as_str)
                .map(str::to_owned);
            record.tags = tags;
            record.links = links;
            record.evidence.push(NativeEvidence {
                locator: native_locator("openwiki", &id),
                revision: None,
                field: Some("/markdown".into()),
            });
            record.metadata = json!({ "okf_version": "0.2", "page_metadata": fields, "page": relative, "current_checkout_verified": false });
            batch.records.push(record);
        }
    }
    Ok(batch)
}

fn frontmatter(markdown: &str) -> Result<(Value, &str)> {
    let mut lines = markdown.split_inclusive('\n');
    let opening = lines.next().context("empty OKF document")?;
    ensure!(
        opening.trim_end_matches(['\r', '\n']) == "---",
        "OKF document must begin with YAML frontmatter"
    );
    let mut offset = opening.len();
    let mut yaml = String::new();
    for line in lines {
        offset += line.len();
        if line.trim_end_matches(['\r', '\n']) == "---" {
            let fields: Value =
                serde_yaml::from_str(&yaml).context("invalid OKF YAML frontmatter")?;
            object(&fields, "OKF frontmatter")?;
            return Ok((fields, &markdown[offset..]));
        }
        yaml.push_str(line);
    }
    bail!("OKF document has no closing frontmatter delimiter")
}

fn validate_okf_page(fields: &Value) -> Result<()> {
    nonempty_string(fields, "type")?;
    for key in ["title", "description", "resource", "timestamp"] {
        if fields.get(key).is_some() {
            nonempty_string(fields, key)?;
        }
    }
    if let Some(status) = fields.get("status") {
        ensure!(
            matches!(status.as_str(), Some("draft" | "stable" | "deprecated")),
            "invalid OKF status; expected draft, stable, or deprecated"
        );
    }
    if let Some(generated) = fields.get("generated") {
        validate_actor_event(generated)?;
    }
    if let Some(verified) = fields.get("verified") {
        if let Some(events) = verified.as_array() {
            for event in events {
                validate_actor_event(event)?;
            }
        } else {
            validate_actor_event(verified)?;
        }
    }
    if let Some(deadline) = fields.get("stale_after") {
        let at = deadline
            .as_str()
            .context("OKF stale_after must be an ISO 8601 timestamp")?;
        chrono::DateTime::parse_from_rfc3339(at).context("invalid OKF stale_after timestamp")?;
    }
    Ok(())
}

fn validate_actor_event(event: &Value) -> Result<()> {
    object(event, "OKF actor event")?;
    nonempty_string(event, "by")?;
    if event.get("at").is_some() {
        chrono::DateTime::parse_from_rfc3339(string(event, "at")?)
            .context("invalid OKF actor event timestamp")?;
    }
    Ok(())
}

fn valid_sha256(version: &str) -> bool {
    version.strip_prefix("sha256:").is_some_and(|hash| {
        hash.len() == 64
            && hash
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    })
}

fn check_keys(value: &Value, allowed: &[&str], description: &str) -> Result<()> {
    let fields = object(value, description)?;
    for key in fields.keys() {
        ensure!(
            allowed.contains(&key.as_str()),
            "unsupported field {key:?} in {description}"
        );
    }
    Ok(())
}

fn is_concept_page(relative: &str) -> bool {
    let basename = relative
        .rsplit('/')
        .next()
        .unwrap_or(relative)
        .to_ascii_lowercase();
    relative.ends_with(".md")
        && !matches!(basename.as_str(), "index.md" | "log.md" | "instructions.md")
}

fn openwiki_files(root: &Path) -> Result<Vec<(String, PathBuf)>> {
    fn visit(root: &Path, directory: &Path, files: &mut Vec<(String, PathBuf)>) -> Result<()> {
        let mut entries = fs::read_dir(directory)
            .with_context(|| format!("read OpenWiki directory {}", directory.display()))?
            .collect::<std::io::Result<Vec<_>>>()?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_str().context("OpenWiki paths must be UTF-8")?;
            ensure!(
                !name.chars().any(char::is_control),
                "OpenWiki path contains control characters"
            );
            let metadata = fs::symlink_metadata(&path)?;
            ensure!(
                !metadata.file_type().is_symlink(),
                "symlink not allowed in OpenWiki import: {}",
                path.display()
            );
            if name.starts_with('.') && !(directory == root && name == ".claims") {
                continue;
            }
            if metadata.is_dir() {
                visit(root, &path, files)?;
            } else if metadata.is_file() {
                files.push((
                    path.strip_prefix(root)?
                        .to_str()
                        .context("OpenWiki paths must be UTF-8")?
                        .replace('\\', "/"),
                    path,
                ));
            }
        }
        Ok(())
    }
    let mut files = Vec::new();
    visit(root, root, &mut files)?;
    Ok(files)
}

fn read_engram(source: &ImportSource, path: &Path, limit: usize) -> Result<ImportBatch> {
    ensure!(
        path.is_file(),
        "Engram export is missing or not a regular file (not treated as deletion)"
    );
    let text = util::read_limited(path, limit)?;
    let export: Value = serde_json::from_str(&text).context("invalid Engram JSON export")?;
    object(&export, "Engram export")?;
    let version = string(&export, "version")?;
    ensure!(
        matches!(version, "0.1.0" | "0.2.0"),
        "unsupported Engram export version {version:?}; supported versions: 0.1.0, 0.2.0"
    );
    let observations = required_array(&export, "observations")?;
    let sessions = optional_array(&export, "sessions")?;
    let mut project_by_session = BTreeMap::new();
    for session in sessions {
        object(session, "Engram session")?;
        let id = identifier(session, "id")?;
        let project = optional_string(session, "project")?.filter(|p| !p.trim().is_empty());
        ensure!(
            project_by_session
                .insert(id.to_owned(), project.map(str::to_owned))
                .is_none(),
            "duplicate Engram session id {id}"
        );
    }
    let mut batch = ImportBatch {
        format: format!("engram-json/{version}"),
        ..ImportBatch::default()
    };
    let mut by_native_id = BTreeMap::new();
    for (index, native) in observations.iter().enumerate() {
        object(native, "Engram observation")?;
        let session_id = optional_string(native, "session_id")?;
        let own_project = optional_string(native, "project")?.filter(|s| !s.trim().is_empty());
        let project = own_project.or_else(|| {
            session_id
                .and_then(|id| project_by_session.get(id))
                .and_then(|p| p.as_deref())
        });
        if source
            .project
            .as_deref()
            .is_some_and(|filter| project != Some(filter))
        {
            continue;
        }
        let native_id =
            if let Some(sync_id) = optional_string(native, "sync_id")?.filter(|s| !s.is_empty()) {
                valid_id(sync_id)?;
                sync_id.to_owned()
            } else {
                let id = native
                    .get("id")
                    .and_then(Value::as_i64)
                    .filter(|id| *id > 0)
                    .context("Engram observations without sync_id require a positive integer id")?;
                format!("observation:{id}")
            };
        let title = nonempty_string(native, "title")?;
        let content = nonempty_string(native, "content")?;
        let mut record = AdapterRecord::new(&native_id, content, native.clone());
        record.kind = ObservationKind::Recollection;
        record.title = title.to_owned();
        record.subject = optional_string(native, "topic_key")?
            .filter(|s| !s.trim().is_empty())
            .unwrap_or(title)
            .to_owned();
        record.scope.repository = project.map(str::to_owned);
        record.lifecycle = if optional_string(native, "deleted_at")?.is_some_and(|s| !s.is_empty())
        {
            "deleted"
        } else {
            "active"
        }
        .into();
        record.observed_at = optional_string(native, "updated_at")?
            .or(optional_string(native, "created_at")?)
            .map(str::to_owned);
        record.evidence.push(NativeEvidence {
            locator: native_locator("engram", &native_id),
            revision: None,
            field: Some("/content".into()),
        });
        if let Some(kind) = optional_string(native, "type")?.filter(|s| !s.trim().is_empty()) {
            record.tags.push(kind.to_owned());
        }
        if let Some(key) = optional_string(native, "topic_key")?.filter(|s| !s.trim().is_empty()) {
            record.tags.push(key.to_owned());
        }
        record.links = markdown_links(content);
        record.metadata = json!({
            "export_version": version,
            "session_id": session_id,
            "effective_project": project,
            "source_scope": optional_string(native, "scope")?,
            "review_after": native.get("review_after"),
        });
        ensure!(
            by_native_id
                .insert(native_id.clone(), batch.records.len())
                .is_none(),
            "duplicate Engram native id {native_id} at observation {index}"
        );
        batch.records.push(record);
    }
    let mut relation_ids = BTreeSet::new();
    for relation in optional_array(&export, "relations")? {
        object(relation, "Engram relation")?;
        let source_id = identifier(relation, "source_id")?;
        let target_id = identifier(relation, "target_id")?;
        let Some(&source_index) = by_native_id.get(source_id) else {
            continue;
        };
        // Engram's project export closes relations over both endpoints. Apply
        // the same rule when filtering an unscoped export locally.
        if source.project.is_some() && !by_native_id.contains_key(target_id) {
            continue;
        }
        let native_id = identifier(relation, "sync_id")?;
        ensure!(
            relation_ids.insert(native_id.to_owned()),
            "duplicate Engram relation id {native_id}"
        );
        let kind = nonempty_string(relation, "relation")?;
        let status = nonempty_string(relation, "judgment_status")?;
        let active = status == "judged"
            && kind != "pending"
            && optional_string(relation, "superseded_at")?.is_none_or(str::is_empty);
        batch.records[source_index]
            .relationships
            .push(NativeRelationship {
                native_id: Some(native_id.into()),
                target_native_id: target_id.into(),
                kind: kind.into(),
                upstream_status: Some(status.into()),
                active,
                native_record: relation.clone(),
            });
    }
    for record in &mut batch.records {
        record.relationships.sort_by(|a, b| {
            (&a.native_id, &a.target_native_id, &a.kind).cmp(&(
                &b.native_id,
                &b.target_native_id,
                &b.kind,
            ))
        });
    }
    Ok(batch)
}

fn read_beads(source: &ImportSource, path: &Path, limit: usize) -> Result<ImportBatch> {
    ensure!(
        path.is_file(),
        "Beads export is missing or not a regular file (not treated as deletion)"
    );
    let text = util::read_limited(path, limit)?;
    let mut batch = ImportBatch {
        format: "beads-issue-jsonl".into(),
        ..ImportBatch::default()
    };
    let mut skipped_memories = 0usize;
    for (line_index, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let native: Value = serde_json::from_str(line)
            .with_context(|| format!("invalid Beads JSONL at line {}", line_index + 1))?;
        object(&native, "Beads record")?;
        // `bd export` has no version header. Reject a differently versioned
        // format instead of pretending an arbitrary envelope is this contract.
        ensure!(
            native.get("schema_version").is_none() && native.get("schemaVersion").is_none(),
            "unsupported versioned Beads record at line {}; expected native bd export JSONL",
            line_index + 1
        );
        let record_type = optional_string(&native, "_type")?.unwrap_or("issue");
        match record_type {
            "memory" => {
                if !source.include_memories {
                    skipped_memories += 1;
                    continue;
                }
                let key = identifier(&native, "key")?;
                let content = nonempty_string(&native, "value")?;
                let id = format!("memory:{key}");
                let mut record = AdapterRecord::new(&id, content, native.clone());
                record.title = key.into();
                record.subject = key.into();
                record.scope.repository = source.project.clone();
                record.evidence.push(NativeEvidence {
                    locator: native_locator("beads", &id),
                    revision: None,
                    field: Some("/value".into()),
                });
                record.links = markdown_links(content);
                batch.records.push(record);
            }
            "issue" => {
                // Some legacy exports represent persistent context as a custom
                // memory issue type. Keep the same explicit opt-in boundary.
                if optional_string(&native, "issue_type")? == Some("memory")
                    && !source.include_memories
                {
                    skipped_memories += 1;
                    continue;
                }
                let id = identifier(&native, "id")?;
                let title = nonempty_string(&native, "title")?;
                let mut parts = vec![title.to_owned()];
                let status = optional_string(&native, "status")?
                    .filter(|s| !s.is_empty())
                    .unwrap_or("unknown");
                parts.push(format!("Work status: {status}"));
                for (field, label) in [
                    ("description", "Description"),
                    ("design", "Design"),
                    ("acceptance_criteria", "Acceptance criteria"),
                    ("notes", "Notes"),
                    ("close_reason", "Close reason"),
                ] {
                    if let Some(value) =
                        optional_string(&native, field)?.filter(|s| !s.trim().is_empty())
                    {
                        parts.push(format!("{label}: {value}"));
                    }
                }
                for comment in optional_array(&native, "comments")? {
                    object(comment, "Beads comment")?;
                    if let Some(text) =
                        optional_string(comment, "text")?.filter(|s| !s.trim().is_empty())
                    {
                        parts.push(format!("Comment: {text}"));
                    }
                }
                let mut record = AdapterRecord::new(id, parts.join("\n\n"), native.clone());
                record.kind = ObservationKind::WorkState;
                record.title = title.into();
                record.subject = id.into();
                // Project is an importer-owned namespace here: Beads does not
                // serialize its internal SourceRepo field in issue exports.
                record.scope.repository = source.project.clone();
                if let Some(metadata) = native.get("metadata").filter(|v| v.is_object()) {
                    record.scope.component =
                        optional_string(metadata, "component")?.map(str::to_owned);
                    record.scope.environment =
                        optional_string(metadata, "environment")?.map(str::to_owned);
                    if let Some(subject) =
                        optional_string(metadata, "subject")?.filter(|s| !s.trim().is_empty())
                    {
                        record.subject = subject.into();
                    }
                }
                record.lifecycle = status.into();
                record.observed_at = optional_string(&native, "updated_at")?
                    .or(optional_string(&native, "created_at")?)
                    .map(str::to_owned);
                record.evidence.push(NativeEvidence {
                    locator: native_locator("beads", id),
                    revision: None,
                    field: Some(String::new()),
                });
                record.tags = string_array(&native, "labels")?;
                if let Some(issue_type) =
                    optional_string(&native, "issue_type")?.filter(|s| !s.is_empty())
                {
                    record.tags.push(issue_type.into());
                }
                record.links = markdown_links(&record.statement);
                if let Some(external) =
                    optional_string(&native, "external_ref")?.filter(|s| !s.is_empty())
                {
                    record.links.push(external.into());
                }
                for dependency in optional_array(&native, "dependencies")? {
                    object(dependency, "Beads dependency")?;
                    if let Some(owner) = optional_string(dependency, "issue_id")? {
                        ensure!(
                            owner == id,
                            "Beads dependency on {id} declares a different issue_id"
                        );
                    }
                    let target = identifier(dependency, "depends_on_id")?;
                    let kind = nonempty_string(dependency, "type")?;
                    record.relationships.push(NativeRelationship {
                        native_id: optional_string(dependency, "id")?.map(str::to_owned),
                        target_native_id: target.into(),
                        kind: kind.into(),
                        upstream_status: None,
                        active: true,
                        native_record: dependency.clone(),
                    });
                }
                record.tags.sort();
                record.tags.dedup();
                record.links.sort();
                record.links.dedup();
                batch.records.push(record);
            }
            other => bail!(
                "unsupported Beads record type {other:?} at line {}",
                line_index + 1
            ),
        }
    }
    if skipped_memories > 0 {
        batch.warnings.push(format!(
            "Excluded {skipped_memories} Beads memory records; include_memories defaults to false."
        ));
    }
    Ok(batch)
}

fn object<'a>(value: &'a Value, description: &str) -> Result<&'a serde_json::Map<String, Value>> {
    value
        .as_object()
        .with_context(|| format!("{description} must be a JSON object"))
}

fn string<'a>(value: &'a Value, field: &str) -> Result<&'a str> {
    value
        .get(field)
        .and_then(Value::as_str)
        .with_context(|| format!("field {field:?} must be a string"))
}

fn nonempty_string<'a>(value: &'a Value, field: &str) -> Result<&'a str> {
    let s = string(value, field)?;
    ensure!(!s.trim().is_empty(), "field {field:?} must not be empty");
    Ok(s)
}

fn optional_string<'a>(value: &'a Value, field: &str) -> Result<Option<&'a str>> {
    match value.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s)),
        _ => bail!("field {field:?} must be a string or null"),
    }
}

fn identifier<'a>(value: &'a Value, field: &str) -> Result<&'a str> {
    let id = string(value, field)?;
    valid_id(id)?;
    Ok(id)
}

fn valid_id(id: &str) -> Result<()> {
    ensure!(
        !id.is_empty() && id == id.trim() && !id.chars().any(char::is_control),
        "native identity must be a nonempty canonical string without control characters"
    );
    Ok(())
}

fn required_array<'a>(value: &'a Value, field: &str) -> Result<&'a [Value]> {
    ensure!(
        value.get(field).is_some(),
        "required array {field:?} is missing"
    );
    optional_array(value, field)
}

fn optional_array<'a>(value: &'a Value, field: &str) -> Result<&'a [Value]> {
    match value.get(field) {
        Some(Value::Array(values)) => Ok(values),
        None | Some(Value::Null) => Ok(&[]),
        _ => bail!("field {field:?} must be an array or null"),
    }
}

fn string_array(value: &Value, field: &str) -> Result<Vec<String>> {
    optional_array(value, field)?
        .iter()
        .map(|v| {
            let s = v
                .as_str()
                .with_context(|| format!("{field} entries must be strings"))?;
            ensure!(!s.trim().is_empty(), "{field} entries must not be empty");
            Ok(s.to_owned())
        })
        .collect()
}

fn native_locator(origin: &str, id: &str) -> String {
    format!(
        "{origin}://{}",
        percent_encoding::utf8_percent_encode(id, percent_encoding::NON_ALPHANUMERIC)
    )
}

fn markdown_links(text: &str) -> Vec<String> {
    let mut links: Vec<String> = Parser::new_ext(text, Options::all())
        .filter_map(|event| match event {
            Event::Start(Tag::Link { dest_url, .. }) if !dest_url.is_empty() => {
                Some(dest_url.into_string())
            }
            _ => None,
        })
        .collect();
    links.sort();
    links.dedup();
    links
}
