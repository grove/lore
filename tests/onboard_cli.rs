//! Regression tests for the thin onboard adapter. It must use context's
//! evidence, revision, privacy, budget and inspection contracts, not a lesson engine.

mod common;

use common::*;
use lore::{
    config::ResolvedConfig,
    context::{self, adaptive::AdaptiveResult},
    engine::{self, UpdateOptions},
};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

const CONDITION: &str = "Production dispatch preserves tenant ordering with a queue capacity of 128 jobs; staging uses 64 jobs. A full queue returns Busy without accepting a job.";
const PRIVATE_CODE: &str = "const CHECKOUT_ONLY_ORIENTATION_SENTINEL: usize = 128;\n";

async fn fixture() -> (tempfile::TempDir, ResolvedConfig) {
    let (temp, mut config, model) = project();
    put(
        &config,
        "dispatch.md",
        "DECISION dispatch: Harbor separates dispatch from worker execution so that a slow tenant cannot exhaust every worker.\n",
    );
    put(&config, "capacity.md", &format!("DECISION dispatch: {CONDITION}\n"));
    engine::update(&config, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    fs::create_dir(config.base.join("src")).unwrap();
    fs::write(config.base.join("src/dispatch.rs"), PRIVATE_CODE).unwrap();

    // The offline fallback is deliberately selected through a locally denied
    // provider, not by turning off the shared decision-context pipeline.
    config.config.models.generative.provider = "openai".into();
    config.config.models.generative.model = "denied-orientation-fixture".into();
    config.config.models.decision = None;
    config.config.models.embedding = None;
    config.config.privacy.local_only = true;
    config.config.context.cache = true;
    config.config.context.inspection.enabled = true;
    config.config.context.inspection.root = Some(".".into());
    fs::write(
        &config.config_path,
        serde_yaml::to_string(&config.config).unwrap(),
    )
    .unwrap();
    let config = ResolvedConfig::load_for_read(&config.config_path).unwrap();
    (temp, config)
}

fn invoke(config: &ResolvedConfig, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_lore"))
        .arg("--config")
        .arg(&config.config_path)
        .args(args)
        .env_remove("OPENAI_API_KEY")
        .env_remove("TYPESAFE_API_KEY")
        .env_remove("LORE_INSPECTION_ROOT")
        .env_remove("LORE_ALLOW_HOSTED_EGRESS")
        .env_remove("LORE_ALLOW_CHECKOUT_EGRESS")
        .env_remove("LORE_USAGE_LEDGER")
        .output()
        .unwrap()
}

fn success(output: Output) -> String {
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    String::from_utf8(output.stdout).unwrap()
}

fn snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(root: &Path, path: &Path, entries: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            let kind = fs::symlink_metadata(&path).unwrap();
            assert!(!kind.file_type().is_symlink());
            if kind.is_dir() {
                walk(root, &path, entries);
            } else {
                entries.insert(path.strip_prefix(root).unwrap().to_owned(), fs::read(path).unwrap());
            }
        }
    }
    let mut files = BTreeMap::new();
    walk(root, root, &mut files);
    files
}

#[tokio::test]
async fn initialized_orientation_is_shared_schema_five_context_without_writes() {
    let (_temp, config) = fixture().await;
    let before = snapshot(&config.base);
    let task = "Refactor dispatch without changing tenant ordering";
    let raw_onboard = success(invoke(
        &config,
        &["--json", "onboard", "--task", task, "--max-tokens", "6000", "--no-inspect", "--no-cache"],
    ));
    let raw_context = success(invoke(
        &config,
        &["--json", "context", task, "--max-tokens", "6000", "--no-inspect", "--no-cache"],
    ));
    let onboard: Value = serde_json::from_str(&raw_onboard).unwrap();
    let context: Value = serde_json::from_str(&raw_context).unwrap();

    assert_eq!(onboard["schema_version"], 5);
    assert_eq!(onboard["snapshot"], context["snapshot"]);
    assert_eq!(onboard["capabilities"], context["capabilities"]);
    assert_eq!(onboard["source_relationships"], context["source_relationships"]);
    assert_eq!(onboard["intelligence"], context["intelligence"]);
    assert_eq!(onboard["capabilities"]["inspection"], "disabled_by_caller");
    assert_eq!(onboard["capabilities"]["hosted_egress"], false);
    assert_eq!(onboard["capabilities"]["execution"], false);
    assert_eq!(onboard["capabilities"]["source_write"], false);
    assert!(onboard.get("tutorial").is_none());
    assert!(onboard.get("feedback").is_none());
    assert!(onboard.get("revision_key").is_none());
    assert!(!raw_onboard.contains("CHECKOUT_ONLY_ORIENTATION_SENTINEL"));
    assert!(raw_onboard.contains("dispatch") || raw_onboard.contains("tenant"));
    assert_eq!(snapshot(&config.base), before);
}

#[tokio::test]
async fn topic_and_default_orientation_share_context_budget_and_provenance() {
    let (_temp, config) = fixture().await;
    let before = snapshot(&config.base);
    for args in [
        vec!["--topic", "Explain dispatch", "--max-tokens", "6000"],
        vec!["--max-tokens", "6000"],
    ] {
        let mut command = vec!["--json", "onboard", "--no-cache", "--no-inspect"];
        command.extend(args.clone());
        let raw = success(invoke(&config, &command));
        let parsed: AdaptiveResult = serde_json::from_str(&raw).unwrap();
        let printed = success(invoke(
            &config,
            &{
                let mut args = vec!["onboard", "--no-cache", "--no-inspect"];
                args.extend(args);
                args
            },
        ));
        assert_eq!(parsed.schema_version, 5);
        assert_eq!(parsed.capabilities.inspection, "disabled_by_caller");
        assert!(!printed.contains("CHECKOUT_ONLY_ORIENTATION_SENTINEL"));
        let used = parsed.budget.used_tokens;
        assert!(context::count_tokens(&raw) <= used);
        assert!(context::count_tokens(&printed) <= used);
        assert!(used <= 6000);
        assert!(!printed.contains("Exercise:"));
        assert_eq!(parsed.snapshot.registry_revision, serde_json::from_value::<AdaptiveResult>(
            serde_json::from_str(&raw).unwrap()
        ).unwrap().snapshot.registry_revision);
    }
    assert_eq!(snapshot(&config.base), before);
}

#[test]
fn removed_learning_arguments_reject_before_configuration_access() {
    let temp = tempfile::tempdir().unwrap();
    let missing = temp.path().join("missing.yml");
    for args in [
        vec!["--mode", "tutorial"],
        vec!["--hint", "1"],
        vec!["--answer", "past answer"],
        vec!["--lesson", "blake3:past-lesson"],
        vec!["--show-solution"],
        vec!["--activity", "transfer"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_lore"))
            .args(["--json", "--config"])
            .arg(&missing)
            .arg("onboard")
            .args(&args)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        let error: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(error["code"], "invalid_arguments", "{args:?}");
        assert!(error["error"].as_str().unwrap().contains(args[0]), "{error}");
    }
    assert!(!missing.exists());
}
