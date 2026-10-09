//! Bounded, read-only checkout observations. This module never starts a process,
//! changes a source file, or invokes a model. Paths supplied by a model are
//! resolved exclusively against the catalog built by `discover`.
use super::InspectionPath;
use crate::{config::ResolvedConfig, util};
use anyhow::{Context, Result, bail, ensure};
use globset::{Glob, GlobSet, GlobSetBuilder};
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::{self, File, Metadata},
    io::Read,
    path::{Component, Path, PathBuf},
    time::{Duration, Instant, UNIX_EPOCH},
};

const MAX_DEPTH: usize = 24;
const MAX_EXCERPT_BYTES: usize = 12_000;
const MAX_IGNORE_BYTES: usize = 16_384;
const STATIC_QUALIFICATION: &str =
    "Static source inspection; not proof of runtime behavior or test execution.";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct InspectionSettings {
    /// Opt-in until independent coding evaluations justify enabling inspection
    /// by default. Enabling inspection does not authorize hosted code egress.
    pub enabled: bool,
    /// Explicit checkout directory, relative to the configuration directory.
    /// Without this, use the nearest safely discoverable `.git` ancestor.
    pub root: Option<PathBuf>,
    /// Unique source files attempted; rereads consume bytes and time, not slots.
    pub max_files: usize,
    pub max_file_bytes: usize,
    /// Includes source rereads and the bounded local `.gitignore` reads.
    pub max_total_bytes: usize,
    /// Counts all directory entries, including excluded entries.
    pub max_index_entries: usize,
    /// Cumulative inspection I/O time; intervening inference time is excluded.
    pub max_elapsed_ms: u64,
    pub max_excerpt_lines: usize,
}

impl Default for InspectionSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            root: None,
            max_files: 6,
            max_file_bytes: 65_536,
            max_total_bytes: 262_144,
            max_index_entries: 2_000,
            max_elapsed_ms: 1_000,
            max_excerpt_lines: 80,
        }
    }
}

impl InspectionSettings {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            (1..=20).contains(&self.max_files),
            "inspection.max_files must be 1..20"
        );
        ensure!(
            (1..=262_144).contains(&self.max_file_bytes),
            "inspection.max_file_bytes must be 1..262144"
        );
        ensure!(
            self.max_total_bytes >= self.max_file_bytes && self.max_total_bytes <= 1_048_576,
            "inspection.max_total_bytes must be at least max_file_bytes and at most 1048576"
        );
        ensure!(
            (1..=20_000).contains(&self.max_index_entries),
            "inspection.max_index_entries must be 1..20000"
        );
        ensure!(
            (1..=10_000).contains(&self.max_elapsed_ms),
            "inspection.max_elapsed_ms must be 1..10000"
        );
        ensure!(
            (1..=200).contains(&self.max_excerpt_lines),
            "inspection.max_excerpt_lines must be 1..200"
        );
        if let Some(root) = &self.root {
            ensure!(
                !root.as_os_str().is_empty()
                    && !root.is_absolute()
                    && root
                        .to_str()
                        .is_some_and(|s| s.len() <= 4096 && !s.chars().any(char::is_control))
                    && !root.components().any(|c| matches!(c, Component::Prefix(_))),
                "inspection.root must be a nonempty configuration-relative path"
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CodeObservation {
    pub id: String,
    /// Exact, checkout-relative path with forward slashes.
    pub path: String,
    /// One-based, inclusive bounds of the exact, unmodified excerpt.
    pub start_line: usize,
    pub end_line: usize,
    pub excerpt: String,
    /// SHA-256 digest of the complete original file bytes, including line endings.
    pub content_hash: String,
    /// `static_source` or `static_test`; a test was read, never executed.
    pub kind: String,
    pub qualification: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectionCandidate {
    pub path: String,
    pub kind: String,
    pub bytes: u64,
    pub reasons: Vec<String>,
    pub score: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectionBudget {
    pub max_files: usize,
    pub files_read: usize,
    pub max_file_bytes: usize,
    pub max_total_bytes: usize,
    pub bytes_read: usize,
    pub ignore_files_read: usize,
    pub max_index_entries: usize,
    pub index_entries: usize,
    pub max_elapsed_ms: u64,
    pub elapsed_ms: u64,
    pub exhausted: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectionReport {
    /// `disabled`, `unavailable`, `complete`, or `partial`.
    pub status: String,
    pub root: Option<String>,
    /// Includes the checkout identity, eligible paths and file metadata, and
    /// ignore-file digests. Contents of cached observations are rehashed too.
    pub index_digest: String,
    /// Incomplete catalogs must never authorize an intelligence cache hit.
    pub index_complete: bool,
    pub observations: Vec<CodeObservation>,
    pub budget: InspectionBudget,
    pub warnings: Vec<String>,
}

pub struct InspectionSession {
    settings: InspectionSettings,
    root: Option<PathBuf>,
    root_handle: Option<safe_fs::Directory>,
    root_identity: String,
    task_terms: BTreeSet<String>,
    candidates: Vec<InspectionCandidate>,
    attempted: BTreeSet<String>,
    observations: Vec<CodeObservation>,
    report: InspectionReport,
    elapsed: Duration,
}

impl InspectionSession {
    pub fn discover(
        config: &ResolvedConfig,
        task: &str,
        hints: &[InspectionPath],
        paths: &[String],
        settings: &InspectionSettings,
    ) -> Result<Self> {
        settings.validate()?;
        let mut session = Self {
            settings: settings.clone(),
            root: None,
            root_handle: None,
            root_identity: String::new(),
            task_terms: terms(task),
            candidates: Vec::new(),
            attempted: BTreeSet::new(),
            observations: Vec::new(),
            report: InspectionReport {
                status: if settings.enabled {
                    "unavailable"
                } else {
                    "disabled"
                }
                .into(),
                root: None,
                index_digest: String::new(),
                index_complete: false,
                observations: Vec::new(),
                budget: InspectionBudget {
                    max_files: settings.max_files,
                    files_read: 0,
                    max_file_bytes: settings.max_file_bytes,
                    max_total_bytes: settings.max_total_bytes,
                    bytes_read: 0,
                    ignore_files_read: 0,
                    max_index_entries: settings.max_index_entries,
                    index_entries: 0,
                    max_elapsed_ms: settings.max_elapsed_ms,
                    elapsed_ms: 0,
                    exhausted: Vec::new(),
                },
                warnings: Vec::new(),
            },
            elapsed: Duration::ZERO,
        };
        if !settings.enabled {
            return Ok(session);
        }
        let phase = Instant::now();
        let Some(root) = checkout_root(config, settings)? else {
            session.warn(
                "No local checkout was found; configure context.inspection.root to select one.",
            );
            session.elapsed += phase.elapsed();
            return Ok(session);
        };
        ensure!(
            !root.starts_with(&config.wiki) && !root.starts_with(&config.state),
            "inspection root cannot be generated Lore output"
        );
        let handle = safe_fs::Directory::open(&root)?;
        session.root_identity = identity(&handle.metadata()?);
        session.report.root = Some(root.to_string_lossy().into_owned());
        session.root = Some(root.clone());
        session.root_handle = Some(handle);
        let mut builder = GlobSetBuilder::new();
        for pattern in &config.config.sources.exclude {
            builder.add(Glob::new(pattern).context("invalid source exclude glob")?);
        }
        let exclusions = builder.build()?;
        let mut index = Vec::new();
        let mut ignore_hashes = Vec::new();
        session.report.index_complete = true;
        session.walk(
            Path::new(""),
            &[],
            &exclusions,
            config,
            &phase,
            &mut index,
            &mut ignore_hashes,
        )?;
        index.sort();
        ignore_hashes.sort();
        session.report.index_digest = util::json_digest(&(
            "checkout-index-v1",
            &root,
            &session.root_identity,
            index,
            ignore_hashes,
        ))?;
        session.rank(config, hints, paths);
        session.elapsed += phase.elapsed();
        session.report.status = if session.report.index_complete {
            "complete"
        } else {
            "partial"
        }
        .into();
        Ok(session)
    }

    pub fn candidates(&self) -> &[InspectionCandidate] {
        &self.candidates
    }

    pub fn report(&self) -> InspectionReport {
        let mut report = self.report.clone();
        report.observations = self.observations.clone();
        report.budget.elapsed_ms = self.elapsed.as_millis().min(u64::MAX as u128) as u64;
        report
    }

    /// Read at most three task-relevant files, favoring an implementation and
    /// its corresponding test. Leave capacity for subsequent investigation.
    pub fn inspect_initial(&mut self) {
        let selected: Vec<String> = self
            .candidates
            .iter()
            .filter(|c| c.score > 0)
            .take(self.settings.max_files.min(3))
            .map(|c| c.path.clone())
            .collect();
        for path in selected {
            if let Err(error) = self.inspect(&path) {
                self.warn(&format!("Inspection skipped {path}: {error}"));
            }
        }
    }

    /// Exact catalog lookup is the only path-selection interface. Repeated
    /// reads consume byte/time budget and replace the older observation.
    pub fn inspect(&mut self, path: &str) -> Result<CodeObservation> {
        let phase = Instant::now();
        let result = self.inspect_impl(path, &phase);
        self.elapsed += phase.elapsed();
        if result.is_err() && self.report.status == "complete" {
            self.report.status = "partial".into();
        }
        result
    }

    /// Rehash every cached observation, including deeper investigation reads.
    /// A metadata-only comparison is never sufficient for a cache hit.
    pub fn revalidate(&mut self, observations: &[CodeObservation]) -> Result<bool> {
        if observations.len() > self.settings.max_files {
            return Ok(false);
        }
        let mut paths = BTreeSet::new();
        let mut valid = true;
        for expected in observations {
            if !paths.insert(expected.path.clone()) || validate_observation(expected).is_err() {
                return Ok(false);
            }
            match self.inspect(&expected.path) {
                Ok(observed) => valid &= observed == *expected,
                Err(_) => valid = false,
            }
        }
        Ok(valid)
    }

    fn inspect_impl(&mut self, path: &str, phase: &Instant) -> Result<CodeObservation> {
        ensure!(
            valid_relative(path),
            "inspection requires an exact allowlisted relative path"
        );
        let candidate = self
            .candidates
            .iter()
            .find(|c| c.path == path)
            .context("inspection path is not in the allowlisted checkout catalog")?
            .clone();
        if !self.attempted.contains(path) {
            if self.attempted.len() >= self.settings.max_files {
                self.exhaust("files");
                bail!("inspection file budget exhausted");
            }
            self.attempted.insert(path.into());
            self.report.budget.files_read = self.attempted.len();
        }
        self.check_time(phase)?;
        self.check_root()?;
        let bytes = self.read_file(Path::new(path), self.settings.max_file_bytes, phase)?;
        let text = source_text(&bytes)?;
        ensure!(!generated_text(text), "generated source is excluded");
        ensure!(
            !sensitive_text(text),
            "potential credential content is excluded"
        );
        let (start_line, end_line, excerpt) =
            excerpt(text, &self.task_terms, self.settings.max_excerpt_lines)?;
        let content_hash = format!("sha256:{:x}", Sha256::digest(&bytes));
        let mut observation = CodeObservation {
            id: String::new(),
            path: candidate.path,
            start_line,
            end_line,
            excerpt,
            content_hash,
            kind: candidate.kind,
            qualification: STATIC_QUALIFICATION.into(),
        };
        observation.id = observation_id(&observation)?;
        self.check_time(phase)?;
        self.check_root()?;
        self.observations.retain(|o| o.path != observation.path);
        self.observations.push(observation.clone());
        Ok(observation)
    }

    fn check_root(&self) -> Result<()> {
        let root = self.root.as_ref().context("inspection is unavailable")?;
        let current = safe_fs::Directory::open(root)?;
        ensure!(
            identity(&current.metadata()?) == self.root_identity,
            "checkout root changed during inspection"
        );
        Ok(())
    }

    fn read_file(&mut self, relative: &Path, max: usize, phase: &Instant) -> Result<Vec<u8>> {
        self.check_time(phase)?;
        let root = self
            .root_handle
            .as_ref()
            .context("inspection is unavailable")?;
        let mut file = root.file(relative)?;
        let before = file.metadata()?;
        ensure!(
            before.is_file(),
            "only regular source files may be inspected"
        );
        ensure!(
            before.len() <= max as u64,
            "file exceeds the inspection per-file byte limit"
        );
        let remaining = self
            .settings
            .max_total_bytes
            .saturating_sub(self.report.budget.bytes_read);
        if before.len() > remaining as u64 {
            self.exhaust("bytes");
            bail!("inspection total byte budget exhausted");
        }
        let mut bytes = Vec::with_capacity(before.len() as usize);
        // Never read a sentinel byte beyond the budget. A growth/shrink race is
        // rejected using the descriptor metadata before and after this read.
        let read = (&mut file).take(before.len()).read_to_end(&mut bytes);
        self.report.budget.bytes_read += bytes.len();
        read.context("read allowlisted source file")?;
        let after = file.metadata()?;
        let current = root.file(relative)?.metadata()?;
        ensure!(
            bytes.len() as u64 == before.len()
                && revision(&before) == revision(&after)
                && revision(&before) == revision(&current),
            "file changed while being inspected"
        );
        self.check_time(phase)?;
        Ok(bytes)
    }

    #[allow(clippy::too_many_arguments)]
    fn walk(
        &mut self,
        relative: &Path,
        parent_ignores: &[Gitignore],
        exclusions: &GlobSet,
        config: &ResolvedConfig,
        phase: &Instant,
        index: &mut Vec<(String, String, String)>,
        ignore_hashes: &mut Vec<(String, String)>,
    ) -> Result<()> {
        if self.check_time(phase).is_err() {
            self.report.index_complete = false;
            return Ok(());
        }
        if relative.components().count() > MAX_DEPTH {
            self.report.index_complete = false;
            self.warn("Checkout discovery reached its directory-depth limit.");
            return Ok(());
        }
        let root_path = self
            .root
            .as_ref()
            .context("inspection is unavailable")?
            .clone();
        let directory = match self.root_handle.as_ref().unwrap().directory(relative) {
            Ok(dir) => dir,
            Err(_) => {
                self.report.index_complete = false;
                self.warn(
                    "Some checkout directories were unavailable or changed during discovery.",
                );
                return Ok(());
            }
        };
        let before = directory.metadata()?;
        let mut ignores = parent_ignores.to_vec();
        // No global ignore files, Git config, hooks, or parent files are read.
        // Parse only the local `.gitignore` through the same safe boundary.
        let ignore_path = relative.join(".gitignore");
        if directory.child_exists(Path::new(".gitignore"))? {
            match self.read_file(&ignore_path, MAX_IGNORE_BYTES, phase) {
                Ok(bytes) => {
                    self.report.budget.ignore_files_read += 1;
                    let content = match std::str::from_utf8(&bytes) {
                        Ok(content) => content,
                        Err(_) => {
                            self.report.index_complete = false;
                            self.warn("A local .gitignore was not valid UTF-8; its directory was skipped.");
                            return Ok(());
                        }
                    };
                    let mut builder = GitignoreBuilder::new(root_path.join(relative));
                    for line in content.lines() {
                        if builder
                            .add_line(Some(root_path.join(&ignore_path)), line)
                            .is_err()
                        {
                            self.report.index_complete = false;
                            self.warn("A local .gitignore could not be safely parsed; its directory was skipped.");
                            return Ok(());
                        }
                    }
                    ignores.push(builder.build()?);
                    ignore_hashes.push((portable(&ignore_path)?, util::digest(&bytes)));
                }
                Err(_) => {
                    self.report.index_complete = false;
                    self.warn("A local .gitignore could not be read within the inspection limits; its directory was skipped.");
                    return Ok(());
                }
            }
        }
        let remaining = self
            .settings
            .max_index_entries
            .saturating_sub(self.report.budget.index_entries);
        let remaining_time = Duration::from_millis(self.settings.max_elapsed_ms)
            .saturating_sub(self.elapsed + phase.elapsed());
        let (names, complete) = directory.entries(remaining, remaining_time)?;
        self.report.budget.index_entries += names.len();
        if !complete {
            self.report.index_complete = false;
            self.exhaust("index_entries");
        }
        for name in names {
            if self.check_time(phase).is_err() {
                self.report.index_complete = false;
                break;
            }
            let child = relative.join(&name);
            let Ok(path) = portable(&child) else { continue };
            if !valid_relative(&path) || excluded_name(&name) {
                continue;
            }
            let absolute = root_path.join(&child);
            if absolute.starts_with(&config.wiki)
                || absolute.starts_with(&config.state)
                || excluded_path(&path, &absolute, exclusions, config)
            {
                continue;
            }
            let metadata = match directory.metadata_child(Path::new(&name)) {
                Ok(m) => m,
                Err(_) => continue, // Includes symlinks, junctions and special files.
            };
            let is_dir = metadata.is_dir();
            if ignored(&ignores, &absolute, is_dir) {
                continue;
            }
            if is_dir {
                if self.report.budget.index_entries >= self.settings.max_index_entries {
                    self.report.index_complete = false;
                    self.exhaust("index_entries");
                    break;
                }
                self.walk(
                    &child,
                    &ignores,
                    exclusions,
                    config,
                    phase,
                    index,
                    ignore_hashes,
                )?;
            } else if metadata.is_file() && eligible_file(&path) {
                let kind = if is_test(&path) {
                    "static_test"
                } else {
                    "static_source"
                };
                index.push((path.clone(), kind.into(), revision(&metadata)));
                // Oversized files remain in the index, but cannot be selected.
                if metadata.len() > 0 && metadata.len() <= self.settings.max_file_bytes as u64 {
                    self.candidates.push(InspectionCandidate {
                        path,
                        kind: kind.into(),
                        bytes: metadata.len(),
                        reasons: Vec::new(),
                        score: 0,
                    });
                }
            }
        }
        if revision(&before) != revision(&directory.metadata()?) {
            self.report.index_complete = false;
            self.warn("A checkout directory changed during discovery; cache reuse is disabled for this catalog.");
        }
        Ok(())
    }

    fn rank(&mut self, config: &ResolvedConfig, hints: &[InspectionPath], paths: &[String]) {
        let Some(root) = &self.root else { return };
        let mut recorded = BTreeSet::new();
        let mut requested = BTreeSet::new();
        for path in paths {
            if valid_relative(path) {
                requested.insert(path.clone());
            }
        }
        for hint in hints.iter().take(256) {
            if !valid_relative(&hint.path) {
                continue;
            }
            if hint.basis == "mentioned_in_evidence" {
                recorded.insert(hint.path.clone());
            }
            if hint.basis == "source_record"
                && let Some((_, source_root)) =
                    config.roots.iter().find(|(id, _)| *id == hint.root_id)
                && let Ok(path) = source_root.join(&hint.path).strip_prefix(root)
                && let Ok(path) = portable(path)
            {
                recorded.insert(path);
            }
        }
        for candidate in &mut self.candidates {
            if requested
                .iter()
                .any(|p| candidate.path == *p || candidate.path.starts_with(&format!("{p}/")))
            {
                candidate.score += 100;
                candidate.reasons.push("explicit task path".into());
            }
            if recorded.contains(&candidate.path) {
                candidate.score += 60;
                candidate.reasons.push("recorded source location".into());
            }
            let path_terms = terms(&candidate.path);
            let matched = path_terms.intersection(&self.task_terms).count() as u32;
            if matched > 0 {
                candidate.score += matched * 20;
                candidate.reasons.push("task terminology in path".into());
            }
        }
        let seams: Vec<BTreeSet<String>> = self
            .candidates
            .iter()
            .filter(|c| c.kind == "static_source" && c.score > 0)
            .take(16)
            .map(|c| terms(&c.path))
            .collect();
        for candidate in &mut self.candidates {
            if candidate.kind == "static_test" {
                let test_terms = terms(&candidate.path);
                if seams
                    .iter()
                    .any(|seam| seam.intersection(&test_terms).next().is_some())
                {
                    candidate.score += 50;
                    candidate
                        .reasons
                        .push("test path related to a relevant implementation".into());
                }
            }
        }
        self.candidates
            .sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.path.cmp(&b.path)));
    }

    fn check_time(&mut self, phase: &Instant) -> Result<()> {
        if self.elapsed + phase.elapsed() >= Duration::from_millis(self.settings.max_elapsed_ms) {
            self.exhaust("elapsed_ms");
            bail!("inspection elapsed-time budget exhausted");
        }
        Ok(())
    }

    fn exhaust(&mut self, budget: &str) {
        if !self.report.budget.exhausted.iter().any(|b| b == budget) {
            self.report.budget.exhausted.push(budget.into());
        }
    }

    fn warn(&mut self, warning: &str) {
        if self.report.warnings.len() < 16 && !self.report.warnings.iter().any(|w| w == warning) {
            self.report.warnings.push(warning.into());
        }
    }
}

fn checkout_root(
    config: &ResolvedConfig,
    settings: &InspectionSettings,
) -> Result<Option<PathBuf>> {
    if let Some(relative) = &settings.root {
        let root = util::absolute(&config.base, relative)?;
        ensure!(
            root.parent().is_some(),
            "filesystem root cannot be a checkout inspection root"
        );
        safe_fs::Directory::open(&root).context("open configured inspection root")?;
        return Ok(Some(root));
    }
    util::reject_symlinks(&config.base)?;
    for candidate in config.base.ancestors().take(32) {
        if candidate.parent().is_none() {
            break;
        }
        match fs::symlink_metadata(candidate.join(".git")) {
            Ok(metadata) => {
                ensure!(
                    !metadata.file_type().is_symlink(),
                    "checkout .git marker cannot be a symlink"
                );
                if metadata.is_dir() || metadata.is_file() {
                    safe_fs::Directory::open(candidate)?;
                    return Ok(Some(candidate.to_owned()));
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Ok(None),
        }
    }
    Ok(None)
}

fn portable(path: &Path) -> Result<String> {
    let text = path.to_str().context("checkout paths must be UTF-8")?;
    #[cfg(windows)]
    let text = text.replace('\\', "/");
    Ok(text.to_owned())
}

fn valid_relative(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 4096
        && !path.contains('\\')
        && !path.contains(':')
        && !path.chars().any(char::is_control)
        && path
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
        && Path::new(path)
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
}

fn excluded_name(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name.starts_with('.')
        || [
            "node_modules",
            "target",
            "dist",
            "build",
            "coverage",
            "vendor",
            "__pycache__",
            "venv",
            "generated",
            "generated-sources",
            "secrets",
            "credentials",
        ]
        .contains(&name.as_str())
        || name.contains("secret")
        || name.contains("credential")
        || name.contains("private_key")
        || name.ends_with(".min.js")
        || name.ends_with(".min.css")
        || name.ends_with(".generated.rs")
        || name.ends_with(".generated.ts")
        || name.ends_with(".g.cs")
        || name.ends_with(".pb.go")
        || ["id_rsa", "id_dsa", "id_ecdsa", "id_ed25519", "kubeconfig"].contains(&name.as_str())
}

fn excluded_path(
    path: &str,
    absolute: &Path,
    exclusions: &GlobSet,
    config: &ResolvedConfig,
) -> bool {
    exclusions.is_match(path)
        || exclusions.is_match(format!("{path}/"))
        || exclusions.is_match(format!("{path}/__lore_inspection_entry__"))
        || path.split('/').any(|part| exclusions.is_match(part))
        || config.roots.iter().any(|(_, root)| {
            absolute
                .strip_prefix(root)
                .ok()
                .is_some_and(|p| exclusions.is_match(p))
        })
}

fn ignored(ignores: &[Gitignore], path: &Path, is_dir: bool) -> bool {
    for rules in ignores.iter().rev() {
        let matched = rules.matched(path, is_dir);
        if matched.is_ignore() {
            return true;
        }
        if matched.is_whitelist() {
            return false;
        }
    }
    false
}

fn eligible_file(path: &str) -> bool {
    let path = Path::new(path);
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if [
        "makefile",
        "dockerfile",
        "justfile",
        "cmakelists.txt",
        "go.mod",
        "gemfile",
        "rakefile",
    ]
    .contains(&name.as_str())
    {
        return true;
    }
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    [
        "rs", "go", "py", "pyi", "js", "jsx", "ts", "tsx", "mjs", "cjs", "c", "cc", "cpp", "cxx",
        "h", "hh", "hpp", "cs", "java", "kt", "kts", "scala", "swift", "m", "mm", "rb", "php",
        "ex", "exs", "erl", "hrl", "hs", "lhs", "clj", "cljs", "cljc", "edn", "dart", "lua", "pl",
        "pm", "sh", "bash", "zsh", "fish", "sql", "graphql", "gql", "proto", "tf", "hcl", "vue",
        "svelte", "html", "css", "scss", "sass", "less", "sol", "zig", "ml", "mli", "r", "toml",
        "json", "yaml", "yml", "xml", "ini", "cfg",
    ]
    .contains(&ext.as_str())
}

fn is_test(path: &str) -> bool {
    let path = path.to_ascii_lowercase();
    path.split('/')
        .any(|p| ["test", "tests", "spec", "specs", "__tests__"].contains(&p))
        || path.rsplit('/').next().is_some_and(|name| {
            name.starts_with("test_")
                || name.contains("_test.")
                || name.contains(".test.")
                || name.contains(".spec.")
        })
}

fn terms(text: &str) -> BTreeSet<String> {
    let mut separated = String::new();
    let mut lower = false;
    for c in text.chars().take(16_384) {
        if lower && c.is_uppercase() {
            separated.push(' ');
        }
        lower = c.is_lowercase();
        separated.extend(c.to_lowercase());
    }
    separated
        .split(|c: char| !c.is_alphanumeric())
        .filter_map(|word| {
            if word.len() < 3
                || [
                    "the",
                    "and",
                    "for",
                    "with",
                    "from",
                    "this",
                    "that",
                    "into",
                    "implement",
                    "implementation",
                    "change",
                    "update",
                    "add",
                    "fix",
                    "test",
                    "tests",
                    "spec",
                    "specs",
                    "src",
                    "lib",
                    "main",
                    "code",
                    "file",
                    "files",
                    "json",
                    "yaml",
                    "toml",
                    "java",
                    "python",
                    "static",
                    "source",
                    "should",
                    "using",
                    "existing",
                ]
                .contains(&word)
            {
                return None;
            }
            let word = if let Some(stem) = word.strip_suffix("ies") {
                format!("{stem}y")
            } else if word.len() > 4 && word.ends_with('s') && !word.ends_with("ss") {
                word[..word.len() - 1].into()
            } else {
                word.into()
            };
            Some(word)
        })
        .collect()
}

fn source_text(bytes: &[u8]) -> Result<&str> {
    ensure!(
        !bytes.is_empty(),
        "empty files do not provide a source observation"
    );
    ensure!(
        !bytes
            .iter()
            .any(|b| (*b < 32 && !matches!(*b, b'\n' | b'\r' | b'\t' | 12)) || *b == 127),
        "binary or control-byte content is excluded"
    );
    std::str::from_utf8(bytes).context("non-UTF-8 source is excluded")
}

fn generated_text(text: &str) -> bool {
    let header: String = text
        .chars()
        .take(2048)
        .flat_map(char::to_lowercase)
        .collect();
    [
        "@generated",
        "code generated",
        "automatically generated",
        "generated by",
        "do not edit",
    ]
    .iter()
    .any(|marker| header.contains(marker))
}

fn sensitive_text(text: &str) -> bool {
    text.contains("PRIVATE KEY-----")
        || text.lines().any(|line| {
            let Some((key, value)) = line.split_once('=').or_else(|| line.split_once(':')) else {
                return false;
            };
            let key = key.trim().trim_matches(['\'', '"']).to_ascii_lowercase();
            let sensitive = [
                "password",
                "passwd",
                "api_key",
                "apikey",
                "api-key",
                "access_token",
                "client_secret",
                "private_key",
                "aws_secret_access_key",
            ]
            .contains(&key.as_str());
            let value = value.trim().trim_matches([',', '\'', '"']);
            sensitive
                && value.len() >= 8
                && !value.starts_with('$')
                && !value.contains("env(")
                && !value.contains("getenv(")
        })
}

fn excerpt(
    text: &str,
    task_terms: &BTreeSet<String>,
    max_lines: usize,
) -> Result<(usize, usize, String)> {
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    ensure!(!lines.is_empty(), "empty source observation");
    let best = lines
        .iter()
        .enumerate()
        .max_by(|(ai, a), (bi, b)| {
            let sa = terms(a).intersection(task_terms).count();
            let sb = terms(b).intersection(task_terms).count();
            sa.cmp(&sb).then_with(|| bi.cmp(ai))
        })
        .map(|(i, _)| i)
        .unwrap_or(0);
    ensure!(
        lines[best].len() <= MAX_EXCERPT_BYTES,
        "relevant source line exceeds the excerpt byte limit"
    );
    let mut start = best.saturating_sub(max_lines / 3);
    while lines[start..=best].iter().map(|l| l.len()).sum::<usize>() > MAX_EXCERPT_BYTES {
        start += 1;
    }
    let mut end = start;
    let mut bytes = 0;
    while end < lines.len()
        && end - start < max_lines
        && bytes + lines[end].len() <= MAX_EXCERPT_BYTES
    {
        bytes += lines[end].len();
        end += 1;
    }
    ensure!(
        end > best,
        "relevant source line does not fit the observation"
    );
    Ok((start + 1, end, lines[start..end].concat()))
}

fn observation_id(observation: &CodeObservation) -> Result<String> {
    // Compact UTF-8 JSON array, deliberately reproducible by independent
    // evaluators using standard-library SHA-256 (including Python).
    let identity = serde_json::to_vec(&(
        &observation.path,
        observation.start_line,
        observation.end_line,
        &observation.excerpt,
        &observation.content_hash,
    ))?;
    Ok(format!("co_{:x}", Sha256::digest(identity)))
}

/// Validate the transport identity without touching the checkout. The full
/// file digest is independently established by `inspect`/`revalidate`; this
/// check prevents stale or malformed snippet bindings in downstream catalogs.
pub(crate) fn validate_observation(observation: &CodeObservation) -> Result<()> {
    ensure!(
        valid_relative(&observation.path),
        "invalid code observation path"
    );
    ensure!(
        observation.start_line > 0
            && observation.end_line >= observation.start_line
            && observation.end_line - observation.start_line < 200
            && observation.excerpt.len() <= MAX_EXCERPT_BYTES
            && observation.excerpt.split_inclusive('\n').count()
                == observation.end_line - observation.start_line + 1,
        "invalid code observation line bounds"
    );
    source_text(observation.excerpt.as_bytes())?;
    ensure!(
        observation
            .content_hash
            .strip_prefix("sha256:")
            .is_some_and(|hash| hash.len() == 64
                && hash
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))),
        "invalid code observation file hash"
    );
    ensure!(
        observation.kind
            == if is_test(&observation.path) {
                "static_test"
            } else {
                "static_source"
            }
            && observation.qualification == STATIC_QUALIFICATION,
        "invalid static source qualification"
    );
    ensure!(
        observation.id == observation_id(observation)?,
        "code observation identity does not match its contents"
    );
    Ok(())
}

fn identity(metadata: &Metadata) -> String {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        format!("{}:{}", metadata.dev(), metadata.ino())
    }
    #[cfg(not(unix))]
    {
        format!("{:?}", metadata.created().ok())
    }
}

fn revision(metadata: &Metadata) -> String {
    let modified = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok());
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        format!(
            "{}:{}:{:?}:{}:{}:{}",
            identity(metadata),
            metadata.len(),
            modified,
            metadata.ctime(),
            metadata.ctime_nsec(),
            metadata.mode()
        )
    }
    #[cfg(not(unix))]
    {
        format!("{}:{}:{:?}", identity(metadata), metadata.len(), modified)
    }
}

/// Descriptor-relative filesystem access. Unix uses no-follow `openat` for
/// every component, and reads directory entries from the already opened fd.
/// Windows holds non-write/non-delete-sharing handles to every component and
/// rejects all reparse points, including directory junctions.
mod safe_fs {
    use super::*;

    #[cfg(unix)]
    mod platform {
        use super::*;
        use std::{
            ffi::{CStr, CString},
            os::{
                fd::{AsRawFd, FromRawFd, IntoRawFd},
                unix::ffi::OsStrExt,
            },
        };

        pub struct Directory {
            file: File,
        }
        impl Directory {
            pub fn open(path: &Path) -> Result<Self> {
                ensure!(path.is_absolute(), "checkout directory must be absolute");
                let mut directory = Self {
                    file: File::open("/")?,
                };
                for component in path.components() {
                    match component {
                        Component::RootDir => {}
                        Component::Normal(name) => {
                            directory = Self {
                                file: open_at(&directory.file, name, true)?,
                            }
                        }
                        _ => bail!("checkout directory must be normalized"),
                    }
                }
                Ok(directory)
            }
            pub fn directory(&self, path: &Path) -> Result<Self> {
                let mut directory = Self {
                    file: self.file.try_clone()?,
                };
                for component in path.components() {
                    let Component::Normal(name) = component else {
                        bail!("relative directory must be normalized")
                    };
                    directory = Self {
                        file: open_at(&directory.file, name, true)?,
                    };
                }
                Ok(directory)
            }
            pub fn file(&self, path: &Path) -> Result<File> {
                let parent = self.directory(path.parent().unwrap_or(Path::new("")))?;
                let file = open_at(
                    &parent.file,
                    path.file_name().context("source path has no filename")?,
                    false,
                )?;
                ensure!(file.metadata()?.is_file(), "source is not a regular file");
                Ok(file)
            }
            pub fn metadata(&self) -> Result<Metadata> {
                Ok(self.file.metadata()?)
            }
            pub fn metadata_child(&self, path: &Path) -> Result<Metadata> {
                let name = path.file_name().context("entry has no filename")?;
                Ok(open_at(&self.file, name, false)?.metadata()?)
            }
            pub fn child_exists(&self, path: &Path) -> Result<bool> {
                let name = CString::new(path.as_os_str().as_bytes())?;
                let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
                // SAFETY: stat is writable and its contents are never consumed;
                // only the return status is used. The child is not followed.
                let result = unsafe {
                    libc::fstatat(
                        self.file.as_raw_fd(),
                        name.as_ptr(),
                        stat.as_mut_ptr(),
                        libc::AT_SYMLINK_NOFOLLOW,
                    )
                };
                if result == 0 {
                    return Ok(true);
                }
                let error = std::io::Error::last_os_error();
                if error.kind() == std::io::ErrorKind::NotFound {
                    Ok(false)
                } else {
                    Err(error.into())
                }
            }
            pub fn entries(&self, max: usize, time: Duration) -> Result<(Vec<String>, bool)> {
                // Open a new directory description: dup/try_clone would share
                // readdir offsets with the session's root descriptor.
                let file = open_at(&self.file, std::ffi::OsStr::new("."), true)?;
                let fd = file.into_raw_fd();
                // SAFETY: fd is an owned directory descriptor. fdopendir takes
                // ownership on success; the error branch closes it explicitly.
                let stream = unsafe { libc::fdopendir(fd) };
                if stream.is_null() {
                    let error = std::io::Error::last_os_error();
                    unsafe {
                        libc::close(fd);
                    }
                    return Err(error.into());
                }
                struct Stream(*mut libc::DIR);
                impl Drop for Stream {
                    fn drop(&mut self) {
                        unsafe {
                            libc::closedir(self.0);
                        }
                    }
                }
                let stream = Stream(stream);
                let mut names = Vec::new();
                let mut complete = true;
                let start = Instant::now();
                loop {
                    if names.len() >= max || start.elapsed() >= time {
                        complete = false;
                        break;
                    }
                    clear_errno();
                    // SAFETY: stream is live and used only by this synchronous
                    // iterator. Copy d_name before the next readdir call.
                    let entry = unsafe { libc::readdir(stream.0) };
                    if entry.is_null() {
                        if !clean_eof() {
                            complete = false;
                        }
                        break;
                    }
                    let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) }.to_bytes();
                    if name == b"." || name == b".." {
                        continue;
                    }
                    // Non-UTF-8 entries still consume the discovery budget.
                    names.push(std::str::from_utf8(name).unwrap_or("\0").to_owned());
                }
                names.sort();
                Ok((names, complete))
            }
        }
        fn clear_errno() {
            #[cfg(any(target_os = "linux", target_os = "dragonfly"))]
            unsafe {
                *libc::__errno_location() = 0;
            }
            #[cfg(any(target_vendor = "apple", target_os = "freebsd"))]
            unsafe {
                *libc::__error() = 0;
            }
            #[cfg(any(target_os = "android", target_os = "netbsd", target_os = "openbsd"))]
            unsafe {
                *libc::__errno() = 0;
            }
        }
        fn clean_eof() -> bool {
            #[cfg(any(
                target_os = "linux",
                target_os = "android",
                target_vendor = "apple",
                target_os = "freebsd",
                target_os = "dragonfly",
                target_os = "netbsd",
                target_os = "openbsd"
            ))]
            {
                std::io::Error::last_os_error().raw_os_error() == Some(0)
            }
            // Keep observations useful on other Unix targets, but do not claim
            // the catalog was complete without an errno-aware EOF check.
            #[cfg(not(any(
                target_os = "linux",
                target_os = "android",
                target_vendor = "apple",
                target_os = "freebsd",
                target_os = "dragonfly",
                target_os = "netbsd",
                target_os = "openbsd"
            )))]
            {
                false
            }
        }
        fn open_at(parent: &File, name: &std::ffi::OsStr, directory: bool) -> Result<File> {
            let name = CString::new(name.as_bytes()).context("NUL in checkout path")?;
            let flags = libc::O_RDONLY
                | libc::O_CLOEXEC
                | libc::O_NOFOLLOW
                | libc::O_NONBLOCK
                | if directory { libc::O_DIRECTORY } else { 0 };
            // SAFETY: the parent fd and NUL-terminated name live for the call;
            // no O_CREAT is set. Every path component is opened separately.
            let fd = unsafe { libc::openat(parent.as_raw_fd(), name.as_ptr(), flags) };
            if fd < 0 {
                return Err(std::io::Error::last_os_error().into());
            }
            // SAFETY: successful openat returned a new owned descriptor.
            let file = unsafe { File::from_raw_fd(fd) };
            let metadata = file.metadata()?;
            ensure!(
                metadata.is_file() || metadata.is_dir(),
                "special filesystem entries are excluded"
            );
            Ok(file)
        }
    }

    #[cfg(windows)]
    mod platform {
        use super::*;
        use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x00200000;
        const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x02000000;
        const FILE_SHARE_READ: u32 = 1;
        pub struct Directory {
            path: PathBuf,
            guards: Vec<File>,
        }
        pub struct SourceFile {
            file: File,
            _guards: Vec<File>,
        }
        impl SourceFile {
            pub fn metadata(&self) -> std::io::Result<Metadata> {
                self.file.metadata()
            }
        }
        impl Read for SourceFile {
            fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
                self.file.read(buffer)
            }
        }
        impl Directory {
            pub fn open(path: &Path) -> Result<Self> {
                ensure!(path.is_absolute(), "checkout directory must be absolute");
                let mut current = PathBuf::new();
                let mut guards = Vec::new();
                for component in path.components() {
                    ensure!(
                        !matches!(component, Component::CurDir | Component::ParentDir),
                        "checkout directory must be normalized"
                    );
                    current.push(component);
                    if matches!(component, Component::Prefix(_)) {
                        continue;
                    }
                    let handle = open(&current)?;
                    ensure!(
                        handle.metadata()?.is_dir(),
                        "checkout component is not a directory"
                    );
                    guards.push(handle);
                }
                Ok(Self {
                    path: path.into(),
                    guards,
                })
            }
            pub fn directory(&self, path: &Path) -> Result<Self> {
                ensure!(
                    path.components().all(|c| matches!(c, Component::Normal(_))),
                    "relative directory must be normalized"
                );
                Self::open(&self.path.join(path))
            }
            pub fn file(&self, path: &Path) -> Result<SourceFile> {
                let parent = self.directory(path.parent().unwrap_or(Path::new("")))?;
                let file = open(
                    &parent
                        .path
                        .join(path.file_name().context("source path has no filename")?),
                )?;
                ensure!(file.metadata()?.is_file(), "source is not a regular file");
                Ok(SourceFile {
                    file,
                    _guards: parent.guards,
                })
            }
            pub fn metadata(&self) -> Result<Metadata> {
                Ok(self
                    .guards
                    .last()
                    .context("directory has no handle")?
                    .metadata()?)
            }
            pub fn metadata_child(&self, path: &Path) -> Result<Metadata> {
                Ok(open(&self.path.join(path))?.metadata()?)
            }
            pub fn child_exists(&self, path: &Path) -> Result<bool> {
                match fs::symlink_metadata(self.path.join(path)) {
                    Ok(_) => Ok(true),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
                    Err(error) => Err(error.into()),
                }
            }
            pub fn entries(&self, max: usize, time: Duration) -> Result<(Vec<String>, bool)> {
                let mut names = Vec::new();
                let mut complete = true;
                let start = Instant::now();
                let mut entries = fs::read_dir(&self.path)?;
                loop {
                    if names.len() >= max || start.elapsed() >= time {
                        complete = false;
                        break;
                    }
                    let Some(entry) = entries.next() else { break };
                    names.push(entry?.file_name().to_str().unwrap_or("\0").into());
                }
                names.sort();
                Ok((names, complete))
            }
        }
        fn open(path: &Path) -> Result<File> {
            let file = fs::OpenOptions::new()
                .read(true)
                .share_mode(FILE_SHARE_READ)
                .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
                .open(path)?;
            ensure!(
                file.metadata()?.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT == 0,
                "symlinks and reparse points are excluded"
            );
            Ok(file)
        }
    }
    #[cfg(not(any(unix, windows)))]
    compile_error!("safe checkout inspection requires Unix or Windows filesystem support");
    pub use platform::Directory;
}
