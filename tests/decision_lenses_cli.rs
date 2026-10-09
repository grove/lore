mod common;

use lore::{
    context,
    engine::{self, UpdateOptions},
    storage,
};
use serde_json::Value;
use std::{fs, process::Command};

#[tokio::test]
async fn decision_and_case_commands_keep_source_status_and_work_without_a_provider() {
    let (_dir, mut config, model) = common::project();
    common::put(
        &config,
        "dispatch.md",
        "# Dispatch\n## Decision\nDECISION dispatch: Queue admission remains bounded; emergency drains have a separate review requirement.\n## Rationale\nDECISION dispatch: Bounded admission prevents unbounded request retention.\n## Observed outcome\nREPORT dispatch: The staging drill completed; production behavior was not verified.\n",
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
    let bytes = fs::read(config.state.join("state.db")).unwrap();
    let conn = storage::read_only(&config.state.join("state.db")).unwrap();
    let revision = storage::registry_revision(&conn).unwrap();
    for command in ["decisions", "cases"] {
        let output = Command::new(env!("CARGO_BIN_EXE_lore"))
            .arg("--config")
            .arg(&config.config_path)
            .args(["--json", command, "dispatch", "--max-tokens", "12000"])
            .env_remove("OPENAI_API_KEY")
            .env_remove("LORE_INSPECTION_ROOT")
            .env_remove("LORE_ALLOW_HOSTED_EGRESS")
            .env_remove("LORE_ALLOW_CHECKOUT_EGRESS")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        let text = String::from_utf8(output.stdout).unwrap();
        let result: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(result["schema_version"], 1);
        assert_eq!(result["model_calls"], 0);
        assert_eq!(result["context"]["schema_version"], 2);
        assert_eq!(result["registry_revision"], revision);
        assert!(text.contains("emergency drains"));
        assert!(text.contains("production behavior was not verified"));
        assert!(context::count_tokens(&text) <= 12000);
        if command == "decisions" {
            assert!(!result["lenses"].as_array().unwrap().is_empty());
        } else {
            assert!(!result["cases"].as_array().unwrap().is_empty());
            assert_eq!(result["runtime_execution"], false);
            assert_eq!(result["tests_run"], 0);
        }
    }
    assert_eq!(bytes, fs::read(config.state.join("state.db")).unwrap());
}
