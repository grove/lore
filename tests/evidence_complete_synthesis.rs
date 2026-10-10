mod common;
use common::*;
use lore::{
    engine::{self, UpdateOptions},
    inference::*,
};
use serde_json::{Value, json};
use std::{
    fs,
    sync::atomic::{AtomicUsize, Ordering},
};

const BAD: &str = "POL-002 supersedes POL-001. This proves enforcement.";
const GOOD: &str =
    "POL-002 supersedes POL-001. This documents a decision, not proof of enforcement.";
const UNTOUCHED: &str = "The board originally chose two-person approval.";

#[derive(Clone, Copy)]
enum Behavior {
    Repair,
    ForgeLocation,
    ForgeCitation,
    ForgeFinding,
}

struct LocatedModel {
    delegate: FakeModel,
    behavior: Behavior,
    patches: AtomicUsize,
    checks: AtomicUsize,
    endpoint_seen: AtomicUsize,
}
impl LocatedModel {
    fn new(behavior: Behavior) -> Self {
        Self {
            delegate: FakeModel::new(),
            behavior,
            patches: AtomicUsize::new(0),
            checks: AtomicUsize::new(0),
            endpoint_seen: AtomicUsize::new(0),
        }
    }
}
impl GenerativeModel for LocatedModel {
    fn descriptor(&self) -> &ModelDescriptor {
        self.delegate.descriptor()
    }
    fn generate<'a>(&'a self, r: &'a GenerationRequest) -> ModelFuture<'a, GenerationResponse> {
        Box::pin(async move {
            assert!(r.input.len() + r.instructions.len() <= 64_000);
            let input: Value = serde_json::from_str(&r.input).unwrap();
            let task = input["task"].as_str().unwrap();
            let link = input["documented_decision_relationships"]
                .as_array()
                .and_then(|links| links.iter().find(|l| l["from_label"] == "POL-002"));
            let Some(link) = link else {
                return self.delegate.generate(r).await;
            };
            if task == "synthesize" && input["topic"] == "access-policy" {
                let rows = input["knowledge"].as_array().unwrap();
                let from = rows.iter().find(|u| u["id"] == link["from_id"]).unwrap();
                let to = rows.iter().find(|u| u["id"] == link["to_id"]).unwrap();
                assert_ne!(from["topic"], to["topic"]);
                assert!(!from["evidence"].as_array().unwrap().is_empty());
                assert!(!to["evidence"].as_array().unwrap().is_empty());
                assert!(
                    link["exact_excerpt"]
                        .as_str()
                        .unwrap()
                        .contains("supersedes POL-001")
                );
                assert!(
                    input["primary_knowledge_ids"]
                        .as_array()
                        .unwrap()
                        .contains(&to["id"])
                );
                assert!(
                    !input["primary_knowledge_ids"]
                        .as_array()
                        .unwrap()
                        .contains(&from["id"])
                );
                self.endpoint_seen.fetch_add(1, Ordering::SeqCst);
                if let Some(targets) = input["repair_targets"].as_array() {
                    self.patches.fetch_add(1, Ordering::SeqCst);
                    assert_eq!(targets.len(), 1);
                    assert_eq!(targets[0]["section"], 0);
                    assert_eq!(targets[0]["paragraph"], 0);
                    assert_eq!(targets[0]["original_text"], BAD);
                    assert_eq!(targets[0]["findings"].as_array().unwrap().len(), 2);
                    assert!(
                        !r.input.contains(UNTOUCHED),
                        "A good paragraph is not a repair target"
                    );
                    let paragraph = if matches!(self.behavior, Behavior::ForgeLocation) {
                        1
                    } else {
                        0
                    };
                    let ids = if matches!(self.behavior, Behavior::ForgeCitation) {
                        json!([to["id"], "not-a-citable-id"])
                    } else {
                        json!([to["id"], from["id"]])
                    };
                    return Ok(GenerationResponse {
                        usage: None,
                        model: "fixture".into(),
                        text: json!({"repairs":[{
                            "section":0, "paragraph":paragraph, "text":GOOD, "knowledge_ids":ids
                        }]})
                        .to_string(),
                    });
                }
                return Ok(GenerationResponse {
                    usage: None,
                    model: "fixture".into(),
                    text: json!({"sections":[{
                        "heading":"Documented policy", "paragraphs":[
                            {"text":BAD, "knowledge_ids":[to["id"]]},
                            {"text":UNTOUCHED, "knowledge_ids":[to["id"]]}
                        ]
                    }]})
                    .to_string(),
                });
            }
            if task == "verify" && input["draft"]["sections"][0]["heading"] == "Documented policy" {
                self.checks.fetch_add(1, Ordering::SeqCst);
                assert!(
                    r.instructions
                        .contains("not the order in which sections or paragraphs")
                );
                assert_eq!(
                    input["draft"]["sections"][0]["paragraphs"][1]["text"],
                    UNTOUCHED
                );
                if input["draft"]["sections"][0]["paragraphs"][0]["text"] == BAD {
                    let excerpt = if matches!(self.behavior, Behavior::ForgeFinding) {
                        "Sentence not in this document."
                    } else {
                        "This proves enforcement."
                    };
                    return Ok(GenerationResponse { usage: None, model:"fixture".into(), text:json!({
                        "supported":false,
                        "issues":["Cite the successor as well as the predecessor.", "Do not promote a policy decision to proven enforcement."],
                        "findings":[
                            {"section":0,"paragraph":0,"excerpt":"POL-002 supersedes POL-001.","reason":"Cite the successor as well as the predecessor."},
                            {"section":0,"paragraph":0,"excerpt":excerpt,"reason":"Do not promote a policy decision to proven enforcement."}
                        ]
                    }).to_string() });
                }
                let ids = input["draft"]["sections"][0]["paragraphs"][0]["knowledge_ids"]
                    .as_array()
                    .unwrap();
                assert!(ids.contains(&link["from_id"]) && ids.contains(&link["to_id"]));
                assert_eq!(input["draft"]["sections"][0]["paragraphs"][0]["text"], GOOD);
            }
            self.delegate.generate(r).await
        })
    }
}

fn original(cfg: &lore::config::ResolvedConfig) {
    put(
        cfg,
        "POL-001.md",
        "DECISION access-policy: The board chose two-person approval.\n",
    );
}
fn successor(cfg: &lore::config::ResolvedConfig) {
    put(
        cfg,
        "POL-002.md",
        "DECISION revised-policy: POL-002 supersedes POL-001 and selects committee approval.\n",
    );
}

#[tokio::test]
async fn closed_external_citations_and_located_repairs_preserve_good_paragraphs() {
    let (_tmp, cfg, _) = project();
    original(&cfg);
    successor(&cfg);
    let model = LocatedModel::new(Behavior::Repair);
    let report = engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    assert!(report.degraded_topics.is_empty());
    assert!(!report.degraded_overview);
    assert!(model.endpoint_seen.load(Ordering::SeqCst) > 0);
    assert_eq!(model.patches.load(Ordering::SeqCst), 1);
    assert_eq!(model.checks.load(Ordering::SeqCst), 2);
    let page = fs::read_to_string(cfg.wiki.join("topics/access-policy.md")).unwrap();
    assert!(page.contains(GOOD));
    assert!(page.contains(UNTOUCHED));
    assert!(!page.contains("This proves enforcement."));
    assert!(
        page.contains("POL-002.md"),
        "The supplementary citation must have a real source footnote"
    );
    let before = fs::read(cfg.state.join("state.db")).unwrap();
    let noop = engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    assert!(noop.no_op);
    assert_eq!(noop.model_calls, 0);
    assert_eq!(before, fs::read(cfg.state.join("state.db")).unwrap());
    assert_eq!(
        page,
        fs::read_to_string(cfg.wiki.join("topics/access-policy.md")).unwrap()
    );
}

#[tokio::test]
async fn forged_locations_findings_and_citations_cannot_replace_a_publication() {
    for behavior in [
        Behavior::ForgeLocation,
        Behavior::ForgeCitation,
        Behavior::ForgeFinding,
    ] {
        let (_tmp, cfg, normal) = project();
        original(&cfg);
        engine::update(&cfg, &normal, None, UpdateOptions::default())
            .await
            .unwrap();
        let before_db = fs::read(cfg.state.join("state.db")).unwrap();
        let before_index = fs::read(cfg.wiki.join("index.md")).unwrap();
        let before_topic = fs::read(cfg.wiki.join("topics/access-policy.md")).unwrap();
        successor(&cfg);
        let model = LocatedModel::new(behavior);
        assert!(
            engine::update(&cfg, &model, None, UpdateOptions::default())
                .await
                .is_err()
        );
        assert_eq!(before_db, fs::read(cfg.state.join("state.db")).unwrap());
        assert_eq!(before_index, fs::read(cfg.wiki.join("index.md")).unwrap());
        assert_eq!(
            before_topic,
            fs::read(cfg.wiki.join("topics/access-policy.md")).unwrap()
        );
    }
}
