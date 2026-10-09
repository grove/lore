#![allow(dead_code)]
use lore::{
    config::{Config, ResolvedConfig},
    inference::*,
};
use serde_json::{Value, json};
use std::{
    fs,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};
pub struct FakeModel {
    descriptor: ModelDescriptor,
    pub calls: AtomicUsize,
    pub fail_synthesis: AtomicBool,
    pub invalid_quote: AtomicBool,
    pub repair_quote: AtomicBool,
    pub reject_verification: AtomicBool,
}
impl FakeModel {
    pub fn new() -> Self {
        Self {
            descriptor: ModelDescriptor {
                provider: Provider::Ollama,
                model: "fixture-v1".into(),
                location: ExecutionLocation::Local,
            },
            calls: AtomicUsize::new(0),
            fail_synthesis: AtomicBool::new(false),
            invalid_quote: AtomicBool::new(false),
            repair_quote: AtomicBool::new(false),
            reject_verification: AtomicBool::new(false),
        }
    }
}
fn complete_citations(input: &Value, text: &str, mut ids: Vec<Value>) -> Vec<Value> {
    if let Some(links) = input["documented_decision_relationships"].as_array() {
        for link in links {
            if text.contains(link["from_label"].as_str().unwrap_or("\0"))
                && text.contains(link["to_label"].as_str().unwrap_or("\0"))
            {
                for field in ["from_id", "to_id"] {
                    if !ids.contains(&link[field]) {
                        ids.push(link[field].clone());
                    }
                }
            }
        }
    }
    ids
}
impl GenerativeModel for FakeModel {
    fn descriptor(&self) -> &ModelDescriptor {
        &self.descriptor
    }
    fn generate<'a>(&'a self, r: &'a GenerationRequest) -> ModelFuture<'a, GenerationResponse> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let input: Value = serde_json::from_str(&r.input).unwrap();
            let task = input["task"].as_str().unwrap();
            let output = match task {
                "extract" => {
                    let text = input["text"].as_str().unwrap();
                    let mut assertions = Vec::new();
                    for line in text.lines() {
                        let Some((prefix, rest)) = line.split_once(' ') else {
                            continue;
                        };
                        let (kind, mut lifecycle) = match prefix {
                            "DECISION" => ("decision", "accepted"),
                            "PLAN" => ("proposal", "proposed"),
                            "REPORT" => ("reported_outcome", "completed"),
                            "ISSUE" => ("issue_state", "unknown"),
                            _ => continue,
                        };
                        if input["context"]
                            .as_str()
                            .unwrap_or("")
                            .contains("status: rejected")
                        {
                            lifecycle = "rejected";
                        }
                        let (topic, statement) =
                            rest.split_once(": ").unwrap_or(("database", rest));
                        let statement = statement.replace(
                            "MySQL remains our database.",
                            "MySQL is the selected database.",
                        );
                        let repaired = self.repair_quote.load(Ordering::SeqCst)
                            && r.instructions
                                .contains("evidence quote is missing or ambiguous");
                        let quote = if self.invalid_quote.load(Ordering::SeqCst) && !repaired {
                            "This passage does not exist."
                        } else {
                            line
                        };
                        assertions.push(json!({"topic":topic,"topic_title":topic,"subject":topic,"statement":statement,"kind":kind,"lifecycle":lifecycle,"scope":"production","effective_at":"","quote":quote}));
                    }
                    json!({"assertions":assertions})
                }
                "reconcile" => {
                    let a = &input["assertion"];
                    let statement = a["statement"].as_str().unwrap();
                    let mut equivalent = String::new();
                    let mut relations = Vec::new();
                    for c in input["candidates"].as_array().unwrap() {
                        if c["statement"] == a["statement"]
                            && c["kind"] == a["kind"]
                            && c["lifecycle"] == a["lifecycle"]
                        {
                            equivalent = c["id"].as_str().unwrap().into();
                            continue;
                        }
                        let old = c["statement"].as_str().unwrap();
                        if statement.contains("reaffirms ADR-001")
                            && old.contains("MySQL")
                            && c["kind"] == "decision"
                        {
                            relations.push(json!({"target_id":c["id"],"kind":"reaffirms","quote":a["quote"],"reason":"The review explicitly reaffirms ADR-001."}));
                        } else if statement.contains("replaces ADR-001")
                            && old.contains("MySQL")
                            && c["kind"] == "decision"
                        {
                            relations.push(json!({"target_id":c["id"],"kind":"supersedes","quote":a["quote"],"reason":"The new ADR explicitly replaces ADR-001."}));
                        } else if statement.contains("PostgreSQL")
                            && old.contains("MySQL")
                            && c["kind"] == a["kind"]
                            && c["lifecycle"] == a["lifecycle"]
                        {
                            relations.push(json!({"target_id":c["id"],"kind":"contradicts","quote":a["quote"],"reason":"Different documented database choices in the same scope."}));
                        }
                    }
                    json!({"equivalent_to":equivalent,"relations":relations,"uncertain":false})
                }
                "synthesize" | "overview" => {
                    if self.fail_synthesis.load(Ordering::SeqCst) {
                        return Err(ModelError::Unavailable("fixture failure".into()));
                    }
                    if let Some(targets) = input["repair_targets"].as_array() {
                        let repairs = targets.iter().map(|target| {
                            let ids = target["original_knowledge_ids"].as_array().cloned().unwrap_or_default();
                            let text = if target["paragraph"].is_null() {
                                target["excerpt"].as_str().unwrap().to_owned()
                            } else {
                                input["knowledge"].as_array().unwrap().iter()
                                    .filter(|u| ids.contains(&u["id"]))
                                    .map(|u| u["statement"].as_str().unwrap())
                                    .collect::<Vec<_>>().join(" ")
                            };
                            let ids = complete_citations(&input, &text, ids);
                            json!({"section":target["section"],"paragraph":target["paragraph"],"text":text,"knowledge_ids":ids})
                        }).collect::<Vec<_>>();
                        json!({"repairs":repairs})
                    } else {
                        let paragraphs = input["knowledge"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|u| {
                                let ids = complete_citations(
                                    &input,
                                    u["statement"].as_str().unwrap(),
                                    vec![u["id"].clone()],
                                );
                                json!({"text":u["statement"],"knowledge_ids":ids})
                            })
                            .collect::<Vec<_>>();
                        json!({"sections":[{"heading":"Understanding","paragraphs":paragraphs}]})
                    }
                }
                "verify" | "verify_overview" => {
                    if self.reject_verification.load(Ordering::SeqCst) {
                        json!({"supported":false,"issues":["fixture verification rejected the draft"],"findings":[]})
                    } else {
                        json!({"supported":true,"issues":[],"findings":[]})
                    }
                }
                _ => panic!("unexpected task {task}"),
            };
            Ok(GenerationResponse {
                model: self.descriptor.model.clone(),
                text: output.to_string(),
            })
        })
    }
}
pub fn project() -> (tempfile::TempDir, ResolvedConfig, FakeModel) {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("docs")).unwrap();
    let mut config = Config::default();
    config.project.name = "fixture".into();
    let path = dir.path().join("lore.yml");
    fs::write(&path, serde_yaml::to_string(&config).unwrap()).unwrap();
    let resolved = ResolvedConfig::load(&path).unwrap();
    (dir, resolved, FakeModel::new())
}
pub fn put(config: &ResolvedConfig, file: &str, text: &str) {
    let path = config.base.join("docs").join(file);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}
