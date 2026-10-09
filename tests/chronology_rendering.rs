mod common;
use common::*;
use lore::{
    engine::{self, UpdateOptions},
    inference::*,
};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    fs,
    sync::atomic::{AtomicUsize, Ordering},
};

#[derive(Clone, Copy)]
enum ResponseMode {
    RepeatedRejection,
    RepairSupportedProse,
    CheckBatchScope,
}
struct ChronologyModel {
    inner: FakeModel,
    mode: ResponseMode,
    rejects: AtomicUsize,
    repairs: AtomicUsize,
    full_verifier_history: AtomicUsize,
}
impl ChronologyModel {
    fn new(mode: ResponseMode) -> Self {
        Self {
            inner: FakeModel::new(),
            mode,
            rejects: AtomicUsize::new(0),
            repairs: AtomicUsize::new(0),
            full_verifier_history: AtomicUsize::new(0),
        }
    }
}
impl GenerativeModel for ChronologyModel {
    fn descriptor(&self) -> &ModelDescriptor {
        self.inner.descriptor()
    }
    fn generate<'a>(
        &'a self,
        request: &'a GenerationRequest,
    ) -> ModelFuture<'a, GenerationResponse> {
        Box::pin(async move {
            let input: Value = serde_json::from_str(&request.input).unwrap();
            let task = input["task"].as_str().unwrap();
            let relations = input["documented_decision_relationships"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            let has_supersession = relations.iter().any(|r| r["relation"] == "supersedes");
            if task == "synthesize" {
                let allowed: BTreeSet<_> = input["knowledge"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|u| u["id"].as_str().unwrap().to_owned())
                    .collect();
                for relation in &relations {
                    assert!(allowed.contains(relation["from_id"].as_str().unwrap()));
                    assert!(allowed.contains(relation["to_id"].as_str().unwrap()));
                }
            }
            if task == "verify" && has_supersession {
                self.full_verifier_history.fetch_add(1, Ordering::SeqCst);
            }
            let mut response = self.inner.generate(request).await?;
            match self.mode {
                ResponseMode::RepeatedRejection if task == "verify" && has_supersession => {
                    self.rejects.fetch_add(1, Ordering::SeqCst);
                    response.text = json!({
                        "supported": false,
                        "issues": ["Paragraph inferred a proposal-before-ADR chronology without supporting local citations."]
                    }).to_string();
                }
                ResponseMode::RepairSupportedProse if task == "synthesize" => {
                    if input["repair_feedback"].is_null() {
                        let mut draft: Value = serde_json::from_str(&response.text).unwrap();
                        draft["sections"][0]["paragraphs"][0]["text"] =
                            json!("Before ADR-002, the entire migration plan was accepted.");
                        response.text = draft.to_string();
                    } else {
                        self.repairs.fetch_add(1, Ordering::SeqCst);
                    }
                }
                ResponseMode::RepairSupportedProse if task == "verify" => {
                    let draft = &input["draft"];
                    let bad = draft.to_string().contains("Before ADR-002");
                    if bad {
                        self.rejects.fetch_add(1, Ordering::SeqCst);
                        response.text = json!({
                            "supported": false,
                            "issues": ["The cited assertion does not establish before/after chronology."]
                        }).to_string();
                    }
                }
                _ => {}
            }
            Ok(response)
        })
    }
}
#[tokio::test]
async fn persistent_semantic_rejection_publishes_only_attributed_source_excerpts() {
    let (_temp, cfg, _) = project();
    let model = ChronologyModel::new(ResponseMode::RepeatedRejection);
    put(
        &cfg,
        "ADR-001.md",
        "DECISION ledger: MySQL is the selected database.\n",
    );
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    put(
        &cfg,
        "ADR-002.md",
        "DECISION ledger: ADR-002 replaces ADR-001; PostgreSQL is selected.\n",
    );
    let report = engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    assert_eq!(report.degraded_topics, vec!["ledger"]);
    assert_eq!(model.rejects.load(Ordering::SeqCst), 3);
    assert!(
        report
            .warnings
            .iter()
            .any(|w| w.contains("SYNTHESIS_DEGRADED"))
    );
    let topic = fs::read_to_string(cfg.wiki.join("topics/ledger.md")).unwrap();
    assert!(topic.contains("lore:degraded-topic-synthesis"));
    assert!(topic.contains("Source excerpts — synthesis requires review"));
    assert!(topic.contains("> DECISION ledger: ADR-002 replaces ADR-001; PostgreSQL is selected."));
    assert!(topic.contains("## Documented decision relationships"));
    assert!(!topic.contains("Before ADR-002, the entire migration plan was accepted."));
    let noop = engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    assert!(noop.no_op);
    assert_eq!(noop.model_calls, 0);
}
#[tokio::test]
async fn verifier_feedback_repairs_unsupported_order_without_degradation() {
    let (_temp, cfg, _) = project();
    let model = ChronologyModel::new(ResponseMode::RepairSupportedProse);
    put(
        &cfg,
        "ADR-001.md",
        "DECISION ledger: MySQL is the selected database.\n",
    );
    put(
        &cfg,
        "ADR-002.md",
        "DECISION ledger: ADR-002 replaces ADR-001; PostgreSQL is selected.\n",
    );
    let report = engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    assert!(report.degraded_topics.is_empty());
    assert!(model.rejects.load(Ordering::SeqCst) > 0);
    assert!(model.repairs.load(Ordering::SeqCst) > 0);
    let topic = fs::read_to_string(cfg.wiki.join("topics/ledger.md")).unwrap();
    assert!(!topic.contains("Before ADR-002, the entire migration plan was accepted."));
    assert!(!topic.contains("lore:degraded-topic-synthesis"));
    assert!(topic.contains("PostgreSQL"));
}
#[tokio::test]
async fn cross_topic_relationship_is_verified_but_not_citable_in_local_writing_batch() {
    let (_temp, cfg, _) = project();
    let model = ChronologyModel::new(ResponseMode::CheckBatchScope);
    put(
        &cfg,
        "ADR-001.md",
        "DECISION legacy: MySQL is the selected database.\n",
    );
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    put(
        &cfg,
        "ADR-002.md",
        "DECISION postgres: ADR-002 replaces ADR-001; PostgreSQL is the new selection.\n",
    );
    let report = engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    assert!(report.degraded_topics.is_empty());
    assert!(model.full_verifier_history.load(Ordering::SeqCst) > 0);
    let legacy = fs::read_to_string(cfg.wiki.join("topics/legacy.md")).unwrap();
    assert!(legacy.contains("## Documented decision relationships"));
    assert!(legacy.contains("explicitly supersedes"));
}
