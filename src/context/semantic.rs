//! Optional semantic retrieval over original registry records.
//!
//! SQLite stores a disposable vector cache, separate from accepted knowledge.
//! Cosine search runs in process and rank fusion feeds the existing bounded
//! relationship expansion. There is no additional service or vector extension.

use crate::{
    domain::KnowledgeView,
    imports,
    inference::{
        EgressPolicy, EmbeddingModel, EmbeddingRequest, MAX_EMBEDDING_INPUTS, Provider,
        valid_embedding,
    },
    storage, util,
};
use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path, time::Duration};

const INDEX_VERSION: &str = "original-record-embeddings-v1";
const MAX_RECORD_BYTES: usize = 8_000;
const MAX_INDEX_RECORDS: usize = 20_000;
const MAX_SEMANTIC_HITS: usize = 48;
const RRF_K: f64 = 60.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticHit {
    pub id: String,
    /// Cosine similarity is a retrieval signal, never factual confidence.
    pub score: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SemanticReport {
    pub knowledge: Vec<SemanticHit>,
    pub observations: Vec<SemanticHit>,
    pub model_calls: u32,
    /// Cached vectors reused, including the task vector.
    pub cache_hits: usize,
    pub indexed_records: usize,
    pub truncated: bool,
    pub warnings: Vec<String>,
}

struct Record {
    kind: &'static str,
    id: String,
    revision: String,
    input: String,
}

/// The caller may instead use an in-memory Connection for --no-cache or when
/// a disposable cache cannot be opened. Never migrate the truth registry here.
pub fn open_cache(path: &Path) -> Result<Connection> {
    util::reject_symlinks(path)?;
    let parent = path
        .parent()
        .context("semantic cache needs a parent directory")?;
    util::private_dir(parent)?;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    match options.open(path) {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error).context("create semantic cache"),
    }
    let cache = Connection::open(path).context("open semantic cache")?;
    cache.busy_timeout(Duration::from_secs(5))?;
    initialize(&cache)?;
    Ok(cache)
}

fn initialize(cache: &Connection) -> Result<()> {
    let truth_registry: bool = cache.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='knowledge_units')",
        [],
        |row| row.get(0),
    )?;
    ensure!(
        !truth_registry,
        "semantic caches must be separate from the knowledge registry"
    );
    cache.execute_batch(
        "CREATE TABLE IF NOT EXISTS semantic_vectors_v1(
            model_identity TEXT NOT NULL,
            record_kind TEXT NOT NULL,
            record_id TEXT NOT NULL,
            revision TEXT NOT NULL,
            dimensions INTEGER NOT NULL,
            vector_json TEXT NOT NULL,
            PRIMARY KEY(model_identity,record_kind,record_id)
         );",
    )?;
    Ok(())
}

fn bounded_text(mut text: String, max_bytes: usize, truncated: &mut bool) -> String {
    if text.len() > max_bytes {
        let mut end = max_bytes;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
        *truncated = true;
    }
    text
}

fn knowledge_text(view: &KnowledgeView) -> String {
    format!(
        "{}\nTopic: {}\nSubject: {}\nScope: {}\nKind: {}\nLifecycle: {}",
        view.statement, view.topic_title, view.subject, view.scope, view.kind, view.lifecycle
    )
}

fn records(
    conn: &Connection,
    max_bytes: usize,
    report: &mut SemanticReport,
) -> Result<Vec<Record>> {
    let mut records = Vec::new();
    for view in storage::views(conn)? {
        if view.support_state == "unsupported" || view.evidence.is_empty() {
            continue;
        }
        // Search provenance is original, retained evidence, not wiki text.
        // Do not transmit a damaged registry's unsupported text to a model.
        for evidence in &view.evidence {
            let snapshot = storage::evidence_snapshot(conn, &evidence.id)?;
            ensure!(
                !snapshot.excerpt.is_empty() && util::digest(&snapshot.excerpt) == snapshot.digest,
                "semantic retrieval found an invalid documentary evidence snapshot"
            );
        }
        let revision = util::json_digest(&(INDEX_VERSION, max_bytes, &view))?;
        let input = bounded_text(knowledge_text(&view), max_bytes, &mut report.truncated);
        records.push(Record {
            kind: "knowledge",
            id: view.id,
            revision,
            input,
        });
    }
    for observation in imports::views(conn)? {
        imports::evidence(conn, &observation.evidence_id)?;
        let record = &observation.record;
        // Full snapshot fingerprints include revisions/metadata beyond the
        // bounded text; a changed suffix cannot accidentally reuse a vector.
        let revision = util::json_digest(&(INDEX_VERSION, max_bytes, &observation))?;
        let text = format!(
            "{}\nTitle: {}\nSubject: {}\nScope: {}\nTags: {}\nLifecycle: {}\nKind: {:?}",
            record.statement,
            record.title,
            record.subject,
            serde_json::to_string(&record.scope)?,
            record.tags.join(" "),
            record.lifecycle,
            record.kind
        );
        let input = bounded_text(text, max_bytes, &mut report.truncated);
        records.push(Record {
            kind: "observation",
            id: observation.id,
            revision,
            input,
        });
    }
    records.sort_by(|a, b| (&a.kind, &a.id).cmp(&(&b.kind, &b.id)));
    if records.len() > MAX_INDEX_RECORDS {
        records.truncate(MAX_INDEX_RECORDS);
        report.truncated = true;
        report.warnings.push(format!(
            "Semantic indexing is limited to {MAX_INDEX_RECORDS} original records; lexical retrieval remains available for the full registry."
        ));
    }
    Ok(records)
}

fn cached_vector(cache: &Connection, identity: &str, record: &Record) -> Result<Option<Vec<f32>>> {
    let cached: Option<(i64, String)> = cache
        .query_row(
            "SELECT dimensions,vector_json FROM semantic_vectors_v1
         WHERE model_identity=?1 AND record_kind=?2 AND record_id=?3 AND revision=?4",
            params![identity, record.kind, record.id, record.revision],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    Ok(cached.and_then(|(dimensions, raw)| {
        let vector: Vec<f32> = serde_json::from_str(&raw).ok()?;
        (dimensions == vector.len() as i64 && valid_embedding(&vector)).then_some(vector)
    }))
}

fn save_vector(cache: &Connection, identity: &str, record: &Record, vector: &[f32]) -> Result<()> {
    cache.execute(
        "INSERT INTO semantic_vectors_v1 VALUES(?1,?2,?3,?4,?5,?6)
         ON CONFLICT(model_identity,record_kind,record_id) DO UPDATE SET
         revision=excluded.revision,dimensions=excluded.dimensions,vector_json=excluded.vector_json",
        params![identity,record.kind,record.id,record.revision,vector.len() as i64,serde_json::to_string(vector)?],
    )?;
    Ok(())
}

/// Incrementally embed original registry units plus the task, then search the
/// valid vectors. The exact model identity includes the configured endpoint.
/// The egress gate precedes even cache reads, so prior hosted results cannot
/// bypass a subsequently enabled local-only policy.
pub async fn search(
    registry: &Connection,
    cache: &Connection,
    model: &dyn EmbeddingModel,
    policy: EgressPolicy,
    task: &str,
    paths: &[String],
    max_input_bytes: usize,
) -> Result<SemanticReport> {
    policy.authorize(model.descriptor()).map_err(|_| {
        anyhow::anyhow!("semantic retrieval is disabled by the configured local-only policy")
    })?;
    if matches!(policy, EgressPolicy::LocalOnly) {
        ensure!(
            model.descriptor().provider == Provider::Ollama
                && !model.descriptor().model.contains(":cloud")
                && !model.descriptor().model.ends_with("-cloud"),
            "semantic retrieval is disabled by the configured local-only policy"
        );
    }
    ensure!(
        (256..=1_000_000).contains(&max_input_bytes),
        "invalid embedding input budget"
    );
    initialize(cache)?;
    let identity = util::digest(format!("{INDEX_VERSION}:{}", model.cache_identity()));
    let mut report = SemanticReport::default();
    let max_record_bytes = MAX_RECORD_BYTES.min(max_input_bytes);
    let mut records = records(registry, max_record_bytes, &mut report)?;
    if records.is_empty() || task.trim().is_empty() && paths.is_empty() {
        return Ok(report);
    }
    let mut paths = paths
        .iter()
        .map(|p| p.trim().replace('\\', "/"))
        .collect::<Vec<_>>();
    paths.sort();
    paths.dedup();
    let full_query = if paths.is_empty() {
        task.trim().to_owned()
    } else {
        format!("{}\nPaths: {}", task.trim(), paths.join(", "))
    };
    let query_revision = util::json_digest(&(INDEX_VERSION, max_record_bytes, &full_query))?;
    let query = Record {
        kind: "query",
        id: query_revision.clone(),
        revision: query_revision,
        input: bounded_text(full_query, max_record_bytes, &mut report.truncated),
    };
    // Put the task first so its dimensions establish this search's vector space.
    records.insert(0, query);
    let mut vectors = Vec::with_capacity(records.len());
    for record in &records {
        let cached = cached_vector(cache, &identity, record)?;
        report.cache_hits += usize::from(cached.is_some());
        vectors.push(cached);
    }
    // A corrupted or outdated row with different dimensions is regenerated.
    // Dimensions changing under the same model ID never mix vector spaces.
    if let Some(dimensions) = vectors[0].as_ref().map(Vec::len) {
        for vector in vectors.iter_mut().skip(1) {
            if vector
                .as_ref()
                .is_some_and(|value| value.len() != dimensions)
            {
                *vector = None;
                report.cache_hits -= 1;
            }
        }
    }
    let mut pending: Vec<usize> = vectors
        .iter()
        .enumerate()
        .filter_map(|(index, vector)| vector.is_none().then_some(index))
        .collect();
    let mut position = 0;
    let mut dimensions = vectors[0].as_ref().map(Vec::len);
    while position < pending.len() {
        let start = position;
        let mut bytes = 0;
        while position < pending.len() && position - start < MAX_EMBEDDING_INPUTS {
            let next = records[pending[position]].input.len();
            if position > start && bytes + next > max_input_bytes {
                break;
            }
            bytes += next;
            position += 1;
        }
        let batch: Vec<usize> = pending[start..position].to_vec();
        let request = EmbeddingRequest {
            inputs: batch
                .iter()
                .map(|index| records[*index].input.clone())
                .collect(),
        };
        request
            .validate(max_input_bytes)
            .map_err(|error| anyhow::anyhow!("{error:?}"))?;
        policy.authorize(model.descriptor()).map_err(|_| {
            anyhow::anyhow!("semantic retrieval is disabled by the configured local-only policy")
        })?;
        report.model_calls += 1;
        let response = model
            .embed(&request)
            .await
            .map_err(|error| anyhow::anyhow!("embedding inference failed: {error:?}"))?;
        let returned_dimensions = response
            .validate(&request, model.descriptor())
            .map_err(|error| anyhow::anyhow!("{error:?}"))?;
        if let Some(expected) = dimensions {
            if expected != returned_dimensions {
                // A provider may update a mutable model alias in place. A
                // cached task vector must not permanently strand that task
                // in the old dimensional space. Only this exact provider /
                // endpoint / model cache is invalidated; the next query can
                // rebuild coherently, including its task vector.
                cache.execute(
                    "DELETE FROM semantic_vectors_v1 WHERE model_identity=?1",
                    [&identity],
                )?;
                anyhow::bail!(
                    "embedding model dimensions changed; invalidated this model's semantic cache for the next query"
                );
            }
        } else {
            dimensions = Some(returned_dimensions);
            // The query was regenerated and defines a new dimensional space.
            // Re-embed any previously cached incompatible original records.
            for (index, vector) in vectors.iter_mut().enumerate() {
                if vector
                    .as_ref()
                    .is_some_and(|v| v.len() != returned_dimensions)
                {
                    *vector = None;
                    report.cache_hits -= 1;
                    if !pending.contains(&index) {
                        pending.push(index);
                    }
                }
            }
        }
        // Validation is complete before this batch can publish cache rows.
        let transaction = cache.unchecked_transaction()?;
        for (index, vector) in batch.iter().zip(response.embeddings) {
            save_vector(&transaction, &identity, &records[*index], &vector)?;
            vectors[*index] = Some(vector);
        }
        transaction.commit()?;
    }
    let query = vectors[0].as_ref().context("query embedding unavailable")?;
    for (record, vector) in records.iter().zip(&vectors).skip(1) {
        let vector = vector.as_ref().context("record embedding unavailable")?;
        let score = cosine(query, vector).context("incompatible cached embedding dimensions")?;
        report.indexed_records += 1;
        // Zero/negative alignment supplies no positive retrieval signal.
        if score <= 0.0 {
            continue;
        }
        let hits = if record.kind == "knowledge" {
            &mut report.knowledge
        } else {
            &mut report.observations
        };
        hits.push(SemanticHit {
            id: record.id.clone(),
            score,
        });
    }
    for hits in [&mut report.knowledge, &mut report.observations] {
        hits.sort_by(|a, b| b.score.total_cmp(&a.score).then_with(|| a.id.cmp(&b.id)));
        if hits.len() > MAX_SEMANTIC_HITS {
            report.truncated = true;
            hits.truncate(MAX_SEMANTIC_HITS);
        }
    }
    if report.truncated {
        report.warnings.push(
            "Semantic search used bounded record/query text or candidate limits; the retrieved context is not exhaustive.".into()
        );
    }
    Ok(report)
}

pub fn cosine(left: &[f32], right: &[f32]) -> Option<f64> {
    if left.len() != right.len() || !valid_embedding(left) || !valid_embedding(right) {
        return None;
    }
    let dot: f64 = left
        .iter()
        .zip(right)
        .map(|(a, b)| f64::from(*a) * f64::from(*b))
        .sum();
    let length = |vector: &[f32]| {
        vector
            .iter()
            .map(|value| f64::from(*value).powi(2))
            .sum::<f64>()
            .sqrt()
    };
    Some((dot / (length(left) * length(right))).clamp(-1.0, 1.0))
}

/// Reciprocal rank fusion avoids comparing unrelated BM25/cosine scales.
/// Independent rank order and stable identifiers settle all score ties.
pub fn fuse_ranks(lexical: &[(String, f64)], semantic: &[SemanticHit]) -> Vec<(String, f64)> {
    let mut fused = BTreeMap::<String, f64>::new();
    for stream in [
        lexical.to_vec(),
        semantic
            .iter()
            .map(|hit| (hit.id.clone(), hit.score))
            .collect(),
    ] {
        let mut dedup = BTreeMap::<String, f64>::new();
        for (id, score) in stream {
            if score.is_finite() && score > 0.0 && !id.is_empty() {
                dedup
                    .entry(id)
                    .and_modify(|value| *value = value.max(score))
                    .or_insert(score);
            }
        }
        let mut ranked: Vec<_> = dedup.into_iter().collect();
        ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        for (rank, (id, _)) in ranked.into_iter().enumerate() {
            *fused.entry(id).or_default() += 1.0 / (RRF_K + rank as f64 + 1.0);
        }
    }
    let maximum = 2.0 / (RRF_K + 1.0);
    let mut ranked: Vec<_> = fused
        .into_iter()
        .map(|(id, score)| (id, score / maximum))
        .collect();
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    ranked
}
