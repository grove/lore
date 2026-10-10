//! Deterministic contract fixtures exercise semantic retrieval independently
//! of any embedding vendor or corpus-specific production synonym mappings.
use lore::{
    context::{
        self, ContextOptions, retrieval,
        semantic::{self, SemanticHit},
    },
    domain::{AssertionProposal, ImportKind},
    imports::{
        self, Inventory, SourceBatch,
        adapters::{AdapterRecord, ImportBatch, NativeEvidence},
    },
    inference::*,
    sources::{self, Document},
    storage, util,
};
use rusqlite::{Connection, params};
use serde_json::json;
use std::sync::{
    Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

struct Embeddings {
    descriptor: ModelDescriptor,
    identity: String,
    calls: AtomicUsize,
    inputs: Mutex<Vec<Vec<String>>>,
    malformed: AtomicBool,
    related: Vec<String>,
}

impl Embeddings {
    fn new(related: &[&str]) -> Self {
        Self {
            descriptor: ModelDescriptor {
                provider: Provider::Ollama,
                model: "synthetic-vector-fixture".into(),
                location: ExecutionLocation::Local,
            },
            identity: "fixture-endpoint-a".into(),
            calls: AtomicUsize::new(0),
            inputs: Mutex::new(Vec::new()),
            malformed: AtomicBool::new(false),
            related: related.iter().map(|text| (*text).to_owned()).collect(),
        }
    }
}

impl EmbeddingModel for Embeddings {
    fn descriptor(&self) -> &ModelDescriptor {
        &self.descriptor
    }
    fn cache_identity(&self) -> String {
        format!(
            "{}:{}",
            model_cache_identity(&self.descriptor),
            self.identity
        )
    }
    fn embed<'a>(&'a self, request: &'a EmbeddingRequest) -> ModelFuture<'a, EmbeddingResponse> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.inputs.lock().unwrap().push(request.inputs.clone());
            let embeddings = request
                .inputs
                .iter()
                .map(|input| {
                    if self.malformed.load(Ordering::SeqCst) {
                        vec![f32::NAN, 0.0, 0.0]
                    } else if self.related.iter().any(|needle| input.contains(needle)) {
                        vec![1.0, 0.0, 0.0]
                    } else {
                        vec![0.0, 1.0, 0.0]
                    }
                })
                .collect();
            Ok(EmbeddingResponse {
                usage: None,
                model: self.descriptor.model.clone(),
                embeddings,
            })
        })
    }
}

fn database() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    storage::migrate(&conn).unwrap();
    conn.execute(
        "INSERT INTO projects VALUES('p','Project','2026-10-09')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO source_roots(id,project_id,configured_path) VALUES('docs','p','./docs')",
        [],
    )
    .unwrap();
    conn
}

struct Record {
    id: String,
    assertion: String,
    evidence: String,
}

fn record(conn: &Connection, path: &str, statement: &str) -> Record {
    let document = Document {
        root_id: "docs".into(),
        root_path: "./docs".into(),
        material: Default::default(),
        origin: None,
        relative_path: path.into(),
        physical_path: path.into(),
        text: statement.into(),
        digest: util::digest(statement),
        chunks: sources::split_markdown(statement, "docs", path, 8_000).unwrap(),
    };
    let proposal = AssertionProposal {
        topic: path.trim_end_matches(".md").into(),
        topic_title: path.into(),
        subject: path.into(),
        statement: statement.into(),
        kind: "design".into(),
        lifecycle: "active".into(),
        scope: "production".into(),
        effective_at: String::new(),
        quote: statement.into(),
    };
    let (source, source_revision) = storage::begin_source(conn, &document, None).unwrap();
    let chunk = &document.chunks[0];
    let (section, section_revision) =
        storage::begin_section(conn, &source, &source_revision, chunk).unwrap();
    let (assertion, evidence) = storage::capture_assertion(
        conn,
        storage::AssertionCapture {
            document: &document,
            chunk,
            proposal: &proposal,
            source: &source,
            source_revision: &source_revision,
            section: &section,
            section_revision: &section_revision,
            model: "fixture",
        },
    )
    .unwrap();
    let id = storage::create_unit(conn, "p", &assertion, &proposal).unwrap();
    Record {
        id,
        assertion,
        evidence,
    }
}

fn native(conn: &Connection, statement: &str) -> String {
    let mut record = AdapterRecord::new("incident-7", statement, json!({"summary": statement}));
    record.evidence.push(NativeEvidence {
        locator: "memory://incident-7".into(),
        revision: Some("recorded-revision-3".into()),
        field: Some("/summary".into()),
    });
    let batch = ImportBatch {
        format: "fixture-v1".into(),
        records: vec![record],
        warnings: vec![],
    };
    let digest = util::json_digest(&batch).unwrap();
    let inventory = Inventory {
        sources: vec![SourceBatch {
            id: "memories".into(),
            kind: ImportKind::Engram,
            path: "/fixture/export.json".into(),
            batch,
            digest: digest.clone(),
        }],
        digest,
        warnings: vec![],
    };
    imports::storage::persist(conn, "p", &inventory).unwrap();
    imports::views(conn).unwrap()[0].id.clone()
}

#[tokio::test]
async fn retrieves_paraphrased_original_units_then_expands_recorded_relations() {
    let conn = database();
    let anchor = record(
        &conn,
        "protocol.md",
        "Retried operations retain one stable deduplication token.",
    );
    let dependency = record(
        &conn,
        "capacity.md",
        "Crates above nine kilograms need a second handler.",
    );
    let unrelated = record(
        &conn,
        "garden.md",
        "Mulch protects young orchard saplings during drought.",
    );
    storage::add_relation(
        &conn,
        &anchor.id,
        &dependency.id,
        "related_to",
        &anchor.assertion,
        &anchor.evidence,
    )
    .unwrap();
    storage::refresh_knowledge(&conn, "p").unwrap();
    let task = "Prevent repeat freight dispatch";
    assert!(retrieval::retrieve(&conn, task, &[]).unwrap().is_empty());
    let cache = Connection::open_in_memory().unwrap();
    let model = Embeddings::new(&[task, "stable deduplication token"]);
    let report = semantic::search(
        &conn,
        &cache,
        &model,
        EgressPolicy::LocalOnly,
        task,
        &[],
        32_000,
    )
    .await
    .unwrap();
    assert_eq!(report.knowledge.len(), 1);
    assert_eq!(report.knowledge[0].id, anchor.id);
    let result =
        retrieval::retrieve_hybrid_with_report(&conn, task, &[], &report.knowledge).unwrap();
    assert!(result.hits.iter().any(|hit| {
        hit.knowledge.id == anchor.id
            && hit
                .reasons
                .iter()
                .any(|reason| reason.starts_with("semantic relevance"))
    }));
    assert!(result.hits.iter().any(|hit| {
        hit.knowledge.id == dependency.id
            && hit
                .reasons
                .iter()
                .any(|reason| reason.contains("related_to relationship"))
    }));
    assert!(
        !result
            .hits
            .iter()
            .any(|hit| hit.knowledge.id == unrelated.id)
    );
    assert_eq!(
        conn.query_row::<i64, _, _>("SELECT count(*) FROM model_calls", [], |row| row.get(0))
            .unwrap(),
        0
    );
    assert_eq!(
        conn.query_row::<i64, _, _>(
            "SELECT count(*) FROM sqlite_master WHERE name='semantic_vectors_v1'",
            [],
            |row| row.get(0)
        )
        .unwrap(),
        0
    );
}

#[tokio::test]
async fn retrieves_source_owned_observations_with_no_lexical_seed() {
    let conn = database();
    let id = native(
        &conn,
        "A refrigeration failure spoiled the consignment before arrival.",
    );
    let task = "Improve cold cargo handling";
    let model = Embeddings::new(&[task, "refrigeration failure"]);
    let cache = Connection::open_in_memory().unwrap();
    let report = semantic::search(
        &conn,
        &cache,
        &model,
        EgressPolicy::LocalOnly,
        task,
        &[],
        32_000,
    )
    .await
    .unwrap();
    let options = ContextOptions {
        task: task.into(),
        paths: vec![],
        max_tokens: 12_000,
    };
    assert!(
        context::build_context(&conn, &options)
            .unwrap()
            .imported_observations
            .is_empty()
    );
    let result = context::build_context_with_semantic(&conn, &options, &report).unwrap();
    assert_eq!(result.imported_observations.len(), 1);
    assert_eq!(result.imported_observations[0].id, id);
    assert!(
        result.imported_observations[0]
            .relevance
            .iter()
            .any(|reason| reason.starts_with("semantic relevance"))
    );
    for evidence in result.imported_evidence {
        imports::evidence(&conn, &evidence.id).unwrap();
    }
}

#[tokio::test]
async fn caches_tasks_and_records_and_only_reembeds_changed_native_revision() {
    let conn = database();
    record(
        &conn,
        "protocol.md",
        "Each operation retains a stable deduplication token.",
    );
    storage::refresh_knowledge(&conn, "p").unwrap();
    let id = native(&conn, "An interrupted session lost a deduplication token.");
    let cache = Connection::open_in_memory().unwrap();
    let model = Embeddings::new(&["token", "Prevent repetition"]);
    let first = semantic::search(
        &conn,
        &cache,
        &model,
        EgressPolicy::LocalOnly,
        "Prevent repetition",
        &[],
        32_000,
    )
    .await
    .unwrap();
    assert_eq!(first.indexed_records, 2);
    assert_eq!(first.model_calls, 1);
    assert_eq!(first.cache_hits, 0);
    let second = semantic::search(
        &conn,
        &cache,
        &model,
        EgressPolicy::LocalOnly,
        "Prevent repetition",
        &[],
        32_000,
    )
    .await
    .unwrap();
    assert_eq!(second.model_calls, 0);
    assert_eq!(second.cache_hits, 3);
    assert_eq!(model.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        native(
            &conn,
            "A revised session preserves its deduplication token."
        ),
        id
    );
    let third = semantic::search(
        &conn,
        &cache,
        &model,
        EgressPolicy::LocalOnly,
        "Prevent repetition",
        &[],
        32_000,
    )
    .await
    .unwrap();
    assert_eq!(third.model_calls, 1);
    assert_eq!(third.cache_hits, 2);
    let inputs = model.inputs.lock().unwrap();
    assert_eq!(inputs[1].len(), 1);
    assert!(inputs[1][0].contains("revised session"));
}

#[tokio::test]
async fn full_revision_fingerprints_invalidate_changes_beyond_bounded_embedding_input() {
    let conn = database();
    let prefix = "A token remains stable. ".repeat(100);
    native(&conn, &format!("{prefix}First suffix."));
    let cache = Connection::open_in_memory().unwrap();
    let model = Embeddings::new(&["token", "Prevent repetition"]);
    let first = semantic::search(
        &conn,
        &cache,
        &model,
        EgressPolicy::LocalOnly,
        "Prevent repetition",
        &[],
        512,
    )
    .await
    .unwrap();
    assert!(first.truncated);
    native(&conn, &format!("{prefix}Changed suffix."));
    let second = semantic::search(
        &conn,
        &cache,
        &model,
        EgressPolicy::LocalOnly,
        "Prevent repetition",
        &[],
        512,
    )
    .await
    .unwrap();
    assert_eq!(second.cache_hits, 1);
    assert_eq!(second.model_calls, 1);
    let batches = model.inputs.lock().unwrap();
    assert!(
        batches
            .iter()
            .all(|batch| batch.iter().map(String::len).sum::<usize>() <= 512)
    );
    let document_inputs: Vec<_> = batches
        .iter()
        .flatten()
        .filter(|text| text.starts_with("A token"))
        .collect();
    assert_eq!(document_inputs.len(), 2);
    assert_eq!(document_inputs[0], document_inputs[1]);
}

#[tokio::test]
async fn model_or_endpoint_changes_invalidate_all_vectors() {
    let conn = database();
    native(&conn, "A token remains stable.");
    let cache = Connection::open_in_memory().unwrap();
    let mut model = Embeddings::new(&["token", "Prevent repetition"]);
    semantic::search(
        &conn,
        &cache,
        &model,
        EgressPolicy::LocalOnly,
        "Prevent repetition",
        &[],
        32_000,
    )
    .await
    .unwrap();
    model.identity = "fixture-endpoint-b".into();
    let changed_endpoint = semantic::search(
        &conn,
        &cache,
        &model,
        EgressPolicy::LocalOnly,
        "Prevent repetition",
        &[],
        32_000,
    )
    .await
    .unwrap();
    assert_eq!(changed_endpoint.cache_hits, 0);
    model.descriptor.model = "synthetic-vector-fixture-v2".into();
    let changed_model = semantic::search(
        &conn,
        &cache,
        &model,
        EgressPolicy::LocalOnly,
        "Prevent repetition",
        &[],
        32_000,
    )
    .await
    .unwrap();
    assert_eq!(changed_model.cache_hits, 0);
    assert_eq!(model.calls.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn malformed_or_dimension_mismatched_cache_rows_are_rebuilt_and_stale_ids_ignored() {
    let conn = database();
    let id = native(&conn, "A token remains stable.");
    let cache = Connection::open_in_memory().unwrap();
    let model = Embeddings::new(&["token", "Prevent repetition"]);
    semantic::search(
        &conn,
        &cache,
        &model,
        EgressPolicy::LocalOnly,
        "Prevent repetition",
        &[],
        32_000,
    )
    .await
    .unwrap();
    cache.execute("INSERT INTO semantic_vectors_v1 SELECT model_identity,record_kind,'stale-id',revision,dimensions,vector_json FROM semantic_vectors_v1 WHERE record_kind='observation'", []).unwrap();
    for corrupt in ["[0,0,0]", "[1,0]", "malformed JSON"] {
        cache
            .execute(
                "UPDATE semantic_vectors_v1 SET vector_json=?1, dimensions=?2 WHERE record_id=?3",
                params![corrupt, if corrupt == "[1,0]" { 2 } else { 3 }, id],
            )
            .unwrap();
        let report = semantic::search(
            &conn,
            &cache,
            &model,
            EgressPolicy::LocalOnly,
            "Prevent repetition",
            &[],
            32_000,
        )
        .await
        .unwrap();
        assert_eq!(report.model_calls, 1);
        assert_eq!(report.cache_hits, 1);
        assert_eq!(report.observations.len(), 1);
        assert_eq!(report.observations[0].id, id);
    }
}

#[tokio::test]
async fn hosted_policy_is_checked_even_when_all_vectors_are_cached() {
    let conn = database();
    native(&conn, "A token remains stable.");
    let cache = Connection::open_in_memory().unwrap();
    let mut model = Embeddings::new(&["token", "Prevent repetition"]);
    model.descriptor.provider = Provider::OpenAi;
    model.descriptor.location = ExecutionLocation::Hosted;
    semantic::search(
        &conn,
        &cache,
        &model,
        EgressPolicy::ExplicitHosted,
        "Prevent repetition",
        &[],
        32_000,
    )
    .await
    .unwrap();
    assert!(
        semantic::search(
            &conn,
            &cache,
            &model,
            EgressPolicy::LocalOnly,
            "Prevent repetition",
            &[],
            32_000
        )
        .await
        .is_err()
    );
    assert_eq!(model.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn changed_model_dimensions_recover_on_next_query_without_touching_other_models() {
    let conn = database();
    native(&conn, "A token remains stable.");
    let cache = Connection::open_in_memory().unwrap();
    let model = Embeddings::new(&["token", "Prevent repetition"]);
    semantic::search(
        &conn,
        &cache,
        &model,
        EgressPolicy::LocalOnly,
        "Prevent repetition",
        &[],
        32_000,
    )
    .await
    .unwrap();
    // Another explicitly configured endpoint owns an independent vector space.
    let mut other_model = Embeddings::new(&["token", "Prevent repetition"]);
    other_model.identity = "independent-endpoint".into();
    semantic::search(
        &conn,
        &cache,
        &other_model,
        EgressPolicy::LocalOnly,
        "Prevent repetition",
        &[],
        32_000,
    )
    .await
    .unwrap();
    // Simulate the first model having cached a two-dimensional query before
    // its provider upgraded the same mutable alias to three dimensions.
    cache.execute("UPDATE semantic_vectors_v1 SET dimensions=2, vector_json='[1,0]'
        WHERE record_kind='query' AND model_identity=(SELECT model_identity FROM semantic_vectors_v1 ORDER BY rowid LIMIT 1)", []).unwrap();
    native(&conn, "An updated token remains stable.");
    assert!(
        semantic::search(
            &conn,
            &cache,
            &model,
            EgressPolicy::LocalOnly,
            "Prevent repetition",
            &[],
            32_000
        )
        .await
        .is_err()
    );
    assert_eq!(
        cache
            .query_row::<i64, _, _>("SELECT count(*) FROM semantic_vectors_v1", [], |row| row
                .get(0))
            .unwrap(),
        2
    );
    let recovered = semantic::search(
        &conn,
        &cache,
        &model,
        EgressPolicy::LocalOnly,
        "Prevent repetition",
        &[],
        32_000,
    )
    .await
    .unwrap();
    assert_eq!(recovered.cache_hits, 0);
    assert_eq!(recovered.model_calls, 1);
    assert_eq!(recovered.observations.len(), 1);
    let independent = semantic::search(
        &conn,
        &cache,
        &other_model,
        EgressPolicy::LocalOnly,
        "Prevent repetition",
        &[],
        32_000,
    )
    .await
    .unwrap();
    assert_eq!(independent.cache_hits, 1); // Its task was retained; only the changed record needed updating.
}

#[tokio::test]
async fn rejects_invalid_provider_vectors_before_caching_and_never_writes_truth() {
    let conn = database();
    native(&conn, "A token remains stable.");
    let cache = Connection::open_in_memory().unwrap();
    let model = Embeddings::new(&["token", "Prevent repetition"]);
    model.malformed.store(true, Ordering::SeqCst);
    assert!(
        semantic::search(
            &conn,
            &cache,
            &model,
            EgressPolicy::LocalOnly,
            "Prevent repetition",
            &[],
            32_000
        )
        .await
        .is_err()
    );
    assert_eq!(
        cache
            .query_row::<i64, _, _>("SELECT count(*) FROM semantic_vectors_v1", [], |row| row
                .get(0))
            .unwrap(),
        0
    );
    assert!(
        semantic::search(
            &conn,
            &conn,
            &model,
            EgressPolicy::LocalOnly,
            "Prevent repetition",
            &[],
            32_000
        )
        .await
        .is_err()
    );
    assert_eq!(
        conn.query_row::<i64, _, _>(
            "SELECT count(*) FROM sqlite_master WHERE name='semantic_vectors_v1'",
            [],
            |row| row.get(0)
        )
        .unwrap(),
        0
    );
}

#[test]
fn rank_fusion_is_scale_independent_deduplicated_and_deterministic() {
    let lexical = vec![("alpha".into(), 2000.0), ("common".into(), 800.0)];
    let semantic = vec![
        SemanticHit {
            id: "bravo".into(),
            score: 0.9,
        },
        SemanticHit {
            id: "common".into(),
            score: 0.8,
        },
        SemanticHit {
            id: "common".into(),
            score: 0.2,
        },
        SemanticHit {
            id: "invalid".into(),
            score: f64::NAN,
        },
    ];
    let result = semantic::fuse_ranks(&lexical, &semantic);
    assert_eq!(result[0].0, "common");
    assert_eq!(result[1].0, "alpha");
    assert_eq!(result[2].0, "bravo");
    assert_eq!(result.len(), 3);
    let rescaled = vec![("common".into(), 0.08), ("alpha".into(), 0.2)];
    assert_eq!(result, semantic::fuse_ranks(&rescaled, &semantic));
    assert_eq!(semantic::cosine(&[2.0, 0.0], &[0.5, 0.0]), Some(1.0));
    assert_eq!(semantic::cosine(&[0.0, 0.0], &[0.5, 0.0]), None);
    assert_eq!(semantic::cosine(&[f32::INFINITY, 0.0], &[0.5, 0.0]), None);
    assert_eq!(semantic::cosine(&[1.0], &[0.5, 0.0]), None);
}

#[test]
fn empty_or_invalid_semantic_stream_preserves_fast_retrieval_exactly() {
    let conn = database();
    record(
        &conn,
        "protocol.md",
        "Redelivery retains a stable deduplication token.",
    );
    storage::refresh_knowledge(&conn, "p").unwrap();
    let fast = retrieval::retrieve_with_report(&conn, "Improve redelivery", &[]).unwrap();
    for hits in [
        vec![],
        vec![SemanticHit {
            id: "fabricated-record".into(),
            score: 0.9,
        }],
    ] {
        let hybrid =
            retrieval::retrieve_hybrid_with_report(&conn, "Improve redelivery", &[], &hits)
                .unwrap();
        assert_eq!(fast.truncated, hybrid.truncated);
        assert_eq!(fast.hits.len(), hybrid.hits.len());
        for (left, right) in fast.hits.iter().zip(&hybrid.hits) {
            assert_eq!(left.knowledge.id, right.knowledge.id);
            assert_eq!(left.score, right.score);
            assert_eq!(left.reasons, right.reasons);
        }
    }
}
