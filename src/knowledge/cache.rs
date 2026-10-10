use super::*;
use anyhow::{Context, ensure};
use std::{fs, path::Path};

const OWNER_FILE: &str = ".lore-knowledge-zoom-owner";
const OWNER: &str = "lore-knowledge-zoom-cache-v1\n";
const SNAPSHOT: &str = "snapshot.json";
const MAX_CACHE_BYTES: u64 = 128 * 1024 * 1024;
const INDEX_VERSION: u32 = 1;

/// Work performed by this invocation. Generation and validation both charge
/// canonical topology planning; source loading, summary planning and integrity
/// checks are still required even when no view objects or text are regenerated.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RefreshWork {
    pub topology_reused: bool,
    pub layout_generation_work: usize,
    pub topology_validation_work: usize,
    pub summaries_generated: usize,
    pub summaries_validated: usize,
    pub dependency_revisions_generated: usize,
    pub dependency_revisions_validated: usize,
    pub records_revalidated: usize,
    pub evidence_revalidated: usize,
    pub fallback_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheUpdate {
    pub graph: KnowledgeGraph,
    pub reused_nodes: usize,
    pub regenerated_nodes: usize,
    pub removed_nodes: usize,
    pub wrote_snapshot: bool,
    pub refresh: RefreshWork,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct IncrementalIndex {
    version: u32,
    grouping_signature: String,
    record_dependencies: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Snapshot {
    #[serde(flatten)]
    graph: KnowledgeGraph,
    incremental: IncrementalIndex,
    content_digest: String,
}

impl Snapshot {
    fn digest(&self) -> Result<String> {
        // A corruption checksum, not authentication or source authority. Reuse
        // additionally validates membership and exact summary plans against
        // fresh originals, even if someone recomputes this checksum.
        util::json_digest(&serde_json::json!({
            "graph": self.graph,
            "incremental": self.incremental,
        }))
    }
}

/// Re-read all authoritative inputs, then reuse a validated stable topology.
/// Only nodes with changed complete source dependencies rebuild their summaries
/// and revision values. Structural changes or invalid cached content rebuild
/// the graph. No-op reads still perform source and cached-output validation.
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
        .and_then(|bytes| serde_json::from_slice::<Snapshot>(bytes).ok());
    let (graph, refresh, incremental) = with_inputs(conn, options, |mut graph| {
        let signature = graph::grouping_signature(&graph)?;
        let dependencies = graph::Dependencies::new(&graph)?;
        let initial_report = graph.report.clone();
        let mut refresh = RefreshWork {
            records_revalidated: graph.knowledge.len(),
            evidence_revalidated: graph.evidence.len(),
            ..Default::default()
        };
        let mut layout = None;
        let reused = previous.as_ref().map(|previous| {
            refresh_stable(
                &mut graph,
                previous,
                &signature,
                &dependencies,
                &mut refresh,
                &mut layout,
            )
        });
        match reused {
            Some(Ok(())) => {}
            failure => {
                let reason = match failure {
                    Some(Err(error)) => error.to_string(),
                    _ => "missing or incompatible incremental cache".into(),
                };
                graph.nodes.clear();
                graph.edges.clear();
                graph.report = initial_report;
                // Canonical validation may already have produced this plan.
                // Reuse it on failure so the bounded grouping work happens
                // once, even when a later summary check rejects the cache.
                let layout = layout.unwrap_or_else(|| {
                    let layout = graph::derive_layout(&graph);
                    refresh.layout_generation_work += layout.report.work_used;
                    layout
                });
                graph::assemble_from_layout(&mut graph, layout, &dependencies)?;
                graph::validate_with_dependencies(&graph, &dependencies)?;
                refresh.topology_reused = false;
                refresh.summaries_generated += graph.nodes.len();
                refresh.dependency_revisions_generated += graph.nodes.len();
                refresh.dependency_revisions_validated += graph.nodes.len();
                refresh.fallback_reason = Some(reason);
            }
        };
        let incremental = IncrementalIndex {
            version: INDEX_VERSION,
            grouping_signature: signature,
            record_dependencies: dependencies.records,
        };
        Ok((graph, refresh, incremental))
    })?;
    let old_ids = previous.as_ref().map_or_else(BTreeSet::new, |old| {
        old.graph
            .nodes
            .iter()
            .map(|node| node.id.as_str())
            .collect()
    });
    let current_ids: BTreeSet<_> = graph.nodes.iter().map(|n| n.id.as_str()).collect();
    let removed = old_ids.difference(&current_ids).count();
    let mut snapshot = Snapshot {
        graph,
        incremental,
        content_digest: String::new(),
    };
    snapshot.content_digest = snapshot.digest()?;
    let bytes = serde_json::to_vec_pretty(&snapshot)?;
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
    let regenerated = if refresh.topology_reused {
        refresh.summaries_generated
    } else {
        snapshot.graph.nodes.len()
    };
    Ok(CacheUpdate {
        regenerated_nodes: regenerated,
        reused_nodes: snapshot.graph.nodes.len() - regenerated,
        graph: snapshot.graph,
        removed_nodes: removed,
        wrote_snapshot: wrote,
        refresh,
    })
}

fn refresh_stable(
    graph: &mut KnowledgeGraph,
    previous: &Snapshot,
    signature: &str,
    dependencies: &graph::Dependencies,
    refresh: &mut RefreshWork,
    layout: &mut Option<graph::Layout>,
) -> Result<()> {
    ensure!(
        previous.incremental.version == INDEX_VERSION
            && previous.graph.schema_version == ZOOM_SCHEMA_VERSION
            && previous.graph.grouping_version == GROUPING_VERSION
            && previous.content_digest == previous.digest()?,
        "incompatible or corrupted incremental cache"
    );
    ensure!(
        previous.incremental.grouping_signature == signature
            && graph::grouping_signature(&previous.graph)? == signature,
        "grouping inputs or resource options changed"
    );
    ensure!(
        previous
            .incremental
            .record_dependencies
            .keys()
            .eq(dependencies.records.keys())
            && previous.graph.nodes.len() <= graph.options.max_nodes
            && previous.graph.edges.len() <= graph.options.max_edges,
        "cached topology or dependency index has incompatible structure"
    );
    let canonical = graph::derive_layout(graph);
    refresh.topology_validation_work += canonical.report.work_used;
    *layout = Some(canonical);
    let canonical = layout.as_ref().expect("current source plan was retained");
    graph.nodes = previous.graph.nodes.clone();
    graph.edges = previous.graph.edges.clone();
    graph::validate_reusable_topology(graph, &previous.graph.report, canonical)?;
    graph.report = canonical.report.clone();
    graph::validate_structure(graph)?;
    // Separate the mutable derived nodes from their immutable source inputs.
    let mut nodes = std::mem::take(&mut graph.nodes);
    let summaries = graph::Summaries::new(graph);
    for node in &mut nodes {
        let changed = node.knowledge_ids.iter().any(|id| {
            previous.incremental.record_dependencies.get(id) != dependencies.records.get(id)
        });
        if changed {
            graph::refresh_node(node, &summaries, dependencies)?;
            refresh.summaries_generated += 1;
            refresh.dependency_revisions_generated += 1;
        } else {
            // Neither a cached fingerprint nor a valid checksum establishes
            // that prose was generated from these originals. Check the exact
            // source plan, including all required critical records, directly.
            refresh.summaries_validated += 1;
            summaries.validate(node)?;
        }
        refresh.dependency_revisions_validated += 1;
        ensure!(
            node.revision == dependencies.revision(node)?,
            "view dependency fingerprint does not match its inputs"
        );
    }
    graph.nodes = nodes;
    refresh.topology_reused = true;
    Ok(())
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
