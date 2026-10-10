mod common;

use lore::{
    config::ResolvedConfig,
    context,
    engine::{self, UpdateOptions},
};
use serde_json::Value;
use std::{
    fs,
    process::{Command, Output},
};

fn invoke(config: &ResolvedConfig, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_lore"))
        .arg("--config")
        .arg(&config.config_path)
        .arg("--json")
        .args(arguments)
        .env_remove("OPENAI_API_KEY")
        .env_remove("LORE_INSPECTION_ROOT")
        .env_remove("LORE_ALLOW_HOSTED_EGRESS")
        .env_remove("LORE_ALLOW_CHECKOUT_EGRESS")
        .output()
        .unwrap()
}

fn success(output: Output) -> Value {
    assert!(
        output.status.success(),
        "{} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    serde_json::from_slice(&output.stdout).unwrap()
}

#[tokio::test]
async fn named_baseline_changes_and_guard_form_a_complete_read_only_source_workflow() {
    let (_dir, mut config, model) = common::project();
    common::put(
        &config,
        "queue.md",
        "DECISION queue: Queue admission has capacity 128 except emergency drains.\n",
    );
    engine::update(&config, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    config.config.models.generative.enabled = false;
    fs::write(
        &config.config_path,
        serde_yaml::to_string(&config.config).unwrap(),
    )
    .unwrap();
    let saved = success(invoke(&config, &["baseline", "save", "joined"]));
    assert_eq!(saved["schema_version"], 1);
    assert_eq!(saved["source_write"], false);
    let unchanged = success(invoke(&config, &["changes", "--since", "joined"]));
    assert_eq!(unchanged["registry_changed"], false);
    assert!(unchanged["changes"].as_array().unwrap().is_empty());
    let duplicate = invoke(&config, &["baseline", "save", "joined"]);
    assert!(!duplicate.status.success());
    assert!(
        String::from_utf8(duplicate.stdout)
            .unwrap()
            .contains("--replace")
    );

    common::put(
        &config,
        "limit.md",
        "DECISION queue: Queue diagnostics must never include identity tokens.\n",
    );
    engine::update(&config, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let changed = success(invoke(&config, &["changes", "--since", "joined"]));
    assert_eq!(changed["registry_changed"], true);
    assert!(
        serde_json::to_string(&changed)
            .unwrap()
            .contains("never include identity tokens")
    );
    let before = fs::read(config.state.join("state.db")).unwrap();
    let output = invoke(
        &config,
        &[
            "guard",
            "--since",
            "joined",
            "--task",
            "Improve queue diagnostics",
            "--no-inspect",
            "--no-cache",
        ],
    );
    let text = String::from_utf8(output.stdout.clone()).unwrap();
    let guarded = success(output);
    assert_eq!(guarded["schema_version"], 1);
    assert_eq!(guarded["execution"], false);
    assert_eq!(guarded["source_write"], false);
    assert_eq!(guarded["assessment_status"], "partial_static_guidance");
    assert_eq!(
        guarded["intelligence"]["capabilities"]["inspection"],
        "disabled_by_caller"
    );
    let actual_tokens = context::count_tokens(&text);
    let reported_tokens = guarded["budget"]["used_tokens"].as_u64().unwrap() as usize;
    assert!(actual_tokens <= reported_tokens);
    assert!(reported_tokens <= 8000);
    assert_eq!(before, fs::read(config.state.join("state.db")).unwrap());
    let removed = success(invoke(&config, &["baseline", "remove", "joined"]));
    assert_eq!(removed["removed"], "joined");
    assert!(!config.state.join("baselines/joined.json").exists());
    assert!(config.base.join("docs/queue.md").is_file());
}

#[test]
fn traversal_and_invalid_budgets_are_rejected_before_configuration_loading() {
    let (_dir, config, _model) = common::project();
    fs::remove_file(&config.config_path).unwrap();
    for args in [
        vec!["baseline", "save", "../escape"],
        vec!["changes", "--since", "joined", "--max-tokens", "511"],
        vec!["guard", "--since", "joined", "--max-tokens", "1023"],
    ] {
        let output = invoke(&config, &args);
        assert_eq!(output.status.code(), Some(2));
        let error: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(error["code"], "invalid_arguments");
    }
}
