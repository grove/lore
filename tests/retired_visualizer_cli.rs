//! The browser visualizer is retired; core CLI access and first contact remain.
//! These checks exercise the actual binary without initializing a registry.

use serde_json::Value;
use std::{fs, process::Command};

#[test]
fn retired_visualizer_fails_before_config_or_server_startup() {
    let project = tempfile::tempdir().unwrap();
    let before = fs::read_dir(project.path()).unwrap().count();
    for args in [
        vec!["--json", "visualize"],
        vec!["--json", "visualize", "--port", "4400", "--no-open"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_lore"))
            .current_dir(project.path())
            .args(&args)
            .env_remove("LORE_USAGE_LEDGER")
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(output.stderr.is_empty(), "{args:?}");
        let result: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(result["code"], "invalid_arguments");
        assert!(
            result["error"].as_str().unwrap().contains("visualize"),
            "removed subcommand should fail explicitly: {result}"
        );
        assert_eq!(fs::read_dir(project.path()).unwrap().count(), before);
    }
}

#[test]
fn retained_decision_evidence_and_safety_commands_remain_exposed() {
    let output = Command::new(env!("CARGO_BIN_EXE_lore"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).unwrap();
    for command in [
        "context", "evidence", "decisions", "cases", "review", "search",
        "status", "audit", "init", "update", "integrate",
    ] {
        assert!(
            help.lines()
                .any(|line| line.trim_start().starts_with(&format!("{command} "))),
            "CLI lost the required {command} entry point"
        );
    }
    assert!(
        !help.lines().any(|line| line.trim_start().starts_with("visualize ")),
        "retired browser command is still advertised"
    );
}
