mod common;
use common::*;
use lore::{
    engine::{self, UpdateOptions},
    inference::{
        GenerationRequest, GenerationResponse, GenerativeModel, ModelDescriptor, ModelFuture,
    },
};
use serde_json::{Value, json};
use std::{
    fs,
    sync::atomic::{AtomicUsize, Ordering},
};

/// Simulate an overview writer incorrectly turning the absence of a planned
/// rollout date into a present-tense fact, despite an unrelated later report.
/// The repair must attribute the original claim to its proposal and retain
/// the separate, explicitly unverified source report.
struct TimelineModel {
    inner: FakeModel,
    repairs: AtomicUsize,
}
impl GenerativeModel for TimelineModel {
    fn descriptor(&self) -> &ModelDescriptor {
        self.inner.descriptor()
    }
    fn generate<'a>(
        &'a self,
        request: &'a GenerationRequest,
    ) -> ModelFuture<'a, GenerationResponse> {
        Box::pin(async move {
            let input: Value = serde_json::from_str(&request.input).unwrap();
            let mut answer = self.inner.generate(request).await?;
            if input["task"] == "overview" {
                if input["repair_targets"].as_array().is_some() {
                    self.repairs.fetch_add(1, Ordering::SeqCst);
                } else {
                    let mut draft: Value = serde_json::from_str(&answer.text).unwrap();
                    let kind_by_id = input["knowledge"].as_array().unwrap();
                    for paragraph in draft["sections"][0]["paragraphs"].as_array_mut().unwrap() {
                        if paragraph["knowledge_ids"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .any(|id| {
                                kind_by_id
                                    .iter()
                                    .any(|u| u["id"] == *id && u["kind"] == "proposal")
                            })
                        {
                            paragraph["text"] = json!("No date has been agreed for the rollout.");
                        }
                    }
                    answer.text = draft.to_string();
                }
            }
            Ok(answer)
        })
    }
}

#[tokio::test]
async fn proposal_absence_is_attributed_and_a_later_report_keeps_its_own_status() {
    let (_tmp, cfg, _) = project();
    let model = TimelineModel {
        inner: FakeModel::new(),
        repairs: AtomicUsize::new(0),
    };
    put(
        &cfg,
        "planning/rollout.md",
        "PLAN scheduling: The proposal said no rollout date had been agreed when it was written.\n",
    );
    put(
        &cfg,
        "operations/deployment.md",
        "REPORT scheduling: The operations team reports the rollout became active on 2026-05-03.\n",
    );
    let run = engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    assert!(!run.degraded_overview);
    assert!(run.degraded_topics.is_empty());
    assert!(model.repairs.load(Ordering::SeqCst) >= 1);
    assert!(
        run.quality_diagnostics
            .iter()
            .any(|d| d.check == "deterministic_grounding")
    );
    let overview = fs::read_to_string(cfg.wiki.join("index.md")).unwrap();
    assert!(overview.contains("The proposal said no rollout date"));
    assert!(overview.contains("operations team reports"));
    assert!(!overview.contains("No date has been agreed for the rollout."));
    let no_op = engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    assert!(no_op.no_op);
    assert_eq!(no_op.model_calls, 0);
}
