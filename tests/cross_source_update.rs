//! First 0.4 vertical slice through real scanning, adapters, staged publication,
//! persistent reconciliation, evidence resolution, and local context retrieval.
mod common;

use lore::{
    config::{Config, ImportSource, ResolvedConfig},
    context::{self, ContextOptions},
    domain::ImportKind,
    engine::{self, UpdateOptions},
    imports::{self, relationships},
    inference::{
        GenerationRequest, GenerationResponse, GenerativeModel, ModelDescriptor, ModelFuture,
    },
    reviews, storage,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::Path,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

struct Model {
    documents: common::FakeModel,
    comparisons: AtomicUsize,
    extractions: AtomicUsize,
    invalid_quotes: AtomicBool,
}

impl Model {
    fn new() -> Self {
        Self {
            documents: common::FakeModel::new(),
            comparisons: AtomicUsize::new(0),
            extractions: AtomicUsize::new(0),
            invalid_quotes: AtomicBool::new(false),
        }
    }
}

impl GenerativeModel for Model {
    fn descriptor(&self) -> &ModelDescriptor {
        self.documents.descriptor()
    }
    fn generate<'a>(
        &'a self,
        request: &'a GenerationRequest,
    ) -> ModelFuture<'a, GenerationResponse> {
        Box::pin(async move {
            let input: Value = serde_json::from_str(&request.input).unwrap();
            if input["task"] == "extract" {
                self.extractions.fetch_add(1, Ordering::SeqCst);
            }
            if input["task"] != "cross_source_reconcile" {
                return self.documents.generate(request).await;
            }
            self.comparisons.fetch_add(1, Ordering::SeqCst);
            let left = input["left"]["statement"].as_str().unwrap();
            let right = input["right"]["statement"].as_str().unwrap();
            Ok(GenerationResponse{model:self.descriptor().model.clone(),text:json!({
                "judgment":if left==right {"consistent"} else {"potential_divergence"},
                "same_subject":true,"compatible_scope":true,"compatible_environment":true,"compatible_revision":true,
                "left_quote":if self.invalid_quotes.load(Ordering::SeqCst) {"Invented approval that appears in neither source."} else {left},
                "right_quote":right,
                "reason":"The adopted worker limit and the reported implementation limit differ at the cited implementation revision."
            }).to_string()})
        })
    }
}

fn write_claim(root: &Path, statement: &str, revision: &str) {
    let page_version = format!(
        "sha256:{:x}",
        Sha256::digest(fs::read(root.join("worker-concurrency.md")).unwrap())
    );
    fs::write(root.join(".claims/worker-concurrency.json"),serde_json::to_vec_pretty(&json!({
        "schemaVersion":1,"pageVersion":page_version,
        "verification":{"by":"fixture-code-inspector","at":"2026-10-09T12:00:00Z"},
        "claims":[{"id":"worker-limit","statement":statement,"evidence":[{"resource":"src/workers.rs#L10-L12","version":revision}]}]
    })).unwrap()).unwrap();
}

fn project() -> (tempfile::TempDir, ResolvedConfig, Model) {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("docs")).unwrap();
    let wiki = dir.path().join("openwiki");
    fs::create_dir_all(wiki.join(".claims")).unwrap();
    fs::write(
        wiki.join("index.md"),
        "---\nokf_version: \"0.2\"\n---\n# Implementation knowledge\n",
    )
    .unwrap();
    fs::write(dir.path().join("docs/ADR-001.md"),"# Worker concurrency\n\nStatus: Accepted\n\n## Decision\n\nDECISION worker-concurrency: The worker concurrency limit is three.\n").unwrap();
    fs::write(wiki.join("worker-concurrency.md"),"---\ntype: concept\ntitle: Worker concurrency\nsubject: worker-concurrency\nstatus: stable\nscope:\n  repository: project\n  component: worker\n  environment: production\n---\n# Worker concurrency\n\nImplementation details are retained in the Claims sidecar.\n").unwrap();
    write_claim(
        &wiki,
        "The worker concurrency limit is five.",
        "opaque-file-version-1",
    );
    let mut config = Config::default();
    config.schema_version = 2;
    config.project.name = "project".into();
    config.imports.push(ImportSource {
        id: "implementation".into(),
        kind: ImportKind::Openwiki,
        path: "openwiki".into(),
        project: None,
        include_memories: false,
    });
    let config_path = dir.path().join("lore.yml");
    fs::write(&config_path, serde_yaml::to_string(&config).unwrap()).unwrap();
    (
        dir,
        ResolvedConfig::load(&config_path).unwrap(),
        Model::new(),
    )
}

#[tokio::test]
async fn documented_intent_and_real_openwiki_claim_form_a_versioned_actionable_context_group() {
    let (_dir, config, model) = project();
    let first = engine::update(&config, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    assert_eq!(first.imported_records, 1);
    assert_eq!(
        first.knowledge_units, 1,
        "structured imports must not create additional documentary knowledge units"
    );
    assert_eq!(model.comparisons.load(Ordering::SeqCst), 1);
    let database = config.state.join("state.db");
    let baseline = fs::read(&database).unwrap();
    let conn = storage::read_only(&database).unwrap();
    let original = imports::views(&conn).unwrap().remove(0);
    let knowledge = storage::views(&conn).unwrap().remove(0);
    assert_eq!(knowledge.kind, "decision");
    assert_eq!(knowledge.lifecycle, "accepted");
    let options = ContextOptions {
        task: "Change worker concurrency".into(),
        paths: vec![],
        max_tokens: 20_000,
    };
    let result = context::build_context(&conn, &options).unwrap();
    assert_eq!(result.schema_version, 2);
    assert_eq!(result.model_calls, 0);
    assert_eq!(result.imported_observations.len(), 1);
    assert_eq!(result.discrepancies.len(), 1);
    let discrepancy = &result.discrepancies[0];
    assert_eq!(discrepancy.kind, "potential_discrepancy");
    assert_eq!(discrepancy.knowledge_ids, vec![knowledge.id.clone()]);
    assert_eq!(discrepancy.observation_ids, vec![original.id.clone()]);
    assert!(discrepancy.evidence_ids.contains(&original.evidence_id));
    assert!(
        discrepancy
            .evidence_ids
            .iter()
            .any(|id| result.evidence.iter().any(|e| &e.id == id))
    );
    assert!(
        result
            .recommended_verification
            .iter()
            .any(|item| item.record_ids.contains(&original.id)
                && item.record_ids.contains(&knowledge.id))
    );
    assert!(
        result
            .warnings
            .iter()
            .any(|warning| warning.contains("current checkout"))
    );
    assert!(context::render_context(&result).contains("Potential discrepancies"));
    assert_eq!(
        imports::evidence(&conn, &original.evidence_id)
            .unwrap()
            .record
            .native_record["statement"],
        "The worker concurrency limit is five."
    );
    drop(conn);
    assert_eq!(
        fs::read(&database).unwrap(),
        baseline,
        "context must remain read-only"
    );

    let calls = model.comparisons.load(Ordering::SeqCst);
    let extractions = model.extractions.load(Ordering::SeqCst);
    let repeated = engine::update(&config, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    assert!(repeated.no_op);
    assert_eq!(repeated.model_calls, 0);
    assert_eq!(fs::read(&database).unwrap(), baseline);
    assert_eq!(model.comparisons.load(Ordering::SeqCst), calls);
    assert_eq!(model.extractions.load(Ordering::SeqCst), extractions);

    write_claim(
        &config.base.join("openwiki"),
        "The worker concurrency limit is three.",
        "opaque-file-version-2",
    );
    let revised = engine::update(&config, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    assert!(!revised.no_op);
    assert_eq!(revised.changed_imported_records, 1);
    assert_eq!(
        revised.processed_sections, 0,
        "a changed native snapshot must not re-extract unchanged documentary sections"
    );
    assert_eq!(model.extractions.load(Ordering::SeqCst), extractions);
    let conn = storage::read_only(&database).unwrap();
    let current = imports::views(&conn).unwrap().remove(0);
    assert_eq!(current.id, original.id);
    assert_ne!(current.snapshot_id, original.snapshot_id);
    assert_eq!(
        storage::views(&conn).unwrap()[0].revision_id,
        knowledge.revision_id
    );
    let revised_context = context::build_context(&conn, &options).unwrap();
    assert!(
        revised_context.discrepancies.is_empty(),
        "the old mismatch must not remain a current discrepancy after its inputs change"
    );
    assert!(
        revised_context
            .cross_source_relations
            .iter()
            .any(|relation| relation.kind == "consistent_with")
    );
    assert!(
        revised_context
            .warnings
            .iter()
            .any(|warning| warning.contains("current checkout")),
        "agreement must not imply checkout verification"
    );
    assert!(
        reviews::list(&conn, false).unwrap().is_empty(),
        "a superseded automatic discrepancy question must leave the pending review queue"
    );
    let retained_review = reviews::list(&conn, true).unwrap().remove(0);
    assert_eq!(retained_review.status, "resolved");
    assert_eq!(
        reviews::events(&conn, &retained_review.id)
            .unwrap()
            .last()
            .unwrap()
            .reason_code,
        "inputs_no_longer_raise_question"
    );
    let history = relationships::history(&conn).unwrap();
    assert_eq!(history.len(), 2);
    assert!(
        history
            .iter()
            .any(|r| r.kind == "potential_discrepancy" && !r.active)
    );
    assert!(
        history
            .iter()
            .any(|r| r.kind == "consistent_with" && r.active)
    );
    let historical = imports::evidence(&conn, &original.evidence_id).unwrap();
    assert!(!historical.current);
    assert_eq!(
        historical.record.statement,
        "The worker concurrency limit is five."
    );
}

#[tokio::test]
async fn invalid_semantic_evidence_cannot_publish_changed_imports_or_relationships() {
    let (_dir, config, model) = project();
    engine::update(&config, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let database = config.state.join("state.db");
    let baseline = fs::read(&database).unwrap();
    let page = fs::read(config.wiki.join("index.md")).unwrap();
    write_claim(
        &config.base.join("openwiki"),
        "The worker concurrency limit is six.",
        "opaque-file-version-3",
    );
    model.invalid_quotes.store(true, Ordering::SeqCst);
    assert!(
        engine::update(&config, &model, None, UpdateOptions::default())
            .await
            .is_err()
    );
    assert_eq!(fs::read(&database).unwrap(), baseline);
    assert_eq!(fs::read(config.wiki.join("index.md")).unwrap(), page);
    let conn = storage::read_only(&database).unwrap();
    assert_eq!(
        imports::views(&conn).unwrap()[0].record.statement,
        "The worker concurrency limit is five."
    );
    assert_eq!(relationships::history(&conn).unwrap().len(), 1);
}
