mod common;
use common::*;
use lore::{
    config::{Reasoning, ResolvedConfig},
    engine::{self, UpdateOptions},
    inference::*,
    provider_wire::{ollama_chat_request, openai_responses_request},
};
use serde_json::json;
use std::sync::Mutex;

struct Captured {
    fake: FakeModel,
    seen: Mutex<Vec<(String, Option<ReasoningEffort>)>>,
}
impl Captured {
    fn new() -> Self {
        Self {
            fake: FakeModel::new(),
            seen: Mutex::new(Vec::new()),
        }
    }
}
impl GenerativeModel for Captured {
    fn descriptor(&self) -> &ModelDescriptor {
        self.fake.descriptor()
    }

    fn generate<'a>(
        &'a self,
        request: &'a GenerationRequest,
    ) -> ModelFuture<'a, GenerationResponse> {
        Box::pin(async move {
            let input: serde_json::Value =
                serde_json::from_str(&request.input).expect("Lore input JSON");
            let task = input["task"].as_str().expect("task");
            self.seen.lock().unwrap().push((
                task.to_owned(),
                request.reasoning_effort,
            ));
            self.fake.generate(request).await
        })
    }
}

#[test]
fn defaults_cover_every_generating_and_verifying_task() {
    let r = Reasoning::default();
    assert!(r.enabled);
    for (task, expected) in [
        ("extract", ReasoningEffort::Low),
        ("reconcile", ReasoningEffort::High),
        ("synthesize", ReasoningEffort::Medium),
        ("overview", ReasoningEffort::Medium),
        ("verify", ReasoningEffort::High),
        ("verify_overview", ReasoningEffort::High),
        ("future-task", ReasoningEffort::Medium),
    ] {
        assert_eq!(r.for_task(task), Some(expected), "task: {task}");
    }
}

#[test]
fn reasoning_settings_are_typed_backwards_compatible_and_can_be_disabled() {
    let (_dir, config, _) = project();
    let old_format = serde_yaml::to_string(&config.config).unwrap();
    let parsed: lore::config::Config = serde_yaml::from_str(&old_format).unwrap();
    assert_eq!(
        parsed.models.reasoning.for_task("extract"),
        Some(ReasoningEffort::Low)
    );

    let mut custom = config.config.clone();
    custom.models.reasoning.extraction = ReasoningEffort::Xhigh;
    custom.models.reasoning.reconciliation = ReasoningEffort::Max;
    custom.models.reasoning.verification = ReasoningEffort::None;
    let yaml = serde_yaml::to_string(&custom).unwrap();
    let back: lore::config::Config = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(
        back.models.reasoning.for_task("extract"),
        Some(ReasoningEffort::Xhigh)
    );
    assert_eq!(
        back.models.reasoning.for_task("reconcile"),
        Some(ReasoningEffort::Max)
    );
    assert_eq!(
        back.models.reasoning.for_task("verify"),
        Some(ReasoningEffort::None)
    );
    let bad_yaml = "extraction: ultra";
    assert!(serde_yaml::from_str::<Reasoning>(bad_yaml).is_err());
    let typo = "verificaton: high";
    assert!(serde_yaml::from_str::<Reasoning>(typo).is_err());

    custom.models.reasoning.enabled = false;
    assert_eq!(custom.models.reasoning.for_task("extract"), None);
    assert_eq!(custom.models.reasoning.for_task("verify"), None);

    let orig = config.fingerprint;
    let updated = ResolvedConfig::resolve(custom, &config.config_path).unwrap();
    assert_ne!(orig, updated.fingerprint);
}

#[test]
fn responses_effort_is_optional_and_not_an_ollama_or_decisions_setting() {
    let request = GenerationRequest {
        instructions: "Synthetic test".into(),
        input: "Synthetic text".into(),
        schema: Some(json!({
            "type":"object","properties":{"ok":{"type":"boolean"}},
            "required":["ok"],"additionalProperties":false
        })),
        reasoning_effort: Some(ReasoningEffort::High),
    };
    let openai = openai_responses_request(&request, "gpt-6-luna");
    assert_eq!(openai["reasoning"]["effort"], "high");
    let local = ollama_chat_request(&request, "gemma4:12b");
    assert!(local.get("reasoning").is_none());
    assert!(local.get("reasoning_effort").is_none());

    let request = GenerationRequest {
        reasoning_effort: None,
        ..request
    };
    assert!(openai_responses_request(&request, "gpt-6-luna")
        .get("reasoning")
        .is_none());

    let decisions = lore::provider_wire::openai_decisions_request(
        &DecisionRequest {
            input: "Synthetic candidate".into(),
            questions: vec![DecisionQuestion {
                name: "relevant".into(),
                instructions: "Relevant?".into(),
                kind: QuestionKind::Predicate,
            }],
        },
        "gpt-6-luna",
    )
    .unwrap();
    assert!(decisions.get("reasoning").is_none());
}

#[tokio::test]
async fn configured_effort_routes_to_tasks_and_changes_invalidate_cache() {
    let (_dir, config, _) = project();
    put(
        &config,
        "decision.md",
        "DECISION database: MySQL is the selected database.\n",
    );
    put(
        &config,
        "proposal.md",
        "PLAN database: Consider PostgreSQL migration.\n",
    );
    let capture = Captured::new();
    let first = engine::update(
        &config,
        &capture,
        None,
        UpdateOptions::default(),
    )
    .await
    .unwrap();
    assert!(!first.no_op);
    let seen = capture.seen.lock().unwrap().clone();
    for (task, effort) in seen {
        assert_eq!(
            effort,
            config.config.models.reasoning.for_task(&task),
            "wrong effort for {task}"
        );
    }
    let mut changed = config.config.clone();
    changed.models.reasoning.extraction = ReasoningEffort::Medium;
    changed.models.reasoning.verification = ReasoningEffort::Low;
    let altered = ResolvedConfig::resolve(changed, &config.config_path).unwrap();
    assert_ne!(config.fingerprint, altered.fingerprint);
    let rerun = engine::update(
        &altered,
        &capture,
        None,
        UpdateOptions::default(),
    )
    .await
    .unwrap();
    assert!(!rerun.no_op);
    assert!(rerun.model_calls > 0);
    let no_op = engine::update(
        &altered,
        &capture,
        None,
        UpdateOptions::default(),
    )
    .await
    .unwrap();
    assert!(no_op.no_op);
    assert_eq!(no_op.model_calls, 0);
}
