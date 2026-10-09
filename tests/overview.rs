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

/// A real verifier can reject chronology that appears nowhere in the cited
/// knowledge. The overview must either repair it or show attributed evidence,
/// never publish rejected narrative as if it were verified.
#[derive(Clone, Copy)]
enum OverviewFailure {
    AlwaysUnsupported,
    FirstDraftUnsupported,
}
struct SemanticOverview {
    delegate: FakeModel,
    failure: OverviewFailure,
}
impl SemanticOverview {
    fn new(failure: OverviewFailure) -> Self {
        Self {
            delegate: FakeModel::new(),
            failure,
        }
    }
}
impl GenerativeModel for SemanticOverview {
    fn descriptor(&self) -> &ModelDescriptor {
        self.delegate.descriptor()
    }
    fn generate<'a>(&'a self, req: &'a GenerationRequest) -> ModelFuture<'a, GenerationResponse> {
        Box::pin(async move {
            let input: Value = serde_json::from_str(&req.input).unwrap();
            let task = input["task"].as_str().unwrap();
            if task == "verify_overview" {
                let objection = match self.failure {
                    OverviewFailure::AlwaysUnsupported => true,
                    OverviewFailure::FirstDraftUnsupported => input["draft"]
                        .to_string()
                        .contains("unjustified publication chronology"),
                };
                if objection {
                    return Ok(GenerationResponse {
                        model: "fixture".into(),
                        text:json!({"supported":false,"issues":[
                            "The cited ADR identifies a date but not an established publication date.",
                            "The migration question was broadened beyond its cited ledger-specific scope."
                        ]}).to_string()
                    });
                }
            }
            let mut output = self.delegate.generate(req).await?;
            if task == "overview"
                && matches!(self.failure, OverviewFailure::FirstDraftUnsupported)
                && input.get("repair_feedback").is_none()
            {
                let mut draft: Value = serde_json::from_str(&output.text).unwrap();
                draft["sections"][0]["paragraphs"][0]["text"] =
                    json!("An unjustified publication chronology was inferred from the ADR.");
                output.text = draft.to_string();
            }
            Ok(output)
        })
    }
}
#[tokio::test]
async fn repeated_overview_verifier_rejections_produce_an_explicit_evidence_index() {
    let (_tmp, cfg, _) = project();
    put(
        &cfg,
        "ADR-001.md",
        "DECISION ledger: MySQL is the selected database.\n",
    );
    put(
        &cfg,
        "review.md",
        "DECISION review: The review reaffirms ADR-001 and its database choice.\n",
    );
    let model = SemanticOverview::new(OverviewFailure::AlwaysUnsupported);
    let report = engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    assert!(report.degraded_overview);
    assert!(
        report
            .warnings
            .iter()
            .any(|w| w.contains("OVERVIEW_DEGRADED"))
    );
    let index = fs::read_to_string(cfg.wiki.join("index.md")).unwrap();
    assert!(index.contains("lore:degraded-overview-synthesis"));
    assert!(index.contains("Source excerpts — overview requires review"));
    assert!(index.contains("> DECISION ledger: MySQL is the selected database."));
    assert!(index.contains("## Overview evidence"));
    assert!(index.contains("## Explore the project"));
    assert!(index.contains("Source:"));
    assert!(!index.contains("unjustified publication chronology"));
    assert_eq!(engine::audit(&cfg).unwrap()["ok"], true);
    let noop = engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    assert!(noop.no_op);
    assert!(noop.degraded_overview);
    assert_eq!(noop.model_calls, 0);
}
#[tokio::test]
async fn supported_overview_repair_publishes_no_degradation_marker() {
    let (_tmp, cfg, _) = project();
    put(
        &cfg,
        "ADR-001.md",
        "DECISION ledger: MySQL is the selected database.\n",
    );
    let model = SemanticOverview::new(OverviewFailure::FirstDraftUnsupported);
    let report = engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    assert!(!report.degraded_overview);
    let index = fs::read_to_string(cfg.wiki.join("index.md")).unwrap();
    assert!(!index.contains("lore:degraded-overview-synthesis"));
    assert!(!index.contains("unjustified publication chronology"));
    assert!(index.contains("MySQL is the selected database"));
    assert!(index.contains("## Overview evidence"));
}
