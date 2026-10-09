mod common;
use common::*;
use lore::{
    engine::{self, UpdateOptions},
    inference::{
        GenerationRequest, GenerationResponse, GenerativeModel, ModelDescriptor, ModelFuture,
    },
    storage,
};
use serde_json::{Value, json};
use std::{
    fs,
    sync::atomic::{AtomicUsize, Ordering},
};

/// Two independent topics cite one committee document. A topic may say what
/// its own units establish, but must not claim the rest of that source is empty.
struct CrossTopicModel {
    inner: FakeModel,
    verifier_rejections: AtomicUsize,
    source_context_checks: AtomicUsize,
    dated_heading_checks: AtomicUsize,
    rewrites: AtomicUsize,
    case_syntheses: AtomicUsize,
}
impl CrossTopicModel {
    fn new() -> Self {
        Self {
            inner: FakeModel::new(),
            verifier_rejections: AtomicUsize::new(0),
            source_context_checks: AtomicUsize::new(0),
            dated_heading_checks: AtomicUsize::new(0),
            rewrites: AtomicUsize::new(0),
            case_syntheses: AtomicUsize::new(0),
        }
    }
}
impl GenerativeModel for CrossTopicModel {
    fn descriptor(&self) -> &ModelDescriptor {
        self.inner.descriptor()
    }
    fn generate<'a>(&'a self, r: &'a GenerationRequest) -> ModelFuture<'a, GenerationResponse> {
        Box::pin(async move {
            let input: Value = serde_json::from_str(&r.input).unwrap();
            let task = input["task"].as_str().unwrap();
            let topic = input["topic"].as_str().unwrap_or("");
            if matches!(
                task,
                "synthesize" | "overview" | "verify" | "verify_overview"
            ) {
                let headings = input["source_heading_context"].as_array();
                if headings.is_some_and(|rows| {
                    rows.iter()
                        .any(|row| row["heading_path"].to_string().contains("2026-02-12"))
                }) {
                    self.dated_heading_checks.fetch_add(1, Ordering::SeqCst);
                }
            }
            if task == "verify"
                && input["knowledge"].as_array().is_some_and(|records| {
                    records.iter().any(|record| record["kind"] == "issue_state")
                })
            {
                let siblings = input["related_source_context"].as_array().unwrap();
                if siblings.iter().any(|row| {
                    row["kind"] == "reported_outcome"
                        && row["topic"] == "policy"
                        && row["exact_excerpt"]
                            .as_str()
                            .unwrap_or("")
                            .contains("two-person")
                        && row["citation_role"]
                            == "verifier_context_only_not_citeable_in_this_topic"
                }) {
                    self.source_context_checks.fetch_add(1, Ordering::SeqCst);
                    if input["draft"].to_string().contains("no further detail") {
                        self.verifier_rejections.fetch_add(1, Ordering::SeqCst);
                        return Ok(GenerationResponse {
                            model: "fixture".into(),
                            text: json!({"supported":false, "issues":[
                                "The same source reports an additional policy outcome in another topic."
                            ]}).to_string(),
                        });
                    }
                }
            }
            let mut response = self.inner.generate(r).await?;
            if task == "synthesize" && topic == "case" {
                self.case_syntheses.fetch_add(1, Ordering::SeqCst);
                if input.get("repair_feedback").is_none() {
                    let mut draft: Value = serde_json::from_str(&response.text).unwrap();
                    draft["sections"][0]["paragraphs"][0]["text"] =
                        json!("The report gives no further detail about the case.");
                    response.text = draft.to_string();
                } else {
                    self.rewrites.fetch_add(1, Ordering::SeqCst);
                }
            }
            Ok(response)
        })
    }
}

fn committee_record(outcome: &str) -> String {
    format!(
        "# Committee record — 2026-02-12\n\n\
         ## Case\n\nISSUE case: The inquiry was closed when the report was delivered.\n\n\
         ## Policy\n\nREPORT policy: {outcome}\n"
    )
}

#[tokio::test]
async fn cross_topic_verifier_repairs_false_absence_and_sees_document_date() {
    let (_temp, cfg, _) = project();
    let model = CrossTopicModel::new();
    put(
        &cfg,
        "committee.md",
        &committee_record("The committee reported the two-person approval rule is in use."),
    );
    let first = engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    assert!(first.degraded_topics.is_empty());
    assert!(!first.degraded_overview);
    assert!(model.verifier_rejections.load(Ordering::SeqCst) >= 1);
    assert!(model.source_context_checks.load(Ordering::SeqCst) >= 1);
    assert!(model.dated_heading_checks.load(Ordering::SeqCst) >= 1);
    assert!(model.rewrites.load(Ordering::SeqCst) >= 1);
    assert!(first.quality_diagnostics.iter().any(|d| d.topic == "case"
        && d.check == "semantic_verification"
        && d.issues.iter().any(|i| i.contains("same source"))));
    let case = fs::read_to_string(cfg.wiki.join("topics/case.md")).unwrap();
    assert!(!case.contains("no further detail"));

    let conn = storage::read_only(&cfg.state.join("state.db")).unwrap();
    assert!(
        storage::views(&conn)
            .unwrap()
            .iter()
            .all(|u| u.effective_at.is_empty())
    );
    drop(conn);

    let before_verification = model.source_context_checks.load(Ordering::SeqCst);
    // Only the policy section changes, but it changes the context needed to
    // verify an unchanged case topic sharing the source document.
    put(
        &cfg,
        "committee.md",
        &committee_record("The committee reported the two-person approval rule has been retired."),
    );
    let changed = engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    assert!(!changed.no_op);
    // Reuse the unchanged writer draft if cached, but the changed sibling
    // evidence must trigger a new semantic verification of the case topic.
    assert!(model.source_context_checks.load(Ordering::SeqCst) > before_verification);
    assert!(changed.degraded_topics.is_empty());

    let before = fs::read_to_string(cfg.wiki.join("topics/case.md")).unwrap();
    let noop = engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    assert!(noop.no_op);
    assert_eq!(noop.model_calls, 0);
    assert_eq!(
        before,
        fs::read_to_string(cfg.wiki.join("topics/case.md")).unwrap()
    );
}
