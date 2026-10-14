use serde_json::Value;
use std::{fs, process::Command};

#[test]
fn removed_schemas_fail_before_configuration_registry_or_usage_access() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("lore.yml"), "invalid: [").unwrap();
    fs::create_dir_all(dir.path().join(".lore/context-cache")).unwrap();
    for (path, bytes) in [
        (".lore/state.db", "not a database"),
        (".lore/context-cache/old.json", "old disposable guidance"),
        ("README.md", "Original documentary evidence"),
    ] {
        fs::write(dir.path().join(path), bytes).unwrap();
    }
    for schema in ["3", "4"] {
        for flag in [
            None,
            Some("--inspect"),
            Some("--investigate"),
            Some("--no-inspect"),
        ] {
            for json in [false, true] {
                let mut command = Command::new(env!("CARGO_BIN_EXE_lore"));
                command
                    .current_dir(dir.path())
                    .env("LORE_USAGE_LEDGER", dir.path().join("usage.jsonl"))
                    .args(["context", "Change retries", "--schema-version", schema]);
                if let Some(flag) = flag {
                    command.arg(flag);
                }
                if json {
                    command.arg("--json");
                }
                let output = command.output().unwrap();
                assert_eq!(output.status.code(), Some(2));
                let message = if json {
                    assert!(output.stderr.is_empty());
                    let error: Value = serde_json::from_slice(&output.stdout).unwrap();
                    assert_eq!(error["code"], "invalid_arguments");
                    error["error"].as_str().unwrap().to_owned()
                } else {
                    assert!(output.stdout.is_empty());
                    String::from_utf8(output.stderr).unwrap()
                };
                for expected in [
                    "removed in 0.10.0",
                    "--schema-version 5",
                    "intelligence",
                    "--fast",
                    "CONTEXT_MIGRATION.md",
                ] {
                    assert!(message.contains(expected), "{message}");
                }
            }
        }
    }
    assert!(!dir.path().join("usage.jsonl").exists());
    assert!(!dir.path().join(".lore/decision-cache").exists());
    for (path, bytes) in [
        ("lore.yml", "invalid: ["),
        (".lore/state.db", "not a database"),
        (".lore/context-cache/old.json", "old disposable guidance"),
        ("README.md", "Original documentary evidence"),
    ] {
        assert_eq!(fs::read_to_string(dir.path().join(path)).unwrap(), bytes);
    }
}

#[test]
fn help_advertises_retained_contract_and_breaking_release() {
    let binary = env!("CARGO_BIN_EXE_lore");
    let help = Command::new(binary)
        .args(["context", "--help"])
        .output()
        .unwrap();
    assert!(help.status.success());
    let text = String::from_utf8(help.stdout).unwrap();
    assert!(text.contains("JSON contract: 5 (default)"));
    assert!(text.contains("schemas 3/4 were removed in 0.10.0"));
    let version = Command::new(binary).arg("--version").output().unwrap();
    assert!(version.status.success());
    assert_eq!(
        String::from_utf8(version.stdout).unwrap().trim(),
        "lore 0.10.0"
    );
}
