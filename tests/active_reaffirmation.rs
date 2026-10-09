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

/// Intentionally misclassifies an ongoing policy reaffirmation as `active`.
/// This fixture does not depend on Atlas, software architecture, or ADR files.
struct ActivePolicyModel {
    inner: FakeModel,
}
impl ActivePolicyModel {
    fn new() -> Self {
        Self {
            inner: FakeModel::new(),
        }
    }
}
impl GenerativeModel for ActivePolicyModel {
    fn descriptor(&self) -> &ModelDescriptor {
        self.inner.descriptor()
    }

    fn generate<'a>(
        &'a self,
        request: &'a GenerationRequest,
    ) -> ModelFuture<'a, GenerationResponse> {
        Box::pin(async move {
            let mut response = self.inner.generate(request).await?;
            let input: Value = serde_json::from_str(&request.input).unwrap();
            if input["task"] == "extract"
                && input["source"].as_str().unwrap_or("").contains(":notes/")
            {
                let mut extracted: Value = serde_json::from_str(&response.text).unwrap();
                for assertion in extracted["assertions"].as_array_mut().unwrap() {
                    if assertion["kind"] == "decision" {
                        assertion["lifecycle"] = json!("active");
                    }
                }
                response.text = extracted.to_string();
            }
            Ok(response)
        })
    }
}

#[tokio::test]
async fn active_policy_reaffirmation_is_valid_without_new_acceptance() {
    let (_temp, cfg, _) = project();
    let model = ActivePolicyModel::new();

    put(
        &cfg,
        "policies/POL-017.md",
        "DECISION policy: The board adopted a policy requiring two-person access approval.\n",
    );
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    put(
        &cfg,
        "notes/committee-review.md",
        "DECISION policy: The committee reaffirmed POL-017 and remains committed to two-person access approval.\n",
    );
    let updated = engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    assert!(!updated.no_op);
    let conn = storage::read_only(&cfg.state.join("state.db")).unwrap();
    let current = storage::views(&conn).unwrap();
    let reaffirmation = current
        .iter()
        .find(|x| x.statement.contains("committee reaffirmed"))
        .unwrap();
    assert_eq!(reaffirmation.base_lifecycle, "active");
    assert_eq!(
        storage::relations(&conn)
            .unwrap()
            .iter()
            .filter(|r| r.active && r.kind == "reaffirms")
            .count(),
        1
    );
    drop(conn);

    // A future proposal, negation, and condition are not affirmative events.
    put(
        &cfg,
        "notes/future.md",
        "PLAN policy: The committee might reaffirm POL-017 after a later consultation.\n",
    );
    put(
        &cfg,
        "notes/no-reaffirmation.md",
        "DECISION policy: The committee did not reaffirm POL-017.\n",
    );
    put(
        &cfg,
        "notes/conditional.md",
        "DECISION policy: The committee will reaffirm POL-017 only if stakeholders approve.\n",
    );
    // Even an explicitly positive replacement cannot supersede the accepted
    // policy when the successor is not itself accepted.
    put(
        &cfg,
        "notes/replacement.md",
        "DECISION policy: REG-021 supersedes POL-017 and changes the access rule.\n",
    );

    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let conn = storage::read_only(&cfg.state.join("state.db")).unwrap();
    let relations = storage::relations(&conn).unwrap();
    assert_eq!(
        relations
            .iter()
            .filter(|r| r.active && r.kind == "reaffirms")
            .count(),
        1
    );
    assert!(!relations.iter().any(|r| r.active && r.kind == "supersedes"));
    let original = storage::views(&conn)
        .unwrap()
        .into_iter()
        .find(|x| x.statement.contains("board adopted a policy"))
        .unwrap();
    assert_eq!(original.lifecycle, "accepted");
    drop(conn);

    let no_op = engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    assert!(no_op.no_op);
    assert_eq!(no_op.model_calls, 0);
}
