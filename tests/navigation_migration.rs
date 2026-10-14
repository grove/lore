mod common;

use lore::{
    config::{OutputMode, ResolvedConfig},
    context::{self, ContextResult},
    engine::{self, UpdateOptions},
    insights::{CaseResult, DecisionLenses},
    storage,
};
use serde_json::Value;
use std::{
    fs,
    path::Path,
    process::{Command, Output},
    sync::atomic::Ordering,
};

fn invoke(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_lore"))
        .current_dir(root)
        .args(args)
        .env_remove("OPENAI_API_KEY")
        .env_remove("LORE_USAGE_LEDGER")
        .output()
        .unwrap()
}

fn json(root: &Path, args: &[&str]) -> Value {
    let output = invoke(root, &[&["--json"], args].concat());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn retired_commands_and_response_forms_fail_before_project_or_usage_access() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("lore.yml"), "invalid: [").unwrap();
    fs::create_dir_all(dir.path().join(".lore/knowledge-zoom")).unwrap();
    fs::create_dir(dir.path().join("lore")).unwrap();
    for (path, text) in [
        (".lore/state.db", "not a database"),
        (".lore/knowledge-zoom/snapshot.json", "old cache"),
        (".lore/knowledge-zoom/notes.txt", "user note"),
        ("lore/index.md", "old publication"),
    ] {
        fs::write(dir.path().join(path), text).unwrap();
    }
    for args in [
        vec!["read"],
        vec!["read", "index"],
        vec!["read", "--help"],
        vec!["explore"],
        vec!["explore", "QUEUE_CAPACITY", "--no-cache"],
        vec![
            "explore",
            "--node",
            "kv_old",
            "--compact",
            "--max-tokens",
            "1500",
            "--max-nodes",
            "24",
        ],
        vec!["explore", "--help"],
    ] {
        for json_position in [None, Some("before"), Some("after")] {
            let mut command = Command::new(env!("CARGO_BIN_EXE_lore"));
            command
                .current_dir(dir.path())
                .env("LORE_USAGE_LEDGER", dir.path().join("usage.jsonl"));
            if json_position == Some("before") {
                command.arg("--json");
            }
            command.args(&args);
            if json_position == Some("after") {
                command.arg("--json");
            }
            let output = command.output().unwrap();
            assert_eq!(output.status.code(), Some(2), "{args:?}");
            let message = if json_position.is_some() {
                assert!(output.stderr.is_empty());
                let error: Value = serde_json::from_slice(&output.stdout).unwrap();
                assert_eq!(error["code"], "command_retired");
                assert_eq!(error["command"], args[0]);
                error["error"].as_str().unwrap().to_owned()
            } else {
                assert!(output.stdout.is_empty());
                String::from_utf8(output.stderr).unwrap()
            };
            for replacement in ["context", "search", "decisions", "cases", "evidence"] {
                assert!(message.contains(replacement), "{message}");
            }
        }
    }
    assert!(!dir.path().join("usage.jsonl").exists());
    assert_eq!(
        fs::read_to_string(dir.path().join(".lore/state.db")).unwrap(),
        "not a database"
    );
    assert_eq!(
        fs::read_to_string(dir.path().join(".lore/knowledge-zoom/snapshot.json")).unwrap(),
        "old cache"
    );
    assert_eq!(
        fs::read_to_string(dir.path().join(".lore/knowledge-zoom/notes.txt")).unwrap(),
        "user note"
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("lore/index.md")).unwrap(),
        "old publication"
    );
    let help = invoke(dir.path(), &["--help"]);
    assert!(help.status.success());
    let help = String::from_utf8(help.stdout).unwrap();
    assert!(
        !help
            .lines()
            .any(|line| line.trim_start().starts_with("explore ")
                || line.trim_start().starts_with("read "))
    );
}

#[tokio::test]
async fn original_decisions_cases_and_retired_source_evidence_remain_reachable() {
    for mode in [OutputMode::RegistryOnly, OutputMode::Markdown] {
        let (_dir, mut config, model) = common::project();
        config.config.output.mode = Some(mode);
        config = ResolvedConfig::resolve(config.config.clone(), &config.config_path).unwrap();
        fs::write(
            &config.config_path,
            serde_yaml::to_string(&config.config).unwrap(),
        )
        .unwrap();
        common::put(
            &config,
            "queue.md",
            "# Queue\nDECISION queue: QUEUE_CAPACITY is 128; emergency drains alone may bypass normal admission.\nREPORT queue: The queue incident reported a failed drain.\n",
        );
        engine::update(&config, &model, None, UpdateOptions::default())
            .await
            .unwrap();
        let conn = storage::read_only(&config.state.join("state.db")).unwrap();
        let originals = storage::views(&conn).unwrap();
        assert_eq!(originals.len(), 2);
        drop(conn);
        fs::remove_file(config.base.join("docs/queue.md")).unwrap();
        engine::update(&config, &model, None, UpdateOptions::default())
            .await
            .unwrap();
        let before = fs::read(config.state.join("state.db")).unwrap();
        let calls = model.calls.load(Ordering::SeqCst);
        let discovery = json(&config.base, &["search", "queue"]);
        let selected: ContextResult = serde_json::from_value(json(
            &config.base,
            &[
                "context",
                "QUEUE_CAPACITY queue",
                "--fast",
                "--max-tokens",
                "12000",
            ],
        ))
        .unwrap();
        let decisions: DecisionLenses = serde_json::from_value(json(
            &config.base,
            &["decisions", "queue", "--max-tokens", "12000"],
        ))
        .unwrap();
        let cases: CaseResult = serde_json::from_value(json(
            &config.base,
            &["cases", "queue", "--max-tokens", "12000"],
        ))
        .unwrap();
        for original in &originals {
            assert!(
                discovery["results"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|r| r["id"] == original.id)
            );
            let retained = selected
                .sections
                .items()
                .find(|r| r.id == original.id)
                .unwrap();
            assert_eq!(retained.statement, original.statement);
            assert_eq!(retained.support_state, "historical_only");
            if original.kind == "decision" {
                assert!(
                    decisions
                        .lenses
                        .iter()
                        .any(|lens| lens.record.id == original.id)
                );
            } else {
                assert!(
                    cases
                        .cases
                        .iter()
                        .any(|case| case.record.as_ref().is_some_and(|r| r.id == original.id))
                );
            }
            for citation in &original.evidence {
                let evidence = json(&config.base, &["evidence", &citation.id]);
                assert_eq!(evidence["excerpt"], citation.excerpt);
                assert_eq!(evidence["digest"], lore::util::digest(&citation.excerpt));
            }
        }
        assert!(context::count_tokens(&serde_json::to_string(&selected).unwrap()) <= 12000);
        assert_eq!(before, fs::read(config.state.join("state.db")).unwrap());
        assert_eq!(calls, model.calls.load(Ordering::SeqCst));
        assert!(!config.state.join("knowledge-zoom").exists());
    }
}
