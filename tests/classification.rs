mod common;
use common::*;
use lore::{
    domain,
    engine::{self, UpdateOptions},
    inference::*,
    storage,
};
use serde_json::{Value, json};
struct MixedDocumentModel {
    inner: FakeModel,
}
impl GenerativeModel for MixedDocumentModel {
    fn descriptor(&self) -> &ModelDescriptor {
        self.inner.descriptor()
    }
    fn generate<'a>(&'a self, r: &'a GenerationRequest) -> ModelFuture<'a, GenerationResponse> {
        Box::pin(async move {
            let input: Value = serde_json::from_str(&r.input).unwrap();
            if input["task"] == "extract" {
                assert!(r.instructions.contains(domain::CLASSIFICATION_GUIDANCE));
                let allowed = &r.schema.as_ref().unwrap()["properties"]["assertions"]["items"]["properties"]
                    ["kind"]["enum"];
                assert!(allowed.as_array().unwrap().contains(&json!("design")));
                let assertions=input["text"].as_str().unwrap().lines().filter_map(|line|{
     let (kind,lifecycle,topic)=if line.starts_with("The documented architecture") {("design","unknown","architecture")}
      else if line.starts_with("We propose") {("proposal","proposed","future")}
      else if line.starts_with("Every release requires") {("constraint","unknown","release")}
      else if line.starts_with("The team reports") {("reported_outcome","completed","delivery")}
      else {return None};
     Some(json!({"topic":topic,"topic_title":topic,"subject":"service","statement":line,"kind":kind,"lifecycle":lifecycle,"scope":"production","effective_at":"","quote":line}))
    }).collect::<Vec<_>>();
                return Ok(GenerationResponse {
                    model: "mixed-fixture".into(),
                    text: json!({"assertions":assertions}).to_string(),
                });
            }
            self.inner.generate(r).await
        })
    }
}
#[tokio::test]
async fn design_intent_requirements_and_reported_delivery_are_independent() {
    let (_dir, cfg, fake) = project();
    put(
        &cfg,
        "mixed.md",
        "# System notes\nThe documented architecture writes events to a journal; this is not runtime verification.\nWe propose replacing the journal next year.\nEvery release requires approval and a rollback checklist.\nThe team reports that a deployment completed.\n",
    );
    let model = MixedDocumentModel { inner: fake };
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let conn = storage::read_only(&cfg.state.join("state.db")).unwrap();
    let kinds = storage::views(&conn)
        .unwrap()
        .into_iter()
        .map(|u| u.kind)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        kinds,
        ["design", "proposal", "constraint", "reported_outcome"]
            .into_iter()
            .map(String::from)
            .collect()
    );
    let prose = std::fs::read_to_string(cfg.wiki.join("topics/architecture.md")).unwrap();
    assert!(prose.contains("Documented design, not independently verified"));
    assert!(!prose.contains("Proposed or planned work"));
    assert!(
        engine::update(&cfg, &model, None, UpdateOptions::default())
            .await
            .unwrap()
            .no_op
    );
}
#[test]
fn reaffirms_is_part_of_the_strict_model_contract() {
    let schema = domain::reconciliation_schema();
    assert!(
        schema["properties"]["relations"]["items"]["properties"]["kind"]["enum"]
            .as_array()
            .unwrap()
            .contains(&json!("reaffirms"))
    );
    assert_ne!(
        domain::documentary_basis("design"),
        domain::documentary_basis("proposal")
    );
}
