//! Recoverable two-slot publication. SQLite and the wiki cannot share a single
//! atomic transaction, so an immutable journal allows interrupted installs to
//! roll forward. Neither the previous output nor its baseline is changed before
//! all inference, evidence validation and staging have succeeded.
use crate::{
    config::ResolvedConfig,
    storage::{self, StoredPage},
    util,
};
use anyhow::{Context, Result, ensure};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    path::{Component, Path, PathBuf},
};
const OWNER: &str = ".lore-owned.json";
const JOURNAL: &str = "publication.json";
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Owner {
    project_id: String,
    generation: String,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Journal {
    project_id: String,
    generation: String,
    wiki: PathBuf,
}

pub struct ProjectLock {
    _file: File,
}
impl ProjectLock {
    pub fn acquire(config: &ResolvedConfig) -> Result<Self> {
        util::reject_symlinks(&config.state)?;
        if config.state.exists() {
            let empty = fs::read_dir(&config.state)?.next().is_none();
            if !empty {
                check_owner(&config.state, &config.project_id)?;
            }
        }
        util::private_dir(&config.state)?;
        if !config.state.join(OWNER).exists() {
            util::atomic_write(
                &config.state.join(OWNER),
                &serde_json::to_vec(&Owner {
                    project_id: config.project_id.clone(),
                    generation: String::new(),
                })?,
            )?;
        }
        let path = config.state.join("lock");
        util::reject_symlinks(&path)?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;
        file.try_lock_exclusive()
            .context("another Lore command holds the project lock")?;
        Ok(Self { _file: file })
    }
}
fn check_owner(directory: &Path, project: &str) -> Result<Owner> {
    let owner: Owner = serde_json::from_str(&util::read_limited(&directory.join(OWNER), 4096)?)
        .context("directory is not a recognized Lore-owned output; select an empty directory")?;
    ensure!(
        owner.project_id == project,
        "directory belongs to another Lore project"
    );
    Ok(owner)
}
pub fn page_path(root: &Path, relative: &str) -> Result<PathBuf> {
    let path = Path::new(relative);
    ensure!(
        !path.is_absolute()
            && path.components().all(|c| matches!(c, Component::Normal(_)))
            && path.extension().and_then(|s| s.to_str()) == Some("md"),
        "unsafe generated page path"
    );
    let full = root.join(path);
    util::reject_symlinks(&full)?;
    Ok(full)
}
pub fn validate_existing(
    config: &ResolvedConfig,
    old: &BTreeMap<String, StoredPage>,
    rebuild: bool,
) -> Result<()> {
    if !config.wiki.exists() {
        ensure!(
            old.is_empty() || rebuild,
            "published wiki is missing; use --rebuild to regenerate it"
        );
        return Ok(());
    }
    check_owner(&config.wiki, &config.project_id)?;
    ensure!(
        !old.is_empty() || rebuild,
        "wiki exists without its matching database baseline; use --rebuild"
    );
    if rebuild {
        return Ok(());
    }
    let mut found = BTreeSet::new();
    walk_files(&config.wiki, &config.wiki, &mut found)?;
    found.remove(OWNER);
    ensure!(
        found == old.keys().cloned().collect(),
        "wiki has missing or unmanaged files; preserve your edits elsewhere, then use --rebuild"
    );
    for page in old.values() {
        let text = util::read_limited(&page_path(&config.wiki, &page.path)?, 4_000_000)?;
        ensure!(
            util::digest(text) == page.output_digest,
            "generated page was edited: {}; use --rebuild only to discard generated edits",
            page.path
        );
    }
    Ok(())
}
fn walk_files(root: &Path, path: &Path, out: &mut BTreeSet<String>) -> Result<()> {
    util::reject_symlinks(path)?;
    for e in fs::read_dir(path)? {
        let e = e?;
        util::reject_symlinks(&e.path())?;
        if e.file_type()?.is_dir() {
            walk_files(root, &e.path(), out)?;
        } else if e.file_type()?.is_file() {
            out.insert(
                e.path()
                    .strip_prefix(root)?
                    .to_str()
                    .context("non UTF-8 generated path")?
                    .replace('\\', "/"),
            );
        } else {
            anyhow::bail!("unsupported entry in generated output");
        }
    }
    Ok(())
}
pub fn has_pending(config: &ResolvedConfig) -> bool {
    config.state.join(JOURNAL).exists()
}
fn stage_wiki(config: &ResolvedConfig, generation: &str) -> Result<PathBuf> {
    Ok(config
        .wiki
        .parent()
        .context("wiki has no parent")?
        .join(format!(".lore-stage-{generation}")))
}
fn backup_wiki(config: &ResolvedConfig, generation: &str) -> Result<PathBuf> {
    Ok(config
        .wiki
        .parent()
        .context("wiki has no parent")?
        .join(format!(".lore-backup-{generation}")))
}
pub fn stage_dir(config: &ResolvedConfig, generation: &str) -> PathBuf {
    config.state.join("staging").join(generation)
}

pub fn recover(config: &ResolvedConfig) -> Result<bool> {
    if !has_pending(config) {
        return Ok(false);
    }
    let journal: Journal =
        serde_json::from_str(&util::read_limited(&config.state.join(JOURNAL), 16_384)?)?;
    ensure!(
        journal.project_id == config.project_id && journal.wiki == config.wiki,
        "pending publication belongs to different paths/configuration; restore the original configuration to recover"
    );
    ensure!(
        journal.generation.len() <= 80
            && !journal.generation.is_empty()
            && journal
                .generation
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_'),
        "invalid publication generation"
    );
    let generation = &journal.generation;
    let wiki_stage = stage_wiki(config, generation)?;
    let wiki_backup = backup_wiki(config, generation)?;
    let db_stage = stage_dir(config, generation).join("state.db");
    let db_target = config.state.join("state.db");
    let db_backup = config.state.join(format!("previous-{generation}.db"));
    install_slot(&wiki_stage, &config.wiki, &wiki_backup)?;
    install_slot(&db_stage, &db_target, &db_backup)?;
    let owner = check_owner(&config.wiki, &config.project_id)?;
    ensure!(
        owner.generation == *generation,
        "wiki generation does not match journal"
    );
    {
        let conn = storage::read_only(&db_target)?;
        ensure!(
            storage::meta(&conn, "generation")?.as_deref() == Some(generation.as_str()),
            "database generation does not match journal"
        );
        validate_existing(config, &storage::pages(&conn)?, false)?;
    }
    if wiki_backup.exists() {
        check_owner(&wiki_backup, &config.project_id)?;
        fs::remove_dir_all(&wiki_backup)?;
    }
    if db_backup.exists() {
        fs::remove_file(&db_backup)?;
    }
    let stage = stage_dir(config, generation);
    if stage.exists() {
        fs::remove_dir_all(stage)?;
    }
    fs::remove_file(config.state.join(JOURNAL))?;
    util::sync_directory(&config.state)?;
    Ok(true)
}
fn install_slot(stage: &Path, target: &Path, backup: &Path) -> Result<()> {
    for path in [stage, target, backup] {
        util::reject_symlinks(path)?;
    }
    if stage.exists() {
        if target.exists() {
            ensure!(
                !backup.exists(),
                "ambiguous publication state: refusing to overwrite backup"
            );
            fs::rename(target, backup)?;
        }
        fs::rename(stage, target)?;
        util::sync_directory(target.parent().context("target has no parent")?)?;
    } else {
        ensure!(
            target.exists(),
            "both staged and installed publication are missing; retained backup requires manual recovery"
        );
    }
    Ok(())
}

pub fn commit(
    config: &ResolvedConfig,
    generation: &str,
    pages: &BTreeMap<String, StoredPage>,
) -> Result<()> {
    ensure!(
        !has_pending(config),
        "recover previous publication before committing another"
    );
    let wiki_stage = stage_wiki(config, generation)?;
    ensure!(!wiki_stage.exists(), "staging output already exists");
    util::private_dir(&wiki_stage)?;
    for page in pages.values() {
        ensure!(
            util::digest(&page.content) == page.output_digest,
            "staged page digest mismatch"
        );
        util::atomic_write(
            &page_path(&wiki_stage, &page.path)?,
            page.content.as_bytes(),
        )?;
    }
    util::atomic_write(
        &wiki_stage.join(OWNER),
        &serde_json::to_vec(&Owner {
            project_id: config.project_id.clone(),
            generation: generation.to_owned(),
        })?,
    )?;
    util::sync_directory(&wiki_stage)?;
    let db = stage_dir(config, generation).join("state.db");
    ensure!(db.is_file(), "staged database missing");
    File::open(&db)?.sync_all()?;
    let journal = Journal {
        project_id: config.project_id.clone(),
        generation: generation.to_owned(),
        wiki: config.wiki.clone(),
    };
    util::atomic_write(&config.state.join(JOURNAL), &serde_json::to_vec(&journal)?)?;
    recover(config)?;
    Ok(())
}

pub struct StageGuard<'a> {
    config: &'a ResolvedConfig,
    generation: String,
}
impl<'a> StageGuard<'a> {
    pub fn new(config: &'a ResolvedConfig, generation: &str) -> Result<Self> {
        let path = stage_dir(config, generation);
        util::private_dir(&path)?;
        Ok(Self {
            config,
            generation: generation.to_owned(),
        })
    }
}
impl Drop for StageGuard<'_> {
    fn drop(&mut self) {
        // Once a journal exists, recovery owns these files, even if a rename or
        // fsync failed. Deleting them here would destroy the roll-forward path.
        if !has_pending(self.config) {
            let _ = fs::remove_dir_all(stage_dir(self.config, &self.generation));
            if let Ok(p) = stage_wiki(self.config, &self.generation) {
                let _ = fs::remove_dir_all(p);
            }
        }
    }
}
pub fn cleanup_abandoned(config: &ResolvedConfig) -> Result<()> {
    ensure!(!has_pending(config), "cannot clean an active publication");
    let staging = config.state.join("staging");
    util::reject_symlinks(&staging)?;
    if staging.exists() {
        for entry in fs::read_dir(&staging)? {
            let entry = entry?;
            util::reject_symlinks(&entry.path())?;
            let generation = entry.file_name().to_string_lossy().to_string();
            ensure!(
                generation.starts_with("run_")
                    && generation.len() <= 80
                    && generation
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || c == b'_'),
                "unrecognized staging entry; refusing cleanup"
            );
            let sibling = stage_wiki(config, &generation)?;
            let backup = backup_wiki(config, &generation)?;
            ensure!(
                !backup.exists(),
                "orphaned publication backup requires inspection before cleanup"
            );
            util::reject_symlinks(&sibling)?;
            if sibling.exists() {
                fs::remove_dir_all(sibling)?;
            }
        }
        fs::remove_dir_all(staging)?;
    }
    Ok(())
}

/// Explicit administrative erasure, not a reconciliation operation. It removes
/// all Lore-managed local evidence, caches and generated output. No source file
/// is touched. Filesystem deletion is not a secure-erasure guarantee on SSDs or
/// external backups.
pub fn purge_all(config: &ResolvedConfig) -> Result<()> {
    let _lock = ProjectLock::acquire(config)?;
    recover(config)?;
    cleanup_abandoned(config)?;
    if config.wiki.exists() {
        check_owner(&config.wiki, &config.project_id)?;
        util::reject_symlinks(&config.wiki)?;
        fs::remove_dir_all(&config.wiki)?;
    }
    for e in fs::read_dir(&config.state)? {
        let e = e?;
        if e.file_name() == "lock" || e.file_name() == OWNER {
            continue;
        }
        util::reject_symlinks(&e.path())?;
        if e.file_type()?.is_dir() {
            fs::remove_dir_all(e.path())?;
        } else {
            fs::remove_file(e.path())?;
        }
    }
    util::sync_directory(&config.state)?;
    Ok(())
}
