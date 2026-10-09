use super::*;
use anyhow::{Context, ensure};
use std::{fs, path::Path};

const OWNER_FILE: &str = ".lore-knowledge-zoom-owner";
const OWNER: &str = "lore-knowledge-zoom-cache-v1\n";
const SNAPSHOT: &str = "snapshot.json";
const MAX_CACHE_BYTES: u64 = 128 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheUpdate {
    pub graph: KnowledgeGraph,
    pub reused_nodes: usize,
    pub regenerated_nodes: usize,
    pub removed_nodes: usize,
    pub wrote_snapshot: bool,
}

/// Persist only a disposable derived publication. Current registry evidence is
/// re-read and validated even on a cache hit; old prose is never trusted merely
/// because an attacker retained a fingerprint in the JSON cache.
///
/// Grouping is cheap deterministic work. Unaffected nodes retain byte-identical
/// content/revisions; a genuinely unchanged snapshot does not rewrite its file.
/// This is a correctness/reuse contract, not a claim of measured speed benefit.
pub fn build_cached(
    conn: &Connection,
    options: &ZoomOptions,
    directory: &Path,
) -> Result<CacheUpdate> {
    prepare_directory(directory)?;
    let path = directory.join(SNAPSHOT);
    util::reject_symlinks(&path)?;
    let previous_bytes = match fs::metadata(&path) {
        Ok(metadata) if metadata.is_file() && metadata.len() <= MAX_CACHE_BYTES => {
            util::read_limited(&path, MAX_CACHE_BYTES as usize)
                .ok()
                .map(String::into_bytes)
        }
        Ok(_) => None,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    let previous = previous_bytes
        .as_deref()
        .and_then(|bytes| serde_json::from_slice::<KnowledgeGraph>(bytes).ok());
    let mut graph = build(conn, options)?;
    let old_nodes: BTreeMap<_, _> = previous
        .as_ref()
        .filter(|old| {
            old.schema_version == ZOOM_SCHEMA_VERSION && old.grouping_version == GROUPING_VERSION
        })
        .map(|old| old.nodes.iter().map(|n| (n.id.as_str(), n)).collect())
        .unwrap_or_default();
    let mut reused = 0;
    for node in &mut graph.nodes {
        if let Some(previous) = old_nodes.get(node.id.as_str()) {
            // Equality checks the complete newly verified node, not just a
            // caller-supplied revision token. Poisoned cache summaries miss.
            if *previous == node {
                *node = (*previous).clone();
                reused += 1;
            }
        }
    }
    let current_ids: BTreeSet<_> = graph.nodes.iter().map(|n| n.id.as_str()).collect();
    let removed = old_nodes
        .keys()
        .filter(|id| !current_ids.contains(**id))
        .count();
    let bytes = serde_json::to_vec_pretty(&graph)?;
    ensure!(
        bytes.len() as u64 <= MAX_CACHE_BYTES,
        "derived Knowledge Zoom cache exceeds its byte budget"
    );
    let wrote = previous_bytes.as_deref() != Some(bytes.as_slice());
    if wrote {
        // The complete publication replaces the old file atomically. An
        // interrupted/missing disposable cache is rebuilt from the registry.
        util::atomic_write(&path, &bytes)?;
    }
    Ok(CacheUpdate {
        regenerated_nodes: graph.nodes.len() - reused,
        graph,
        reused_nodes: reused,
        removed_nodes: removed,
        wrote_snapshot: wrote,
    })
}

fn prepare_directory(directory: &Path) -> Result<()> {
    util::reject_symlinks(directory)?;
    let marker = directory.join(OWNER_FILE);
    util::reject_symlinks(&marker)?;
    if directory.exists() {
        ensure!(
            directory.is_dir(),
            "Knowledge Zoom cache path is not a directory"
        );
        if !marker.exists() {
            ensure!(
                fs::read_dir(directory)?.next().is_none(),
                "refusing an unmanaged nonempty Knowledge Zoom cache directory"
            );
        }
    }
    util::private_dir(directory)?;
    if marker.exists() {
        ensure!(
            util::read_limited(&marker, OWNER.len() + 1)? == OWNER,
            "unrecognized Knowledge Zoom cache ownership"
        );
    } else {
        util::atomic_write(&marker, OWNER.as_bytes())?;
    }
    Ok(())
}

/// Remove this feature's disposable state without changing source knowledge.
pub fn purge_cache(directory: &Path) -> Result<()> {
    util::reject_symlinks(directory)?;
    if !directory.exists() {
        return Ok(());
    }
    let marker = directory.join(OWNER_FILE);
    util::reject_symlinks(&marker)?;
    ensure!(
        util::read_limited(&marker, OWNER.len() + 1)
            .context("cache has no valid ownership marker")?
            == OWNER,
        "unrecognized Knowledge Zoom cache ownership"
    );
    let mut entries = Vec::with_capacity(2);
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        ensure!(
            entry.file_name() == OWNER_FILE || entry.file_name() == SNAPSHOT,
            "refusing to purge unmanaged files from the Knowledge Zoom cache"
        );
        util::reject_symlinks(&entry.path())?;
        ensure!(entry.file_type()?.is_file(), "unexpected cache entry type");
        ensure!(entries.len() < 2, "unexpected duplicate cache entries");
        entries.push(entry);
    }
    for entry in entries {
        fs::remove_file(entry.path())?;
    }
    fs::remove_dir(directory)?;
    Ok(())
}
