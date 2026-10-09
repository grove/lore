mod common;
use common::*;
use lore::{
    config::{ResolvedConfig, SourceRoot},
    inference::*,
    provider_wire::*,
    publish, util,
};
use serde_json::json;
use std::fs;
fn request() -> DecisionRequest {
    DecisionRequest {
        input: "Synthetic".into(),
        questions: vec![
            DecisionQuestion {
                name: "first".into(),
                instructions: "Check the first condition".into(),
                kind: QuestionKind::Predicate,
            },
            DecisionQuestion {
                name: "second".into(),
                instructions: "Check the second condition".into(),
                kind: QuestionKind::Predicate,
            },
        ],
    }
}
#[test]
fn normalizes_openai_order_and_rejects_unrequested_systemone_answers() {
    let r = request();
    let response = json!({"model":"m","answers":[{"name":"second","type":"predicate","probability":0.2},{"name":"first","type":"predicate","probability":0.8}]});
    let answer = openai_decisions_response(&r, &response).unwrap();
    assert_eq!(answer.answers[0].name, "first");
    let extra = json!({"model":"m","answers":{"first":{"type":"noul","noul":0.8},"second":{"type":"noul","noul":0.2},"unexpected":{"type":"noul","noul":0.3}}});
    assert!(systemone_response(&r, &extra).is_err());
}
#[test]
fn choice_must_agree_with_returned_probabilities() {
    let r = DecisionRequest {
        input: "Synthetic".into(),
        questions: vec![DecisionQuestion {
            name: "choice".into(),
            instructions: "Choose".into(),
            kind: QuestionKind::Choice {
                options: vec![
                    OptionDefinition {
                        id: "a".into(),
                        description: "A".into(),
                    },
                    OptionDefinition {
                        id: "b".into(),
                        description: "B".into(),
                    },
                ],
            },
        }],
    };
    let raw = json!({"model":"m","answers":{"choice":{"type":"choice","choice":"a","probabilities":{"a":0.1,"b":0.9}}}});
    assert!(systemone_response(&r, &raw).is_err());
}
#[test]
fn rejects_too_small_context_and_sources_inside_generated_state() {
    let (_dir, cfg, _) = project();
    let mut c = cfg.config.clone();
    c.processing.max_section_bytes = 512;
    c.processing.max_context_bytes = 1024;
    assert!(ResolvedConfig::resolve(c, &cfg.config_path).is_err());
    let mut c = cfg.config.clone();
    c.sources.roots = vec![SourceRoot {
        id: "bad".into(),
        path: ".lore/snapshots".into(),
        material: Default::default(),
        origin: None,
    }];
    assert!(ResolvedConfig::resolve(c, &cfg.config_path).is_err());
}
#[test]
fn cleanup_removes_owned_abandoned_sibling_stage() {
    let (_dir, cfg, _) = project();
    let _lock = publish::ProjectLock::acquire(&cfg).unwrap();
    let generation = "run_abandoned";
    util::private_dir(&publish::stage_dir(&cfg, generation)).unwrap();
    let sibling = cfg
        .wiki
        .parent()
        .unwrap()
        .join(format!(".lore-stage-{generation}"));
    fs::create_dir(&sibling).unwrap();
    fs::write(sibling.join("private.md"), "retained evidence").unwrap();
    publish::cleanup_abandoned(&cfg).unwrap();
    assert!(!sibling.exists());
    assert!(!cfg.state.join("staging").exists());
}
#[test]
fn parser_does_not_ingest_publication_backups() {
    let (_dir, cfg, _) = project();
    let mut c = cfg.config.clone();
    c.sources.roots = vec![SourceRoot {
        id: "project".into(),
        path: ".".into(),
        material: Default::default(),
        origin: None,
    }];
    let c = ResolvedConfig::resolve(c, &cfg.config_path).unwrap();
    let backup = cfg.base.join(".lore-backup-run_old");
    fs::create_dir(&backup).unwrap();
    fs::write(backup.join("evidence.md"), "Generated, not original").unwrap();
    assert!(lore::sources::scan(&c).unwrap().documents.is_empty());
}
