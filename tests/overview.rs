mod common;
use common::*;
use lore::{
    engine::{self, UpdateOptions},
    inference::*,
    storage,
};
use serde_json::{Value, json};
use std::fs;
#[tokio::test]
async fn overview_uses_document_ids_and_source_backed_decision_chain() {
    let (_tmp, cfg, model) = project();
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
        "review.md",
        "DECISION review: The review reaffirms ADR-001 and its MySQL choice.\n",
    );
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let before = fs::read(cfg.wiki.join("topics/review.md")).unwrap();
    put(
        &cfg,
        "ADR-027.md",
        "DECISION ledger: PostgreSQL replaces ADR-001 for the production database.\n",
    );
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let index = fs::read_to_string(cfg.wiki.join("index.md")).unwrap();
    assert!(
        index.contains(
            "[ADR-027](topics/ledger.md) explicitly supersedes [ADR-001](topics/ledger.md)"
        )
    );
    assert!(!index.contains("[ledger](topics/ledger.md) explicitly supersedes [ledger]"));
    assert!(index.contains("## Overview evidence"));
    assert!(index.contains("PostgreSQL replaces ADR-001"));
    assert!(index.contains("[Review history](reviews.md)"));
    assert_ne!(before, fs::read(cfg.wiki.join("topics/review.md")).unwrap());
    let conn = storage::read_only(&cfg.state.join("state.db")).unwrap();
    let relation = storage::relation_facts(&conn)
        .unwrap()
        .into_iter()
        .find(|f| f.kind == "supersedes")
        .unwrap();
    assert!(index.contains(&relation.evidence_id));
    let calls: i64 = conn
        .query_row(
            "SELECT count(*) FROM model_calls WHERE task='overview' AND cache_hit=0",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(calls > 0);
    drop(conn);
    assert!(
        engine::update(&cfg, &model, None, UpdateOptions::default())
            .await
            .unwrap()
            .no_op
    );
    assert_eq!(
        index,
        fs::read_to_string(cfg.wiki.join("index.md")).unwrap()
    );
}
struct BadOverview {
    model: FakeModel,
}
impl GenerativeModel for BadOverview {
    fn descriptor(&self) -> &ModelDescriptor {
        self.model.descriptor()
    }
    fn generate<'a>(&'a self, r: &'a GenerationRequest) -> ModelFuture<'a, GenerationResponse> {
        Box::pin(async move {
            let input: Value = serde_json::from_str(&r.input).unwrap();
            if input["task"] == "overview" {
                return Ok(GenerationResponse{model:"fixture".into(),text:json!({"sections":[{"heading":"Wrong","paragraphs":[{"text":"Unsupported fact.","knowledge_ids":["fabricated-unit"]}]}]}).to_string()});
            }
            self.model.generate(r).await
        })
    }
}
#[tokio::test]
async fn unsupported_overview_cannot_replace_the_previous_publication() {
    let (_tmp, cfg, model) = project();
    put(
        &cfg,
        "ADR-001.md",
        "DECISION ledger: MySQL is the selected database.\n",
    );
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let old = fs::read(cfg.wiki.join("index.md")).unwrap();
    let db = fs::read(cfg.state.join("state.db")).unwrap();
    put(&cfg, "cache.md", "DECISION caching: Use a local cache.\n");
    let bad = BadOverview {
        model: FakeModel::new(),
    };
    assert!(
        engine::update(&cfg, &bad, None, UpdateOptions::default())
            .await
            .is_err()
    );
    assert_eq!(old, fs::read(cfg.wiki.join("index.md")).unwrap());
    assert_eq!(db, fs::read(cfg.state.join("state.db")).unwrap());
}
