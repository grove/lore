//! Reviewed task notes are ordinary documentary sources, never auto-approved
//! task receipts. Original revisions and reported/accepted authority stay distinct.
mod common;

use common::*;
use lore::{
    context::{self, ContextOptions},
    engine::{self, UpdateOptions},
    storage,
};
use std::{fs, process::Command, sync::atomic::Ordering};

#[tokio::test]
async fn reviewed_task_note_is_ordinary_reported_evidence_and_withdrawal_keeps_history() {
    let (_tmp, cfg, model) = project();
    put(
        &cfg,
        "guide.md",
        "DECISION queue: Preserve the approved retry limit.\n",
    );
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();

    // A receipt created by an old producer remains caller-owned data.
    // Nothing automatically parses or authorizes its embedded assertions.
    let original_receipt =
        br#"{"contract":"lore.task_outcome","schema_version":1,"reported_checks":{"passed":9}}"#;
    let receipt_path = cfg.base.join("task-outcome.json");
    fs::write(&receipt_path, original_receipt).unwrap();

    // A maintainer writes and reviews a normal Markdown source explicitly.
    // "REPORT" is work history, not an accepted decision or runtime proof.
    let note_path = cfg.base.join("docs/task-note.md");
    let note_text = "# Retry diagnostics investigation\n\nREPORT queue: A team reports a staging retry test passed; production checks and independent approval are still unverified.\n";
    fs::write(&note_path, note_text).unwrap();
    let calls_before = model.calls.load(Ordering::SeqCst);
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    assert!(model.calls.load(Ordering::SeqCst) > calls_before);
    assert_eq!(fs::read(&receipt_path).unwrap(), original_receipt);
    assert_eq!(fs::read_to_string(&note_path).unwrap(), note_text);

    let conn = storage::read_only(&cfg.state.join("state.db")).unwrap();
    let views = storage::views(&conn).unwrap();
    let note = views
        .iter()
        .find(|v| v.kind == "reported_outcome")
        .expect("ordinary Markdown task report was not ingested");
    assert_eq!(note.lifecycle, "completed");
    assert_ne!(note.kind, "decision");
    assert!(
        note.statement
            .contains("independent approval are still unverified")
    );
    let evidence = note.evidence[0].id.clone();
    let captured = storage::evidence_snapshot(&conn, &evidence).unwrap();
    assert!(captured.excerpt.contains("REPORT queue:"));
    assert!(captured.path.ends_with("task-note.md"));

    let decision = views
        .iter()
        .find(|v| v.kind == "decision")
        .expect("accepted decision disappeared");
    assert_eq!(decision.lifecycle, "accepted");
    assert!(decision.statement.contains("approved retry limit"));

    let ctx = context::build_context(
        &conn,
        &ContextOptions {
            task: "Check retry diagnostics against approved policy".into(),
            paths: vec![],
            max_tokens: 8_000,
        },
    )
    .unwrap();
    assert!(
        serde_json::to_string(&ctx).unwrap().contains(&evidence),
        "the reported work should remain accessible through task context"
    );
    drop(conn);

    // Withdrawing a reviewed note removes its current support, not its
    // original source revision or evidence provenance.
    fs::remove_file(&note_path).unwrap();
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let conn = storage::read_only(&cfg.state.join("state.db")).unwrap();
    assert!(storage::evidence_snapshot(&conn, &evidence).is_ok());
    assert!(
        !storage::views(&conn)
            .unwrap()
            .iter()
            .flat_map(|v| &v.evidence)
            .any(|e| e.id == evidence && e.active)
    );
    assert_eq!(fs::read(&receipt_path).unwrap(), original_receipt);
}

#[test]
fn removed_feedback_drafting_rejects_without_configuration_or_file_writes() {
    let project = tempfile::tempdir().unwrap();
    for args in [
        vec![
            "--json",
            "feedback",
            "draft",
            "--receipt",
            "task-outcome.json",
        ],
        vec![
            "--json",
            "feedback",
            "draft",
            "--receipt",
            "task-outcome.json",
            "--output",
            "docs/task-note.md",
            "--write",
        ],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_lore"))
            .current_dir(project.path())
            .args(&args)
            .env_remove("LORE_USAGE_LEDGER")
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(output.stderr.is_empty(), "{args:?}");
        let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(result["code"], "invalid_arguments");
        assert!(
            result["error"].as_str().unwrap().contains("feedback"),
            "removed receipt drafting must fail explicitly"
        );
        assert_eq!(fs::read_dir(project.path()).unwrap().count(), 0);
    }

    let help = Command::new(env!("CARGO_BIN_EXE_lore"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(help.status.success());
    assert!(
        !String::from_utf8(help.stdout)
            .unwrap()
            .lines()
            .any(|line| line.split_whitespace().next() == Some("feedback"))
    );
}
