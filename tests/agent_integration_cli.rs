mod common;
use serde_json::Value;
use std::{fs, path::Path, process::Command};

fn invoke(root: &Path, args: &[&str]) -> (bool, Value) {
    let output = Command::new(env!("CARGO_BIN_EXE_lore"))
        .current_dir(root)
        .arg("--json")
        .args(args)
        .env_remove("LORE_USAGE_LEDGER")
        .env_remove("LORE_INSPECTION_ROOT")
        .env_remove("LORE_ALLOW_HOSTED_EGRESS")
        .env_remove("LORE_ALLOW_CHECKOUT_EGRESS")
        .output()
        .unwrap();
    (
        output.status.success(),
        serde_json::from_slice(&output.stdout).unwrap(),
    )
}

#[test]
fn preview_apply_status_reapply_and_remove_preserve_original_bytes() {
    for (agent, name) in [("codex", "AGENTS.md"), ("claude-code", "CLAUDE.md")] {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let original = "# Existing instructions\r\nPreserve café exactly, without a final newline.";
        fs::write(root.join(name), original).unwrap();
        let (ok, preview) = invoke(&root, &["integrate", agent]);
        assert!(ok, "{preview}");
        assert_eq!(preview["contract"], "lore.agent_integration");
        assert_eq!(preview["managed_version"], 2);
        assert_eq!(preview["source_write"], false);
        assert_eq!(fs::read(root.join(name)).unwrap(), original.as_bytes());
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
        let (ok, result) = invoke(
            &root,
            &[
                "integrate",
                agent,
                "--apply",
                "--expect-digest",
                preview["before_digest"].as_str().unwrap(),
            ],
        );
        assert!(ok, "{result}");
        assert_eq!(result["source_write"], true);
        let installed = fs::read(root.join(name)).unwrap();
        let (_, result) = invoke(&root, &["integrate", agent, "--apply"]);
        assert_eq!(result["changed"], false);
        assert_eq!(fs::read(root.join(name)).unwrap(), installed);
        let (_, status) = invoke(&root, &["integrate", agent, "--status"]);
        assert_eq!(status["installed"], true);
        assert_eq!(status["source_write"], false);
        let (_, preview) = invoke(&root, &["integrate", agent, "--remove"]);
        assert_eq!(preview["changed"], true);
        assert_eq!(fs::read(root.join(name)).unwrap(), installed);
        assert!(invoke(&root, &["integrate", agent, "--remove", "--apply"]).0);
        assert_eq!(fs::read(root.join(name)).unwrap(), original.as_bytes());
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
    }
}

#[test]
fn missing_target_and_stale_preview_do_not_clobber_instructions() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let (_, preview) = invoke(&root, &["integrate", "codex"]);
    assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
    fs::write(root.join("AGENTS.md"), "concurrent instructions").unwrap();
    assert!(
        !invoke(
            &root,
            &[
                "integrate",
                "codex",
                "--apply",
                "--expect-digest",
                preview["before_digest"].as_str().unwrap()
            ]
        )
        .0
    );
    assert_eq!(
        fs::read_to_string(root.join("AGENTS.md")).unwrap(),
        "concurrent instructions"
    );
    assert!(invoke(&root, &["integrate", "claude-code", "--apply"]).0);
    assert!(!root.join("lore.yml").exists());
    assert!(!root.join(".lore").exists());
}

#[test]
fn modified_duplicate_malformed_oversized_and_nonregular_targets_fail_closed() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let (_, preview) = invoke(&root, &["integrate", "codex"]);
    let block = preview["managed_block"].as_str().unwrap();
    for text in [
        block.replace("3000", "9999"),
        block.repeat(2),
        "<!-- lore:agent-context:start -->".into(),
        "x".repeat(1024 * 1024 + 1),
    ] {
        fs::write(root.join("AGENTS.md"), &text).unwrap();
        for args in [
            vec!["integrate", "codex", "--apply"],
            vec!["integrate", "codex", "--remove", "--apply"],
            vec!["integrate", "codex", "--status"],
        ] {
            assert!(!invoke(&root, &args).0);
            assert_eq!(fs::read_to_string(root.join("AGENTS.md")).unwrap(), text);
        }
    }
    fs::remove_file(root.join("AGENTS.md")).unwrap();
    fs::create_dir(root.join("AGENTS.md")).unwrap();
    assert!(!invoke(&root, &["integrate", "codex", "--apply"]).0);
}

#[test]
fn ambiguous_root_explicit_configuration_and_overlapping_install_fail_closed() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    fs::create_dir(root.join(".git")).unwrap();
    fs::create_dir(root.join("nested")).unwrap();
    assert!(!invoke(&root.join("nested"), &["integrate", "codex", "--apply"]).0);
    assert!(!invoke(&root, &["--config", "../lore.yml", "integrate", "codex"]).0);
    fs::write(root.join(".lore-agent-context.lock"), "other installer").unwrap();
    assert!(!invoke(&root, &["integrate", "codex", "--apply"]).0);
    assert!(!root.join("AGENTS.md").exists());
    assert_eq!(
        fs::read_to_string(root.join(".lore-agent-context.lock")).unwrap(),
        "other installer"
    );
}

#[cfg(unix)]
#[test]
fn symlink_target_is_never_followed() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    fs::write(root.join("outside"), "private instructions").unwrap();
    std::os::unix::fs::symlink(root.join("outside"), root.join("AGENTS.md")).unwrap();
    assert!(!invoke(&root, &["integrate", "codex", "--apply"]).0);
    assert_eq!(
        fs::read_to_string(root.join("outside")).unwrap(),
        "private instructions"
    );
}

#[test]
fn installation_ignores_configuration_sources_grants_and_providers() {
    let (_temp, config, _model) = common::project();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let mut settings = config.config.clone();
    settings.providers.insert(
        "ollama".into(),
        lore::config::ProviderSettings {
            base_url: Some(format!("http://{}", listener.local_addr().unwrap())),
            api_key_env: None,
        },
    );
    fs::write(
        &config.config_path,
        serde_yaml::to_string(&settings).unwrap(),
    )
    .unwrap();
    // These are deliberately unusable source/checkout inputs, not install inputs.
    fs::write(config.base.join("docs/private.md"), [0xff]).unwrap();
    fs::write(config.base.join("private.rs"), [0xff]).unwrap();
    let configuration = fs::read(&config.config_path).unwrap();
    let ledger = config.base.join("usage.jsonl");
    for agent in ["codex", "claude-code"] {
        for args in [
            vec!["integrate", agent],
            vec!["integrate", agent, "--apply"],
            vec!["integrate", agent, "--apply"],
            vec!["integrate", agent, "--status"],
            vec!["integrate", agent, "--remove", "--apply"],
        ] {
            let output = Command::new(env!("CARGO_BIN_EXE_lore"))
                .current_dir(&config.base)
                .arg("--json")
                .args(&args)
                .env("LORE_USAGE_LEDGER", &ledger)
                .env_remove("LORE_INSPECTION_ROOT")
                .env_remove("LORE_ALLOW_HOSTED_EGRESS")
                .env_remove("LORE_ALLOW_CHECKOUT_EGRESS")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stdout)
            );
            let usage: Value = serde_json::from_slice(&fs::read(&ledger).unwrap()).unwrap();
            assert_eq!(usage["summary"]["model_calls"], 0);
            assert_eq!(usage["summary"]["provider_request_count"], 0);
            assert_eq!(usage["events"], serde_json::json!([]));
            fs::remove_file(&ledger).unwrap();
            assert!(!config.state.exists());
            assert_eq!(fs::read(&config.config_path).unwrap(), configuration);
            assert_eq!(
                fs::read(config.base.join("docs/private.md")).unwrap(),
                [0xff]
            );
            assert_eq!(fs::read(config.base.join("private.rs")).unwrap(), [0xff]);
            assert_eq!(
                listener.accept().unwrap_err().kind(),
                std::io::ErrorKind::WouldBlock
            );
        }
    }
    // Integration even works when loading any configuration would fail.
    fs::write(&config.config_path, [0xff]).unwrap();
    assert!(invoke(&config.base, &["integrate", "codex", "--apply"]).0);
}

#[test]
fn command_capable_test_double_consumes_bootstrap_and_unavailable_fallback() {
    // This is a deterministic command adapter, not a real Codex/Claude experiment.
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    fs::write(
        root.join("README.md"),
        "# Retries\nNever retry a declined payment.\n",
    )
    .unwrap();
    assert!(invoke(&root, &["integrate", "codex", "--apply"]).0);
    let block = fs::read_to_string(root.join("AGENTS.md")).unwrap();
    assert!(block.contains("lore --json context \"<the intended change>\" --max-tokens 3000"));
    let (ok, result) = invoke(
        &root,
        &["context", "Change retries", "--max-tokens", "3000"],
    );
    assert!(ok, "{result}");
    assert_eq!(result["contract"], "lore.bootstrap_context");
    assert_eq!(result["model_calls"], 0);
    assert!(Command::new(root.join("missing-lore")).output().is_err());
    assert!(block.contains("use original"));
    assert!(block.contains(
        "Missing, stale, denied or omitted evidence does not prove constraints are absent"
    ));
}

#[tokio::test]
async fn command_adapter_handles_initialized_denied_and_stale_context_without_initializing() {
    let (_temp, config, model) = common::project();
    common::put(
        &config,
        "rules.md",
        "DECISION payments: Payment retries preserve idempotency.\n",
    );
    lore::engine::update(
        &config,
        &model,
        None,
        lore::engine::UpdateOptions::default(),
    )
    .await
    .unwrap();
    let mut settings = config.config.clone();
    settings.models.generative.enabled = false;
    fs::write(
        &config.config_path,
        serde_yaml::to_string(&settings).unwrap(),
    )
    .unwrap();
    assert!(invoke(&config.base, &["integrate", "codex", "--apply"]).0);
    let database = fs::read(config.state.join("state.db")).unwrap();
    let (ok, original) = invoke(
        &config.base,
        &[
            "context",
            "Change payment retries",
            "--no-inspect",
            "--no-cache",
            "--max-tokens",
            "3000",
        ],
    );
    assert!(ok, "{original}");
    assert_eq!(original["schema_version"], 5);
    assert!(original["capabilities"].is_object());
    assert!(original["snapshot"].is_object());
    assert_eq!(original["intelligence"]["mode"], "fast_fallback");
    common::put(
        &config,
        "rules.md",
        "DECISION payments: Never retry declined payments.\n",
    );
    let (ok, stale) = invoke(
        &config.base,
        &[
            "context",
            "Change payment retries",
            "--no-inspect",
            "--no-cache",
            "--max-tokens",
            "3000",
        ],
    );
    assert!(ok, "{stale}");
    // A context call must not silently refresh stale documentary state.
    assert_eq!(original["snapshot"], stale["snapshot"]);
    assert_eq!(fs::read(config.state.join("state.db")).unwrap(), database);
}
