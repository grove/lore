mod common;
use common::*;
use lore::{
    context::{self, adaptive::SnapshotManifest},
    engine::{self, UpdateOptions},
    feedback, sources, storage, util,
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, process::Command};
async fn setup() -> (
    tempfile::TempDir,
    lore::config::ResolvedConfig,
    FakeModel,
    Value,
) {
    let (tmp, cfg, model) = project();
    put(
        &cfg,
        "guide.md",
        "DECISION queue: Preserve the retry limit.\n",
    );
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let conn = storage::read_only(&cfg.state.join("state.db")).unwrap();
    let ev = storage::views(&conn).unwrap()[0].evidence[0].id.clone();
    let source_files: BTreeMap<_, _> = sources::scan(&cfg)
        .unwrap()
        .documents
        .iter()
        .map(|d| {
            (
                format!("{}:{}", d.root_id, d.relative_path),
                d.digest.clone(),
            )
        })
        .collect();
    fs::write(cfg.base.join("change.txt"), "changed implementation").unwrap();
    let receipt = json!({"contract":"lore.task_outcome","schema_version":1,"task_id":"task-1","scope":["queue"],"snapshot":SnapshotManifest::capture(&cfg,&conn).unwrap(),"source_files":source_files,"evidence_ids":[ev],"changes":[{"path":"change.txt","before":null,"after":util::digest("changed implementation")}],"checks":[{"event_id":"attempt-1","checker_id":"checker-1","argv_digest":util::json_digest(&["test","queue"]).unwrap(),"exit_status":1,"log_digest":util::digest("private log")},{"event_id":"attempt-2","checker_id":"checker-1","argv_digest":util::json_digest(&["test","queue"]).unwrap(),"exit_status":0,"log_digest":null}],"started_at":"2026-10-11T09:00:00Z","finished_at":null,"usage":{"elapsed_ms":null,"model_calls":null,"input_tokens":null,"output_tokens":null,"cost_usd":null},"notes":[{"kind":"failure","text":"The retry path failed."},{"kind":"repair","text":"Added a bounded retry."}],"actor":{"kind":"agent","id":"caller-1"},"grants":{"inspected_files":["change.txt"],"network":false,"execution":true},"attestations":[]});
    (tmp, cfg, model, receipt)
}
fn save(cfg: &lore::config::ResolvedConfig, value: &Value) -> std::path::PathBuf {
    let path = cfg.base.join("receipt.json");
    fs::write(&path, serde_json::to_vec(value).unwrap()).unwrap();
    path
}
#[tokio::test]
async fn deterministic_read_only_drafts_keep_reports_and_byte_checks_separate() {
    let (_tmp, cfg, model, receipt) = setup().await;
    let path = save(&cfg, &receipt);
    let before = fs::read(cfg.state.join("state.db")).unwrap();
    let calls = model.calls.load(std::sync::atomic::Ordering::SeqCst);
    let draft = feedback::draft(&cfg, &path, &[], None, false).unwrap();
    assert_eq!(
        draft,
        feedback::draft(&cfg, &path, &[], None, false).unwrap()
    );
    assert_eq!(draft["verification"], "self_reported");
    assert_eq!(draft["usage"]["cost_usd"], Value::Null);
    assert_eq!(draft["reported_checks"]["passing"], 1);
    assert_eq!(draft["reported_checks"]["failing"], 1);
    assert_eq!(draft["unverified_changes"], 1);
    assert_eq!(draft["stale"], false);
    let checked = feedback::draft(&cfg, &path, &["change.txt".into()], None, false).unwrap();
    assert_eq!(
        checked["verified_changes"][0]["verification"],
        "current_bytes_only_not_check_execution"
    );
    assert_eq!(checked["authenticated_external_results"], 0);
    assert_eq!(before, fs::read(cfg.state.join("state.db")).unwrap());
    assert_eq!(calls, model.calls.load(std::sync::atomic::Ordering::SeqCst));
    fs::write(cfg.base.join("change.txt"), "tampered").unwrap();
    assert!(feedback::draft(&cfg, &path, &["change.txt".into()], None, false).is_err());
    assert!(feedback::draft(&cfg, &path, &[], None, false).is_ok());
    put(
        &cfg,
        "guide.md",
        "DECISION queue: Preserve a new retry limit.\n",
    );
    let stale = feedback::draft(&cfg, &path, &[], None, false).unwrap();
    assert_eq!(stale["stale"], true);
    assert!(
        stale["markdown"]
            .as_str()
            .unwrap()
            .contains("do not assert a current fact")
    );
}
#[tokio::test]
async fn malformed_partial_and_malicious_receipts_fail_closed() {
    let (_tmp, cfg, _model, original) = setup().await;
    let invalid = vec![
        ("/snapshot/project_id", json!("other-project")),
        ("/evidence_ids/0", json!("missing")),
        ("/changes/0/path", json!("../outside")),
        ("/changes/0/path", json!("/etc/passwd")),
        ("/changes/0/path", json!(".env")),
        ("/changes/0/path", json!("a/./b")),
        ("/changes/0/after", json!("invented")),
        ("/checks/1/event_id", json!("attempt-1")),
        ("/checks/0/exit_status", json!("passed")),
        ("/usage/cost_usd", json!(-1)),
        ("/finished_at", json!("2020-01-01T00:00:00Z")),
        ("/started_at", json!("2026-10-11")),
        ("/notes/0/text", json!("x".repeat(4001))),
    ];
    for (pointer, value) in invalid {
        let mut bad = original.clone();
        *bad.pointer_mut(pointer).unwrap() = value;
        let path = save(&cfg, &bad);
        assert!(
            feedback::draft(&cfg, &path, &[], None, false).is_err(),
            "{pointer}"
        );
    }
    let mut partial = original.clone();
    partial["usage"].as_object_mut().unwrap().remove("cost_usd");
    assert!(feedback::draft(&cfg, &save(&cfg, &partial), &[], None, false).is_err());
    let mut unknown = original.clone();
    unknown["checks"][0]["exit_status"] = Value::Null;
    assert_eq!(
        feedback::draft(&cfg, &save(&cfg, &unknown), &[], None, false).unwrap()["reported_checks"]
            ["unknown"],
        1
    );
}
#[tokio::test]
async fn secret_checker_material_and_claimed_independence_never_enter_public_notes() {
    let (_tmp, cfg, _model, mut receipt) = setup().await;
    let secret = "ANSWER_KEY_private_fixture_sk-secret-123456789";
    receipt["notes"][0]["text"] = json!(secret);
    receipt["actor"]["id"] = json!(secret);
    receipt["task_id"] = json!(secret);
    receipt["checks"][0]["checker_id"] = json!(secret);
    receipt["attestations"] = json!([{"kind":"external_check","actor_id":"runner","capture_digest":util::digest("private capture")},{"kind":"independent_review","actor_id":"reviewer","capture_digest":util::digest("review") }]);
    receipt["grants"]["network"] = json!(true);
    let draft = feedback::draft(&cfg, &save(&cfg, &receipt), &[], None, false).unwrap();
    assert!(!draft.to_string().contains(secret));
    assert_eq!(draft["attestations_verified"], false);
    assert_eq!(draft["independently_reviewed_conclusions"], 0);
    assert_eq!(draft["network_calls"], 0);
    assert_eq!(draft["declared_grants_are_authority"], false);
}
#[tokio::test]
async fn explicit_note_intake_uses_existing_sources_and_preserves_withdrawn_history() {
    let (_tmp, cfg, model, receipt) = setup().await;
    let path = save(&cfg, &receipt);
    let output = cfg.base.join("docs/task-note.md");
    assert!(feedback::draft(&cfg, &path, &[], Some(&output), false).is_err());
    assert!(!output.exists());
    let draft = feedback::draft(&cfg, &path, &[], Some(&output), true).unwrap();
    assert_eq!(draft["source_write"], true);
    let before = fs::read(&output).unwrap();
    assert!(feedback::draft(&cfg, &path, &[], Some(&output), true).is_err());
    assert_eq!(before, fs::read(&output).unwrap());
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let conn = storage::read_only(&cfg.state.join("state.db")).unwrap();
    let views = storage::views(&conn).unwrap();
    let note = views.iter().find(|v| v.kind == "reported_outcome").unwrap();
    let evidence = note.evidence[0].id.clone();
    assert_ne!(note.kind, "decision");
    let ctx = context::build_context(
        &conn,
        &context::ContextOptions {
            task: "feedback task retry checks".into(),
            paths: vec![],
            max_tokens: 6000,
        },
    )
    .unwrap();
    assert!(serde_json::to_string(&ctx).unwrap().contains(&evidence));
    drop(conn);
    fs::remove_file(&output).unwrap();
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
}
#[tokio::test]
async fn cli_read_only_configuration_and_destinations_need_no_provider() {
    let (_tmp, cfg, _model, receipt) = setup().await;
    let path = save(&cfg, &receipt);
    let mut config = cfg.config.clone();
    config.models.generative.enabled = false;
    config.models.generative.provider = "not-installed".into();
    fs::write(&cfg.config_path, serde_yaml::to_string(&config).unwrap()).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_lore"))
        .arg("--config")
        .arg(&cfg.config_path)
        .args(["--json", "feedback", "draft", "--receipt"])
        .arg(path)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    let output: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(output["model_calls"], 0);
    for path in [
        cfg.state.join("note.md"),
        cfg.wiki.join("note.md"),
        cfg.base.join("AGENTS.md"),
        cfg.base.join("CLAUDE.md"),
    ] {
        assert!(feedback::draft(&cfg, &save(&cfg, &receipt), &[], Some(&path), true).is_err());
    }
}
#[cfg(unix)]
#[tokio::test]
async fn symlink_receipts_files_and_destinations_are_rejected() {
    let (_tmp, cfg, _model, receipt) = setup().await;
    let path = save(&cfg, &receipt);
    let linked = cfg.base.join("linked.json");
    std::os::unix::fs::symlink(&path, &linked).unwrap();
    assert!(feedback::draft(&cfg, &linked, &[], None, false).is_err());
    fs::remove_file(cfg.base.join("change.txt")).unwrap();
    std::os::unix::fs::symlink(&path, cfg.base.join("change.txt")).unwrap();
    assert!(feedback::draft(&cfg, &path, &["change.txt".into()], None, false).is_err());
    let output = cfg.base.join("note.md");
    std::os::unix::fs::symlink(&path, &output).unwrap();
    assert!(feedback::draft(&cfg, &path, &[], Some(&output), true).is_err());
}

#[cfg(unix)]
#[tokio::test]
async fn granted_fifo_is_rejected_without_waiting_for_a_writer() {
    use std::{
        ffi::CString,
        os::unix::ffi::OsStrExt,
        process::Stdio,
        time::{Duration, Instant},
    };
    let (_tmp, cfg, _model, receipt) = setup().await;
    let receipt_path = save(&cfg, &receipt);
    let fifo = cfg.base.join("change.txt");
    fs::remove_file(&fifo).unwrap();
    let name = CString::new(fifo.as_os_str().as_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
    let mut child = Command::new(env!("CARGO_BIN_EXE_lore"))
        .arg("--config")
        .arg(&cfg.config_path)
        .args(["--json", "feedback", "draft", "--receipt"])
        .arg(receipt_path)
        .args(["--allow-file", "change.txt"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(!status.success());
            break;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("a FIFO must be rejected before a blocking open");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}
