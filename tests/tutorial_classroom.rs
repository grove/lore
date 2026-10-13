//! Offline, source-only first-contact smoke checks against a bundled practice project.
//! No provider, model, network request or registry compilation is required.
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

fn classroom() -> (tempfile::TempDir, PathBuf) {
    let temp = tempfile::tempdir().expect("temporary classroom");
    let root = temp.path().join("classroom");
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/tutorial-project");
    fs::create_dir_all(root.join("docs")).unwrap();
    for name in [
        "README.md",
        "docs/ADR-001.md",
        "docs/RFC-002.md",
        "docs/runbook.md",
        "docs/INC-014.md",
    ] {
        let destination = root.join(name);
        fs::copy(source.join(name), &destination).unwrap();
    }
    (temp, root)
}

fn invoke(root: &Path, args: &[&str]) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_lore"))
        .current_dir(root)
        .args(args)
        .env_remove("OPENAI_API_KEY")
        .env_remove("LORE_ALLOW_HOSTED_EGRESS")
        .env_remove("LORE_INSPECTION_ROOT")
        .env_remove("LORE_USAGE_LEDGER")
        .output()
        .expect("CLI invocation");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("JSON response")
}

#[test]
fn zero_setup_classroom_onboard_and_context_do_not_write_or_call_models() {
    let (_tmp, root) = classroom();
    let before = fs::read_dir(&root).unwrap().count();
    for (args, contract) in [
        (vec!["--json", "onboard"], "lore.bootstrap_context"),
        (
            vec![
                "--json",
                "context",
                "Can production attempts increase to five?",
            ],
            "lore.bootstrap_context",
        ),
    ] {
        let v = invoke(&root, &args);
        assert_eq!(v["contract"], contract);
        assert_eq!(v["schema_version"], 1);
        assert_eq!(v["mode"], "bootstrap_source_only");
        assert_eq!(v["model_calls"], 0);
        assert_eq!(v["source_write"], false);
        assert!(v["evidence"].as_array().is_some_and(|e| !e.is_empty()));
    }
    assert_eq!(fs::read_dir(&root).unwrap().count(), before);
    assert!(!root.join(".lore").exists());
    assert!(!root.join("lore.yml").exists());
}

#[test]
fn course_codex_integration_is_an_explicit_write_only() {
    let (_tmp, root) = classroom();
    let first = invoke(&root, &["--json", "integrate", "codex"]);
    assert_eq!(first["contract"], "lore.agent_integration");
    assert_eq!(first["source_write"], false);
    assert!(!root.join("AGENTS.md").exists());

    let applied = invoke(&root, &["--json", "integrate", "codex", "--apply"]);
    assert_eq!(applied["source_write"], true);
    let instructions = fs::read_to_string(root.join("AGENTS.md")).unwrap();
    assert!(instructions.contains("## Lore project context"));

    let repeated = invoke(&root, &["--json", "integrate", "codex", "--apply"]);
    assert_eq!(repeated["source_write"], false);
    assert!(!root.join("lore.yml").exists());
    assert!(!root.join(".lore").exists());

    let removal = invoke(
        &root,
        &["--json", "integrate", "codex", "--remove", "--apply"],
    );
    assert_eq!(removal["source_write"], true);
    assert_eq!(fs::read_to_string(root.join("AGENTS.md")).unwrap(), "");
}
