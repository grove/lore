//! Ephemeral, model-free first contact with a project's local Markdown.
//!
//! This is source navigation, not a second registry or a knowledge extraction
//! engine. Source text is always data; it cannot grant inspection or egress.
use crate::{
    config::{Config, ResolvedConfig},
    context::{
        self, ContextBudget, ContextError, ContextOptions,
        inspection::{identity, revision, safe_fs::Directory},
    },
    sources, util,
};
use anyhow::{Context, Result};
use globset::{Glob, GlobSet, GlobSetBuilder};
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{ErrorKind, Read},
    ops::Range,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

pub const MAX_FILES: usize = 256;
pub const MAX_TOTAL_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_FILE_BYTES: usize = 1024 * 1024;
pub const MAX_DEPTH: usize = 8;
const MAX_ENTRIES: usize = 16_384;
const BASIS: &str = "documentary_excerpt_not_runtime_verification";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    pub path: String,
    pub file_sha256: String,
    pub line_start: usize,
    pub line_end: usize,
    pub excerpt: String,
    pub basis: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Discovery {
    /// Includes local ignore/configuration files in the same read limits.
    pub files_read: usize,
    pub source_files_read: usize,
    pub bytes_read: usize,
    pub entries_seen: usize,
    pub skipped_files: usize,
    pub truncated: bool,
    pub limits_reached: BTreeSet<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootstrapResult {
    pub contract: String,
    pub schema_version: u32,
    pub mode: String,
    pub task: String,
    pub project_root: String,
    /// Digest of selected path/full-file-SHA256 identities, not a registry revision.
    pub source_snapshot_sha256: String,
    pub evidence: Vec<Evidence>,
    pub best_next_action: String,
    pub limitations: Vec<String>,
    /// Number of known whole source groups excluded from the response.
    /// Unenumerated files under discovery caps are separately reported below.
    pub omitted_source_groups: usize,
    pub discovery: Discovery,
    pub model_calls: u32,
    pub source_write: bool,
    pub budget: ContextBudget,
}

pub fn error(code: &'static str, message: impl Into<String>) -> anyhow::Error {
    ContextError {
        code,
        message: message.into(),
    }
    .into()
}

/// Configuration bytes and their selected filesystem identity stay bound from
/// first-contact dispatch through the last source-only output validation.
pub struct ConfigurationSnapshot {
    resolved: ResolvedConfig,
    root_identity: String,
    file_revision: String,
    bytes_read: usize,
}

impl ConfigurationSnapshot {
    pub fn load(path: &Path) -> Result<Self> {
        (|| {
            let absolute = util::absolute(&std::env::current_dir()?, path)?;
            let parent = absolute.parent().context("configuration has no parent")?;
            let directory = Directory::open(parent)?;
            Self::read_from(&directory, &absolute)
        })()
        .map_err(|e: anyhow::Error| {
            if e.is::<ContextError>() {
                e
            } else {
                error(
                    "configuration_error",
                    format!("Cannot load selected configuration safely: {e:#}"),
                )
            }
        })
    }

    pub fn resolved(&self) -> &ResolvedConfig {
        &self.resolved
    }

    fn read_from(directory: &Directory, absolute: &Path) -> Result<Self> {
        let name = Path::new(
            absolute
                .file_name()
                .context("configuration has no filename")?,
        );
        let root_identity = identity(&directory.metadata()?);
        let mut file = directory.file(name)?;
        let before = file.metadata()?;
        if before.len() > 128_000 {
            return Err(error(
                "configuration_error",
                "Configuration exceeds 128000 bytes.",
            ));
        }
        let mut bytes = Vec::with_capacity(before.len() as usize);
        (&mut file).take(before.len()).read_to_end(&mut bytes)?;
        validate_read(
            &before,
            &file.metadata()?,
            &directory.file(name)?.metadata()?,
            bytes.len(),
        )?;
        let file_revision = revision(&before);
        validate_configuration(absolute, &root_identity, &file_revision)?;
        let config: Config = serde_yaml::from_slice(&bytes).context("invalid lore.yml")?;
        let resolved = ResolvedConfig::resolve_read_snapshot(config, absolute)?;
        let snapshot = Self {
            resolved,
            root_identity,
            file_revision,
            bytes_read: bytes.len(),
        };
        snapshot.revalidate()?;
        Ok(snapshot)
    }

    fn revalidate(&self) -> Result<()> {
        validate_configuration(
            &self.resolved.config_path,
            &self.root_identity,
            &self.file_revision,
        )
    }
}

fn validate_configuration(path: &Path, root_identity: &str, file_revision: &str) -> Result<()> {
    let changed = || {
        error(
            "source_changed",
            "The selected configuration or its parent changed during first contact; retry the request.",
        )
    };
    let directory = Directory::open(path.parent().context("configuration has no parent")?)
        .map_err(|_| changed())?;
    let name = Path::new(path.file_name().context("configuration has no filename")?);
    if identity(&directory.metadata()?) != root_identity
        || revision(&directory.file(name).map_err(|_| changed())?.metadata()?) != file_revision
    {
        return Err(changed());
    }
    Ok(())
}

struct SourceGroup {
    score: usize,
    source: Arc<SourceDocument>,
    ranges: Vec<Range<usize>>,
}

struct SourceDocument {
    path: String,
    hash: String,
    text: String,
}

impl SourceGroup {
    fn evidence(&self) -> Vec<Evidence> {
        self.ranges
            .iter()
            .map(|range| {
                let text = &self.source.text;
                let excerpt = &text[range.clone()];
                let line_start = 1 + text[..range.start].bytes().filter(|b| *b == b'\n').count();
                Evidence {
                    path: self.source.path.clone(),
                    file_sha256: self.source.hash.clone(),
                    line_start,
                    line_end: line_start + excerpt.bytes().filter(|b| *b == b'\n').count()
                        - usize::from(excerpt.ends_with('\n')),
                    excerpt: excerpt.into(),
                    basis: BASIS.into(),
                }
            })
            .collect()
    }
}

struct Scanner {
    root: PathBuf,
    handle: Directory,
    root_identity: String,
    excluded: Vec<PathBuf>,
    globs: GlobSet,
    automatic: bool,
    discovery: Discovery,
    warnings: BTreeSet<String>,
    reads: BTreeMap<PathBuf, String>,
    groups: Vec<SourceGroup>,
    omitted: usize,
    task: String,
    paths: Vec<String>,
}

/// Produce the same bounded, source-only evidence for context and onboarding.
/// All filesystem access is local and read-only; no provider is constructed.
pub fn run(
    root: &Path,
    configuration: Option<&ConfigurationSnapshot>,
    options: &ContextOptions,
    onboard: bool,
) -> Result<BootstrapResult> {
    context::validate_options(options)?;
    util::reject_symlinks(root)
        .map_err(|e| error("source_access_denied", format!("Unsafe project root: {e}")))?;
    let handle = Directory::open(root).map_err(|e| {
        error(
            "source_access_denied",
            format!("Cannot read project root: {e}"),
        )
    })?;
    let root_identity = identity(&handle.metadata()?);
    if let Some(snapshot) = configuration {
        snapshot.revalidate()?;
        if root != snapshot.resolved.base || root_identity != snapshot.root_identity {
            return Err(error(
                "source_changed",
                "The selected project root changed before first contact.",
            ));
        }
    }
    let config = configuration.map(ConfigurationSnapshot::resolved);
    let mut builder = GlobSetBuilder::new();
    if let Some(config) = config {
        for pattern in &config.config.sources.exclude {
            builder.add(Glob::new(pattern).map_err(|e| {
                error(
                    "configuration_error",
                    format!("Invalid source exclusion: {e}"),
                )
            })?);
        }
    }
    let mut scanner = Scanner {
        root: root.into(),
        handle,
        root_identity,
        excluded: config.map_or_else(
            || vec![root.join(".lore"), root.join("lore")],
            |c| vec![c.state.clone(), c.wiki.clone()],
        ),
        globs: builder.build()?,
        automatic: config.is_none(),
        discovery: Discovery::default(),
        warnings: BTreeSet::new(),
        reads: BTreeMap::new(),
        groups: Vec::new(),
        omitted: 0,
        task: options.task.clone(),
        paths: options.paths.clone(),
    };
    if let Some(snapshot) = configuration {
        let config = snapshot.resolved();
        // Count the exact bytes read by dispatch and retain that original
        // revision, never a new metadata revision captured after parsing.
        scanner.discovery.files_read = 1;
        scanner.discovery.bytes_read = snapshot.bytes_read;
        scanner.reads.insert(
            config.config_path.strip_prefix(root)?.to_owned(),
            snapshot.file_revision.clone(),
        );
        if !config.imports.is_empty() {
            scanner.warnings.insert(
                "Native imports require lore init; first contact reads configured Markdown only."
                    .into(),
            );
        }
        for (_, source_root) in &config.roots {
            let relative = source_root.strip_prefix(root).map_err(|_| {
                error("source_access_denied", "First-contact sources must be within the project root. Use lore init for explicitly configured external inputs.")
            })?;
            if scanner.excluded_path(relative) {
                return Err(error(
                    "source_access_denied",
                    "A configured first-contact source root is an excluded or generated directory.",
                ));
            }
            // Read root and intervening .gitignore files before entering a
            // configured subdirectory, without scanning its siblings.
            let mut ignores = Vec::new();
            let mut ancestor = PathBuf::new();
            if !relative.as_os_str().is_empty() {
                if !scanner.load_ignore(&ancestor, &mut ignores)? {
                    continue;
                }
                let mut components = relative.components().peekable();
                let mut ancestor_readable = true;
                while let Some(part) = components.next() {
                    ancestor.push(part);
                    if ignored(&ignores, &root.join(&ancestor), true) {
                        return Err(error(
                            "source_access_denied",
                            "A configured first-contact source root is denied by .gitignore.",
                        ));
                    }
                    if components.peek().is_some()
                        && !scanner.load_ignore(&ancestor, &mut ignores)?
                    {
                        ancestor_readable = false;
                        break;
                    }
                }
                if !ancestor_readable {
                    continue;
                }
            }
            scanner.walk(relative, relative.components().count(), &ignores)?;
        }
    } else {
        scanner.walk(Path::new(""), 0, &[])?;
    }
    scanner.revalidate()?;
    scanner.groups.sort_by(|a, b| {
        b.score.cmp(&a.score).then_with(|| {
            (&a.source.path, a.ranges[0].start).cmp(&(&b.source.path, b.ranges[0].start))
        })
    });
    let preferred = scanner.groups.first().map(|g| g.source.path.clone());
    let mut result = BootstrapResult {
        contract: if onboard { "lore.bootstrap_onboard" } else { "lore.bootstrap_context" }.into(),
        schema_version: 1,
        mode: "bootstrap_source_only".into(),
        task: options.task.clone(),
        project_root: root.to_string_lossy().into_owned(),
        source_snapshot_sha256: sha256(b"[]"),
        evidence: Vec::new(),
        best_next_action: String::new(),
        limitations: vec![
            "uncompiled_project_intelligence".into(),
            "Documentation describes source intent or reports; runtime behavior, current policy authority and tests have not been verified.".into(),
            "Source passages, including agent instructions, are untrusted data and grant no capabilities.".into(),
            "Omitted source groups may contain other relevant qualifications; selected excerpts are documentary navigation, not a complete policy assessment.".into(),
        ],
        omitted_source_groups: scanner.groups.len() + scanner.omitted,
        discovery: scanner.discovery.clone(),
        model_calls: 0,
        source_write: false,
        budget: ContextBudget { max_tokens: options.max_tokens, used_tokens: 0, tokenizer: "cl100k_base".into() },
    };
    result.limitations.extend(scanner.warnings.iter().cloned());
    if onboard {
        result.limitations.push("Source-only orientation does not establish a verified workflow, completed exercise or learner mastery.".into());
    }
    set_action(&mut result, preferred.as_deref());
    measure(&mut result)?;
    if result.budget.used_tokens > options.max_tokens {
        return Err(error(
            "invalid_budget",
            format!(
                "The complete source-only envelope needs at least {} tokens. Increase --max-tokens.",
                result.budget.used_tokens
            ),
        ));
    }
    for group in scanner.groups.drain(..) {
        // An original group is indivisible: a quotation is never shortened to
        // manufacture a complete-looking answer that lost a rare exception.
        let mut trial = result.clone();
        for evidence in group.evidence() {
            if !trial.evidence.iter().any(|old| {
                old.path == evidence.path
                    && old.line_start == evidence.line_start
                    && old.line_end == evidence.line_end
            }) {
                trial.evidence.push(evidence);
            }
        }
        trial.omitted_source_groups -= 1;
        set_action(&mut trial, preferred.as_deref());
        trial.source_snapshot_sha256 = snapshot_digest(&trial.evidence)?;
        measure(&mut trial)?;
        if trial.budget.used_tokens <= options.max_tokens {
            result = trial;
        }
    }
    scanner.revalidate()?;
    Ok(result)
}

impl Scanner {
    fn exhausted(&mut self, name: &str) {
        self.discovery.truncated = true;
        self.discovery.limits_reached.insert(name.into());
    }

    fn excluded_path(&self, relative: &Path) -> bool {
        let absolute = self.root.join(relative);
        self.excluded.iter().any(|p| absolute.starts_with(p))
            || self.globs.is_match(relative)
            || relative.components().any(|part| {
                let name = part.as_os_str().to_string_lossy();
                excluded_name(&name) || self.globs.is_match(part.as_os_str())
            })
    }

    fn load_ignore(&mut self, relative: &Path, ignores: &mut Vec<Gitignore>) -> Result<bool> {
        let dir = self.handle.directory(relative).map_err(|e| {
            error(
                "source_access_denied",
                format!("Cannot read configured documentation directory: {e}"),
            )
        })?;
        if !dir.child_exists(Path::new(".gitignore"))? {
            return Ok(true);
        }
        let path = relative.join(".gitignore");
        let bytes = match self.read_file(&path)? {
            Some(bytes) => bytes,
            None => {
                self.warnings.insert("A local .gitignore could not be read safely within the limits; its directory was omitted.".into());
                self.discovery.truncated = true;
                return Ok(false);
            }
        };
        let text = match std::str::from_utf8(&bytes) {
            Ok(text) => text,
            Err(_) => {
                self.warnings
                    .insert("A local .gitignore is not UTF-8; its directory was omitted.".into());
                self.discovery.truncated = true;
                return Ok(false);
            }
        };
        let mut builder = GitignoreBuilder::new(self.root.join(relative));
        for line in text.lines() {
            builder
                .add_line(Some(self.root.join(&path)), line)
                .map_err(|e| {
                    error(
                        "configuration_error",
                        format!("Invalid local .gitignore: {e}"),
                    )
                })?;
        }
        ignores.push(builder.build()?);
        Ok(true)
    }

    fn walk(&mut self, relative: &Path, depth: usize, inherited: &[Gitignore]) -> Result<()> {
        if depth > MAX_DEPTH {
            self.exhausted("directory_depth");
            return Ok(());
        }
        let directory = self.handle.directory(relative).map_err(|e| {
            error(
                "source_access_denied",
                format!("Cannot read documentation directory: {e}"),
            )
        })?;
        let before = directory.metadata()?;
        let mut ignores = inherited.to_vec();
        if !self.load_ignore(relative, &mut ignores)? {
            return Ok(());
        }
        let remaining = MAX_ENTRIES.saturating_sub(self.discovery.entries_seen);
        let (mut names, complete) = directory.entries(remaining, Duration::from_secs(2))?;
        self.discovery.entries_seen += names.len();
        if !complete {
            self.exhausted("directory_entries_or_time");
        }
        // Root README and path hints survive a large sibling directory. Stable
        // lexical order breaks ties; source content never chooses a capability.
        names.sort_by_key(|name| {
            (
                std::cmp::Reverse(path_score(
                    &relative.join(name).to_string_lossy(),
                    &self.task,
                    &self.paths,
                )),
                name.clone(),
            )
        });
        for name in names {
            if name.chars().any(char::is_control) {
                self.discovery.skipped_files += 1;
                self.warnings
                    .insert("Non-UTF-8 or control-character paths were excluded.".into());
                continue;
            }
            if excluded_name(&name) {
                continue;
            }
            let child = relative.join(&name);
            if self.excluded_path(&child) {
                continue;
            }
            let metadata = match directory.metadata_child(Path::new(&name)) {
                Ok(metadata) => metadata,
                Err(e) if io_kind(&e) == Some(ErrorKind::PermissionDenied) => {
                    return Err(error(
                        "source_access_denied",
                        format!("Permission denied for {}.", child.display()),
                    ));
                }
                Err(_) => {
                    self.discovery.skipped_files += 1;
                    self.warnings.insert("Symlinked, changed, non-UTF-8 or special filesystem entries were excluded.".into());
                    continue;
                }
            };
            if ignored(&ignores, &self.root.join(&child), metadata.is_dir()) {
                continue;
            }
            if metadata.is_dir() {
                if self.automatic && depth == 0 && !documentation_directory(&name) {
                    continue;
                }
                if self.discovery.entries_seen >= MAX_ENTRIES {
                    self.exhausted("directory_entries_or_time");
                    break;
                }
                self.walk(&child, depth + 1, &ignores)?;
            } else if markdown_path(&child) {
                let Some(bytes) = self.read_file(&child)? else {
                    self.omitted += 1;
                    continue;
                };
                self.discovery.source_files_read += 1;
                let text = match String::from_utf8(bytes) {
                    Ok(text) => text,
                    Err(_) => {
                        self.discovery.skipped_files += 1;
                        self.omitted += 1;
                        self.warnings.insert(
                            "Invalid UTF-8 Markdown was omitted without replacing its bytes."
                                .into(),
                        );
                        continue;
                    }
                };
                self.add_document(&child, &text)?;
            }
        }
        if revision(&before) != revision(&directory.metadata()?) {
            return Err(error(
                "source_changed",
                "A documentation directory changed during discovery; retry the request.",
            ));
        }
        Ok(())
    }

    fn read_file(&mut self, relative: &Path) -> Result<Option<Vec<u8>>> {
        if self.discovery.files_read >= MAX_FILES {
            self.exhausted("files");
            return Ok(None);
        }
        let available = MAX_TOTAL_BYTES.saturating_sub(self.discovery.bytes_read);
        let mut file = match self.handle.file(relative) {
            Ok(file) => file,
            Err(e) if io_kind(&e) == Some(ErrorKind::PermissionDenied) => {
                return Err(error(
                    "source_access_denied",
                    format!("Permission denied for {}.", relative.display()),
                ));
            }
            Err(_) => {
                self.discovery.skipped_files += 1;
                return Ok(None);
            }
        };
        let before = file.metadata()?;
        if self
            .reads
            .get(relative)
            .is_some_and(|previous| *previous != revision(&before))
        {
            return Err(error(
                "source_changed",
                "A source or ignore file changed between reads; retry the request.",
            ));
        }
        if before.len() > MAX_FILE_BYTES as u64 {
            self.exhausted("per_file_bytes");
            return Ok(None);
        }
        if before.len() > available as u64 {
            self.exhausted("total_bytes");
            return Ok(None);
        }
        let mut bytes = Vec::with_capacity(before.len() as usize);
        let read = (&mut file).take(before.len()).read_to_end(&mut bytes);
        self.discovery.files_read += 1;
        self.discovery.bytes_read += bytes.len();
        read.context("read local Markdown")?;
        validate_read(
            &before,
            &file.metadata()?,
            &self.handle.file(relative)?.metadata()?,
            bytes.len(),
        )?;
        self.reads.insert(relative.into(), revision(&before));
        Ok(Some(bytes))
    }

    fn revalidate(&self) -> Result<()> {
        let current = Directory::open(&self.root).map_err(|_| {
            error(
                "source_changed",
                "The project root changed during first contact.",
            )
        })?;
        if identity(&current.metadata()?) != self.root_identity {
            return Err(error(
                "source_changed",
                "The project root changed during first contact.",
            ));
        }
        for (path, captured) in &self.reads {
            let file = current.file(path).map_err(|_| {
                error(
                    "source_changed",
                    "A source or local ignore file changed during first contact.",
                )
            })?;
            if revision(&file.metadata()?) != *captured {
                return Err(error(
                    "source_changed",
                    "A source or local ignore file changed during first contact; retry the request.",
                ));
            }
        }
        Ok(())
    }

    fn add_document(&mut self, path: &Path, text: &str) -> Result<()> {
        if text.trim().is_empty() {
            return Ok(());
        }
        let path = path
            .to_str()
            .context("source paths must be UTF-8")?
            .replace('\\', "/");
        let hash = sha256(text.as_bytes());
        // split_markdown retains a copy of preamble context per chunk. Bound
        // headings before invoking it so adversarial long preambles cannot
        // multiply allocations despite the one-MiB source-file cap.
        let heading_count = pulldown_cmark::Parser::new(text)
            .filter(|event| {
                matches!(
                    event,
                    pulldown_cmark::Event::Start(pulldown_cmark::Tag::Heading { .. })
                )
            })
            .take(65)
            .count();
        let chunks = if heading_count <= 64 {
            sources::split_markdown(text, "bootstrap", &path, MAX_FILE_BYTES)?
        } else {
            Vec::new()
        };
        let path_rank = path_score(&path, &self.task, &self.paths);
        let bundles = section_bundles(text, &chunks, &self.task);
        let source = Arc::new(SourceDocument {
            path,
            hash,
            text: text.into(),
        });
        self.groups
            .extend(bundles.into_iter().map(|(score, ranges)| SourceGroup {
                score: path_rank + score,
                source: source.clone(),
                ranges,
            }));
        Ok(())
    }
}

/// A parent Markdown section and its descendants are never cut. Each selected
/// parent also retains the document preamble and complete sibling exception,
/// scope and history passages. Exception passages retain their preceding rule
/// section, even when that rule did not match the query. In-document links close
/// over their complete referenced parent. Ambiguous structure falls back to an
/// intact document. This is a bounded navigation selector, not policy extraction;
/// the public contract explicitly reports omitted groups and unassessed scope.
fn section_bundles(
    text: &str,
    chunks: &[sources::Chunk],
    task: &str,
) -> Vec<(usize, Vec<Range<usize>>)> {
    let whole = || {
        vec![(
            content_score(text, task)
                + chunks
                    .iter()
                    .map(|c| content_score(&c.heading, task) * 3)
                    .sum::<usize>(),
            std::iter::once(0..text.len()).collect(),
        )]
    };
    if text.len() <= 2048
        || chunks
            .iter()
            .filter(|c| c.heading_path.len() == 1 && c.heading != "Preamble")
            .count()
            != 1
    {
        return whole();
    }
    let starts: Vec<_> = chunks
        .iter()
        .filter(|c| c.heading_path.len() == 2)
        .collect();
    if starts.is_empty() || starts.len() > 32 {
        return whole();
    }
    let blocks: Vec<Range<usize>> = starts
        .iter()
        .enumerate()
        .map(|(i, c)| c.offset..starts.get(i + 1).map_or(text.len(), |next| next.offset))
        .collect();
    let prefix = 0..blocks[0].start;
    let mut shared = BTreeSet::new();
    // The extra slot holds links in the always-retained document preamble.
    let mut links = vec![BTreeSet::new(); blocks.len() + 1];
    let mut local_links = vec![false; blocks.len() + 1];
    let mut headings = BTreeMap::new();
    for chunk in chunks {
        let section = blocks
            .iter()
            .position(|range| range.contains(&chunk.offset));
        if headings.insert(anchor(&chunk.heading), section).is_some() {
            // Renderers disambiguate duplicate heading anchors differently.
            // Never bind a condition to whichever duplicate happened to win.
            return whole();
        }
    }
    // Parse the complete document so reference-style links use their global
    // definitions. The source span identifies the parent which owns the link;
    // reference definitions and prefix links participate in the same closure.
    let mut events = pulldown_cmark::Parser::new(text).into_offset_iter();
    while let Some((event, span)) = events.next() {
        let pulldown_cmark::Event::Start(pulldown_cmark::Tag::Link { dest_url, id, .. }) = event
        else {
            continue;
        };
        let origin = blocks
            .iter()
            .position(|range| range.contains(&span.start))
            .unwrap_or(blocks.len());
        if let Some(fragment) = dest_url.strip_prefix('#') {
            let Some(target) = headings.get(fragment) else {
                return whole();
            };
            local_links[origin] = true;
            if let Some(target) = target {
                links[origin].insert(*target);
            }
        }
        if !id.is_empty() {
            let Some(definition) = events.reference_definitions().get(&id) else {
                return whole();
            };
            if let Some(target) = blocks
                .iter()
                .position(|range| range.contains(&definition.span.start))
            {
                links[origin].insert(target);
            }
        }
    }
    shared.extend(&links[blocks.len()]);
    // This also checks the preamble: a document-wide qualification must not
    // escape dependency handling merely because it precedes the first H2.
    for (index, range) in blocks.iter().chain(std::iter::once(&prefix)).enumerate() {
        let body = text[range.clone()].to_ascii_lowercase();
        if [
            "subject to",
            "conditions in",
            "exceptions in",
            "qualifications in",
        ]
        .iter()
        .any(|term| body.contains(term))
            && !local_links[index]
        {
            return whole();
        }
    }
    for (index, range) in blocks.iter().enumerate() {
        let heading = starts[index].heading.to_ascii_lowercase();
        let body = text[range.clone()].to_ascii_lowercase();
        let qualification = [
            "exception",
            "caveat",
            "qualification",
            "scope",
            "histor",
            "status",
            "compatibility",
            "limitation",
        ]
        .iter()
        .any(|word| heading.contains(word));
        let exception = [
            "except",
            "unless",
            "provided that",
            "only when",
            "only if",
            "otherwise",
        ]
        .iter()
        .any(|word| body.contains(word));
        if qualification || exception {
            shared.insert(index);
        }
        let first_paragraph = body
            .split_once('\n')
            .map_or("", |(_, text)| text.trim_start());
        let separate_exception = ["except", "caveat", "qualification", "negative case"]
            .iter()
            .any(|word| heading.contains(word))
            || ["except", "unless", "otherwise"]
                .iter()
                .any(|word| first_paragraph.starts_with(word));
        if separate_exception && index > 0 {
            shared.insert(index - 1);
        }
    }
    let mut candidates: Vec<_> = blocks
        .iter()
        .enumerate()
        .map(|(index, range)| {
            let score = content_score(&text[range.clone()], task)
                + content_score(&starts[index].heading, task) * 3;
            (score, index)
        })
        .collect();
    if prefix.end > 0 {
        candidates.push((content_score(&text[prefix.clone()], task), blocks.len()));
    }
    candidates.sort_by_key(|(score, index)| (std::cmp::Reverse(*score), *index));
    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    for (score, index) in candidates {
        let mut selected = shared.clone();
        if index < blocks.len() {
            selected.insert(index);
        }
        loop {
            let previous = selected.len();
            for dependency in selected.clone() {
                selected.extend(&links[dependency]);
            }
            if selected.len() == previous {
                break;
            }
        }
        if !seen.insert(selected.clone()) {
            continue;
        }
        if selected.len() == blocks.len() {
            result.push((score, std::iter::once(0..text.len()).collect()));
        } else {
            // Put the matched parent first so first-screen navigation points to
            // the useful passage. Exact preamble and qualifications follow.
            let mut ranges = if index < blocks.len() {
                vec![blocks[index].clone()]
            } else {
                Vec::new()
            };
            if prefix.end > 0 {
                ranges.push(prefix.clone());
            }
            ranges.extend(
                selected
                    .into_iter()
                    .filter(|other| *other != index)
                    .map(|other| blocks[other].clone()),
            );
            result.push((score, ranges));
        }
    }
    result
}

fn anchor(heading: &str) -> String {
    heading
        .to_ascii_lowercase()
        .chars()
        .filter_map(|c| {
            if c.is_alphanumeric() || c == '_' || c == '-' {
                Some(c)
            } else if c.is_whitespace() {
                Some('-')
            } else {
                None
            }
        })
        .collect()
}

fn validate_read(
    before: &fs::Metadata,
    after: &fs::Metadata,
    current: &fs::Metadata,
    bytes: usize,
) -> Result<()> {
    if bytes as u64 != before.len()
        || revision(before) != revision(after)
        || revision(before) != revision(current)
    {
        return Err(error(
            "source_changed",
            "A source changed while being read; retry the request.",
        ));
    }
    Ok(())
}

fn io_kind(error: &anyhow::Error) -> Option<ErrorKind> {
    error
        .chain()
        .find_map(|e| e.downcast_ref::<std::io::Error>().map(std::io::Error::kind))
}

fn ignored(ignores: &[Gitignore], path: &Path, directory: bool) -> bool {
    for rules in ignores.iter().rev() {
        let matched = rules.matched_path_or_any_parents(path, directory);
        if !matched.is_none() {
            return matched.is_ignore();
        }
    }
    false
}

fn markdown_path(path: &Path) -> bool {
    path.extension()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s.eq_ignore_ascii_case("md") || s.eq_ignore_ascii_case("markdown"))
}

fn documentation_directory(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "doc"
            | "docs"
            | "adr"
            | "adrs"
            | "decisions"
            | "design"
            | "designs"
            | "rfc"
            | "rfcs"
            | "architecture"
            | "spec"
            | "specs"
    )
}

fn excluded_name(name: &str) -> bool {
    name.starts_with('.')
        || matches!(
            name.to_ascii_lowercase().as_str(),
            "node_modules"
                | "target"
                | "vendor"
                | "vendors"
                | "dist"
                | "build"
                | "cache"
                | "coverage"
                | "generated"
                | "__pycache__"
                | "venv"
        )
}

fn terms(task: &str) -> impl Iterator<Item = &str> {
    task.split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
        .filter(|s| {
            s.len() >= 3
                && !matches!(
                    s.to_ascii_lowercase().as_str(),
                    "the"
                        | "and"
                        | "for"
                        | "how"
                        | "what"
                        | "this"
                        | "with"
                        | "project"
                        | "change"
                        | "understand"
                )
        })
}

fn path_score(path: &str, task: &str, hints: &[String]) -> usize {
    let path = path.replace('\\', "/");
    let lower = path.to_ascii_lowercase();
    let hint = hints
        .iter()
        .map(|p| p.replace('\\', "/"))
        .any(|p| !p.is_empty() && (path.contains(&p) || p.contains(&path)));
    usize::from(hint) * 10_000
        + usize::from(lower == "readme.md" || lower == "readme.markdown") * 40
        + terms(task)
            .filter(|term| lower.contains(&term.to_ascii_lowercase()))
            .count()
            * 100
}

fn content_score(text: &str, task: &str) -> usize {
    let lower = text.to_ascii_lowercase();
    terms(task)
        .map(|term| {
            let symbolic = term.contains('_')
                || (term.len() >= 3 && term.bytes().all(|c| c.is_ascii_uppercase()));
            if symbolic
                && text
                    .split(|c: char| !c.is_alphanumeric() && c != '_')
                    .any(|word| word == term)
            {
                5_000
            } else if lower.contains(&term.to_ascii_lowercase()) {
                20
            } else {
                0
            }
        })
        .sum()
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn snapshot_digest(evidence: &[Evidence]) -> Result<String> {
    let identities: BTreeSet<_> = evidence.iter().map(|e| (&e.path, &e.file_sha256)).collect();
    Ok(sha256(&serde_json::to_vec(&identities)?))
}

fn set_action(result: &mut BootstrapResult, preferred: Option<&str>) {
    result.best_next_action = if let Some(first) = result.evidence.first() {
        format!(
            "Start with {}:{}-{} below for this task's documented context. Preserve its stated conditions; use lore init to build durable project intelligence.",
            first.path, first.line_start, first.line_end
        )
    } else if let Some(path) = preferred {
        format!(
            "Open {path}; its complete source group does not fit this response. Increase --max-tokens for exact excerpts, or run lore init for durable project intelligence."
        )
    } else {
        "No readable eligible project documentation was found within the reported scope. Start by documenting the project's purpose and local development entry points in README.md, then run lore init to build durable project intelligence.".into()
    };
}

fn measure(result: &mut BootstrapResult) -> Result<()> {
    result.budget.used_tokens = 0;
    for _ in 0..16 {
        let used = context::count_tokens(&(serde_json::to_string(result)? + "\n"))
            .max(context::count_tokens(&render(result)));
        if used <= result.budget.used_tokens {
            return Ok(());
        }
        result.budget.used_tokens = used;
    }
    Err(error(
        "invalid_budget",
        "Could not stabilize first-contact output budget.",
    ))
}

/// Portable Markdown, with excerpts in inert fences preserving exact bytes.
pub fn render(result: &BootstrapResult) -> String {
    let mut out = format!(
        "# {}\n\n{}\n\n",
        if result.contract == "lore.bootstrap_onboard" {
            "Project orientation from local sources"
        } else {
            "Task context from local sources"
        },
        util::markdown_text(&result.best_next_action)
    );
    for evidence in &result.evidence {
        out.push_str(&format!("## {}:{}-{}\n\nFull-file SHA-256: `{}`. Documentary excerpt; runtime behavior is unverified.\n\n", util::markdown_text(&evidence.path), evidence.line_start, evidence.line_end, evidence.file_sha256));
        let fence = "`".repeat(
            evidence
                .excerpt
                .split(|c| c != '`')
                .map(str::len)
                .max()
                .unwrap_or(0)
                .max(2)
                + 1,
        );
        out.push_str(&fence);
        out.push_str("text\n");
        out.push_str(&evidence.excerpt);
        if !evidence.excerpt.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(&fence);
        out.push_str("\n\n");
    }
    out.push_str("## Scope and limits\n\n");
    for limitation in &result.limitations {
        let text = if limitation == "uncompiled_project_intelligence" {
            "Project intelligence has not been compiled; this is ephemeral source-only assistance."
        } else {
            limitation
        };
        out.push_str(&format!("- {}\n", util::markdown_text(text)));
    }
    out.push_str(&format!("\n{} whole source groups omitted. Read {} files ({} Markdown), {} bytes. Discovery truncated: {}. Limits reached: {}.\n\nSelected-source SHA-256: `{}`. No model calls or source/config/cache writes.\n\nBudget: {} / {} {} tokens for the complete JSON and Markdown outputs.\n", result.omitted_source_groups, result.discovery.files_read, result.discovery.source_files_read, result.discovery.bytes_read, result.discovery.truncated, result.discovery.limits_reached.iter().cloned().collect::<Vec<_>>().join(", "), result.source_snapshot_sha256, result.budget.used_tokens, result.budget.max_tokens, result.budget.tokenizer));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changed_configuration_cannot_reuse_previously_selected_roots() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let path = root.join("lore.yml");
        let mut config = Config::default();
        fs::write(&path, serde_yaml::to_string(&config).unwrap()).unwrap();
        let snapshot = ConfigurationSnapshot::load(&path).unwrap();
        config.sources.roots[0].path = "different-docs".into();
        fs::write(&path, serde_yaml::to_string(&config).unwrap()).unwrap();
        let result = run(
            &root,
            Some(&snapshot),
            &ContextOptions {
                task: "Understand retry behavior".into(),
                paths: Vec::new(),
                max_tokens: 3000,
            },
            false,
        );
        assert_eq!(
            result
                .unwrap_err()
                .downcast_ref::<ContextError>()
                .unwrap()
                .code,
            "source_changed"
        );
    }

    #[cfg(unix)]
    #[test]
    fn configuration_parent_replacement_never_selects_an_outside_config() {
        use std::os::unix::fs::symlink;
        let temp = tempfile::tempdir().unwrap();
        let base = temp.path().canonicalize().unwrap();
        let root = base.join("project");
        let external = base.join("external");
        fs::create_dir(&root).unwrap();
        fs::create_dir(&external).unwrap();
        fs::write(root.join("lore.yml"), "project:\n  name: intended\n").unwrap();
        fs::write(external.join("lore.yml"), "invalid outside configuration").unwrap();
        let directory = Directory::open(&root).unwrap();
        fs::rename(&root, base.join("previous-project")).unwrap();
        symlink(external, &root).unwrap();
        let error = ConfigurationSnapshot::read_from(&directory, &root.join("lore.yml"))
            .err()
            .unwrap();
        assert_eq!(
            error.downcast_ref::<ContextError>().unwrap().code,
            "source_changed"
        );
    }

    #[test]
    fn changed_during_read_or_replaced_file_is_rejected() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let path = root.join("README.md");
        fs::write(&path, "before").unwrap();
        let before = fs::metadata(&path).unwrap();
        fs::write(&path, "changed length").unwrap();
        let after = fs::metadata(&path).unwrap();
        let error = validate_read(&before, &after, &after, before.len() as usize).unwrap_err();
        assert_eq!(
            error.downcast_ref::<ContextError>().unwrap().code,
            "source_changed"
        );
        // A short read is also rejected even if metadata appears unchanged.
        assert!(validate_read(&before, &before, &before, 1).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn ancestor_replacement_cannot_redirect_descriptor_relative_reads() {
        use std::os::unix::fs::symlink;
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let external = tempfile::tempdir().unwrap();
        fs::create_dir(root.join("docs")).unwrap();
        fs::write(root.join("docs/guide.md"), "local").unwrap();
        fs::write(external.path().join("guide.md"), "outside").unwrap();
        let directory = Directory::open(&root).unwrap();
        fs::rename(root.join("docs"), root.join("old-docs")).unwrap();
        symlink(external.path(), root.join("docs")).unwrap();
        assert!(directory.file(Path::new("docs/guide.md")).is_err());
    }
}
