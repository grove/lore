mod common;

use common::*;
use lore::{
    config::{ProviderSettings, ResolvedConfig},
    engine::{self, UpdateOptions},
};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    io::ErrorKind,
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Command, Output},
    time::SystemTime,
};

fn run(config: &ResolvedConfig, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_lore"))
        .arg("--config")
        .arg(&config.config_path)
        .args(args)
        .env_remove("OPENAI_API_KEY")
        .env_remove("LORE_CONTEXT_TEST_API_KEY")
        .output()
        .unwrap()
}

fn success(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    serde_json::from_slice(&output.stdout).unwrap()
}

fn error(output: &Output, exit_code: i32, code: &str) {
    assert_eq!(
        output.status.code(),
        Some(exit_code),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["code"], code);
    assert!(!result["error"].as_str().unwrap().is_empty());
}

// Include names, bytes, and modification times so a query cannot silently
// refresh the database, republish a page, create a cache, or touch a source.
fn snapshot(base: &Path) -> BTreeMap<PathBuf, (Vec<u8>, SystemTime)> {
    fn visit(base: &Path, dir: &Path, files: &mut BTreeMap<PathBuf, (Vec<u8>, SystemTime)>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let metadata = fs::metadata(&path).unwrap();
            if metadata.is_dir() {
                visit(base, &path, files);
            } else {
                files.insert(
                    path.strip_prefix(base).unwrap().to_owned(),
                    (fs::read(&path).unwrap(), metadata.modified().unwrap()),
                );
            }
        }
    }
    let mut files = BTreeMap::new();
    visit(base, base, &mut files);
    files
}

async fn compiled_project() -> (tempfile::TempDir, ResolvedConfig, FakeModel) {
    let (temp, cfg, model) = project();
    put(
        &cfg,
        "ADR-001.md",
        "DECISION payments: Payment retries preserve transaction idempotency.\n",
    );
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    (temp, cfg, model)
}

#[test]
fn json_argument_errors_work_before_configuration_is_loaded() {
    let (_temp, cfg, _model) = project();
    fs::remove_file(&cfg.config_path).unwrap();
    for args in [
        vec!["--json", "context"],
        vec!["context", "--json"],
        vec!["--json", "context", ""],
        vec!["context", " \t ", "--json"],
        vec!["--json", "context", "retries", "--max-tokens", "0"],
        vec!["--json", "context", "retries", "--max-tokens", "255"],
        vec!["--json", "context", "retries", "--max-tokens", "100001"],
        vec!["--json", "context", "retries", "--max-tokens", "many"],
        vec!["--json", "context", "retries", "--max-tokens", "-1"],
        vec!["--json", "context", "retries", "--path"],
        vec!["--json", "context", "retries", "--unknown"],
    ] {
        error(&run(&cfg, &args), 2, "invalid_arguments");
    }
    assert!(!cfg.state.exists());
    assert!(!cfg.wiki.exists());
}

#[test]
fn missing_registry_is_an_actionable_json_error_without_initialization() {
    let (_temp, cfg, _model) = project();
    let before = snapshot(&cfg.base);
    let output = run(&cfg, &["--json", "context", "Implement payment retries"]);
    error(&output, 1, "operation_failed");
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(result["error"].as_str().unwrap().contains("lore init"));
    assert_eq!(before, snapshot(&cfg.base));
    assert!(!cfg.state.exists());
}

#[tokio::test]
async fn context_validation_errors_keep_the_typed_json_contract() {
    let (_temp, cfg, _model) = compiled_project().await;
    let before = snapshot(&cfg.base);
    for args in [
        vec!["--json", "context", "!!!"],
        vec!["--json", "context", "Payment\nretries"],
        vec!["--json", "context", "Payment retries", "--path", ""],
    ] {
        error(&run(&cfg, &args), 2, "invalid_query");
    }
    let long_task = "retries ".repeat(500);
    error(
        &run(
            &cfg,
            &["--json", "context", &long_task, "--max-tokens", "512"],
        ),
        2,
        "invalid_budget",
    );
    assert_eq!(before, snapshot(&cfg.base));
}

#[tokio::test]
async fn repeated_context_is_read_only_offline_and_has_resolvable_evidence() {
    let (_temp, cfg, _model) = compiled_project().await;
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let mut config = cfg.config.clone();
    config.processing.timeout_seconds = 1;
    config.processing.retry_attempts = 0;
    config.providers.insert(
        "ollama".into(),
        ProviderSettings {
            base_url: Some(format!("http://{}", listener.local_addr().unwrap())),
            api_key_env: Some("LORE_CONTEXT_TEST_API_KEY".into()),
        },
    );
    fs::write(&cfg.config_path, serde_yaml::to_string(&config).unwrap()).unwrap();
    let before = snapshot(&cfg.base);
    let args = [
        "context",
        "Implement payment retries",
        "--path",
        "src/payments/retry.rs",
        "--path",
        "docs/ADR-001.md",
        "--max-tokens",
        "3000",
        "--json",
    ];
    let first = run(&cfg, &args);
    let result = success(&first);
    assert_eq!(result["schema_version"], 1);
    assert_eq!(result["model_calls"], 0);
    assert_eq!(result["empty"], false);
    assert!(String::from_utf8_lossy(&first.stdout).contains("transaction idempotency"));
    let second = run(&cfg, &args);
    success(&second);
    assert_eq!(first.stdout, second.stdout);

    let evidence = result["evidence"].as_array().unwrap();
    assert!(!evidence.is_empty());
    for citation in evidence {
        let id = citation["id"].as_str().unwrap();
        let passage = success(&run(&cfg, &["--json", "evidence", id]));
        assert!(
            passage.to_string().contains("transaction idempotency"),
            "unresolvable or unrelated citation: {id}"
        );
    }
    assert_eq!(before, snapshot(&cfg.base));
    assert_eq!(listener.accept().unwrap_err().kind(), ErrorKind::WouldBlock);
}

#[tokio::test]
async fn human_and_json_output_fit_the_named_tokenizer_budget() {
    let (_temp, cfg, _model) = compiled_project().await;
    let tokenizer = tiktoken_rs::cl100k_base().unwrap();
    for budget in [512, 1000, 3000] {
        let budget_arg = budget.to_string();
        let machine = run(
            &cfg,
            &[
                "--json",
                "context",
                "Implement payment retries",
                "--max-tokens",
                &budget_arg,
            ],
        );
        let result = success(&machine);
        let human = run(
            &cfg,
            &[
                "context",
                "Implement payment retries",
                "--max-tokens",
                &budget_arg,
            ],
        );
        assert!(human.status.success());
        assert!(human.stderr.is_empty());
        let machine_tokens = tokenizer
            .encode_ordinary(std::str::from_utf8(&machine.stdout).unwrap())
            .len();
        let human_tokens = tokenizer
            .encode_ordinary(std::str::from_utf8(&human.stdout).unwrap())
            .len();
        assert_eq!(result["budget"]["max_tokens"], budget);
        assert_eq!(result["budget"]["tokenizer"], "cl100k_base");
        assert!(machine_tokens <= budget);
        assert!(human_tokens <= budget);
        let reported = result["budget"]["used_tokens"].as_u64().unwrap() as usize;
        assert!(reported >= machine_tokens.max(human_tokens));
        assert!(reported <= budget);
    }
}

#[tokio::test]
async fn context_does_not_require_working_inference_configuration_or_live_sources() {
    let (_temp, cfg, _model) = compiled_project().await;
    let mut config = cfg.config.clone();
    config.models.generative.enabled = false;
    config.models.generative.provider = "provider-no-longer-configured".into();
    config.models.generative.model.clear();
    config.processing.timeout_seconds = 0;
    fs::write(&cfg.config_path, serde_yaml::to_string(&config).unwrap()).unwrap();
    // Retrieval uses the last compiled snapshot. It does not scan source files
    // or silently change support status when the caller is offline.
    fs::remove_dir_all(cfg.base.join("docs")).unwrap();
    let before = snapshot(&cfg.base);
    let output = run(&cfg, &["--json", "context", "Payment retries"]);
    let result = success(&output);
    assert_eq!(result["model_calls"], 0);
    assert_eq!(result["empty"], false);
    assert!(String::from_utf8_lossy(&output.stdout).contains("transaction idempotency"));
    for citation in result["evidence"].as_array().unwrap() {
        success(&run(
            &cfg,
            &["--json", "evidence", citation["id"].as_str().unwrap()],
        ));
    }
    assert_eq!(before, snapshot(&cfg.base));
}

#[tokio::test]
async fn unmatched_task_returns_a_successful_explicit_empty_result() {
    let (_temp, cfg, _model) = compiled_project().await;
    let before = snapshot(&cfg.base);
    let output = run(&cfg, &["--json", "context", "zygomorphic xylophony"]);
    let result = success(&output);
    assert_eq!(result["empty"], true);
    assert_eq!(result["model_calls"], 0);
    assert!(result["evidence"].as_array().unwrap().is_empty());
    assert_eq!(before, snapshot(&cfg.base));
}

#[tokio::test]
async fn pending_publication_is_not_recovered_or_read_by_context() {
    let (_temp, cfg, _model) = compiled_project().await;
    fs::write(cfg.state.join("publication.json"), "{\"pending\":true}").unwrap();
    let before = snapshot(&cfg.base);
    let output = run(&cfg, &["--json", "context", "Payment retries"]);
    error(&output, 1, "operation_failed");
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(result["error"].as_str().unwrap().contains("lore update"));
    assert_eq!(before, snapshot(&cfg.base));
}
