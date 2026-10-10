mod common;

use lore::{
    config::ResolvedConfig,
    context,
    engine::{self, UpdateOptions},
    storage,
};
use serde_json::Value;
use std::{
    fs,
    process::{Command, Output},
};

fn invoke(config: &ResolvedConfig, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_lore"))
        .arg("--config")
        .arg(&config.config_path)
        .args(args)
        .env_remove("OPENAI_API_KEY")
        .env_remove("LORE_INSPECTION_ROOT")
        .env_remove("LORE_ALLOW_HOSTED_EGRESS")
        .env_remove("LORE_ALLOW_CHECKOUT_EGRESS")
        .output()
        .unwrap()
}

fn success(output: Output) -> (Value, String) {
    assert!(
        output.status.success(),
        "{} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let text = String::from_utf8(output.stdout).unwrap();
    (serde_json::from_str(&text).unwrap(), text)
}

async fn fixture() -> (tempfile::TempDir, ResolvedConfig, common::FakeModel) {
    let (dir, mut config, model) = common::project();
    common::put(
        &config,
        "queue.md",
        "# Dispatch\nDECISION queue: QUEUE_CAPACITY is 128; emergency drains alone may bypass normal admission.\n",
    );
    common::put(
        &config,
        "security.md",
        "DECISION security: Dispatch traces never contain identity tokens.\n",
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
    (dir, config, model)
}

#[tokio::test]
async fn direct_exploration_uses_original_evidence_and_complete_budgets_without_inference() {
    let (_dir, config, _model) = fixture().await;
    let database = fs::read(config.state.join("state.db")).unwrap();
    let (result, text) = success(invoke(
        &config,
        &[
            "--json",
            "explore",
            "QUEUE_CAPACITY",
            "--no-cache",
            "--max-tokens",
            "8000",
        ],
    ));
    assert_eq!(result["schema_version"], 1);
    assert_eq!(result["model_calls"], 0);
    assert!(result["knowledge"].as_array().unwrap().iter().any(|r| {
        r["statement"]
            .as_str()
            .unwrap()
            .contains("emergency drains")
    }));
    let conn = storage::read_only(&config.state.join("state.db")).unwrap();
    for evidence in result["evidence"].as_array().unwrap() {
        let original = storage::evidence_snapshot(&conn, evidence["id"].as_str().unwrap()).unwrap();
        assert_eq!(evidence["excerpt"], original.excerpt);
        assert_eq!(evidence["digest"], original.digest);
    }
    assert!(context::count_tokens(&text) <= 8000);
    assert!(result["used_tokens"].as_u64().unwrap() as usize >= context::count_tokens(&text));
    let markdown = invoke(
        &config,
        &[
            "explore",
            "QUEUE_CAPACITY",
            "--no-cache",
            "--max-tokens",
            "8000",
        ],
    );
    assert!(markdown.status.success());
    assert!(context::count_tokens(&String::from_utf8(markdown.stdout).unwrap()) <= 8000);
    assert_eq!(database, fs::read(config.state.join("state.db")).unwrap());
    assert!(!config.state.join("knowledge-zoom").exists());
}

#[tokio::test]
async fn repeated_navigation_revalidates_without_rewriting_an_unchanged_cache() {
    let (_dir, config, _model) = fixture().await;
    let (first, _) = success(invoke(&config, &["--json", "explore", "queue"]));
    let cache = config.state.join("knowledge-zoom/snapshot.json");
    let bytes = fs::read(&cache).unwrap();
    let modified = fs::metadata(&cache).unwrap().modified().unwrap();
    let (second, _) = success(invoke(&config, &["--json", "explore", "queue"]));
    assert_eq!(first, second);
    assert_eq!(bytes, fs::read(&cache).unwrap());
    assert_eq!(modified, fs::metadata(&cache).unwrap().modified().unwrap());
    success(invoke(
        &config,
        &["--json", "explore", "security", "--no-cache"],
    ));
    assert_eq!(bytes, fs::read(&cache).unwrap());
    assert_eq!(modified, fs::metadata(&cache).unwrap().modified().unwrap());
    let id = first["nodes"][0]["id"]
        .as_str()
        .unwrap_or_else(|| panic!("navigation response has no selectable node: {first:#}"));
    let (focused, _) = success(invoke(&config, &["--json", "explore", "--node", id]));
    assert!(!focused["knowledge"].as_array().unwrap().is_empty());
}

#[test]
fn invalid_navigation_limits_fail_before_configuration_loading() {
    let (_dir, config, _model) = common::project();
    fs::remove_file(&config.config_path).unwrap();
    for arguments in [
        vec!["--json", "explore", "--max-nodes", "0"],
        vec!["--json", "explore", "--max-tokens", "511"],
        vec!["--json", "explore", " "],
    ] {
        let output = invoke(&config, &arguments);
        assert_eq!(output.status.code(), Some(2));
        let error: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(error["code"], "invalid_arguments");
    }
}

#[tokio::test]
async fn unmanaged_cache_does_not_block_navigation_or_overwrite_user_files() {
    let (_dir, config, _model) = fixture().await;
    let directory = config.state.join("knowledge-zoom");
    fs::create_dir(&directory).unwrap();
    let note = directory.join("notes.txt");
    fs::write(&note, "retained user note").unwrap();
    let (result, _) = success(invoke(&config, &["--json", "explore", "queue"]));
    assert!(!result["knowledge"].as_array().unwrap().is_empty());
    assert_eq!(fs::read_to_string(note).unwrap(), "retained user note");
    assert!(!directory.join("snapshot.json").exists());
}
