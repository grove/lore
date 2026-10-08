mod common;
use common::*;
use lore::{
    engine::{self, UpdateOptions},
    inference::*,
    reviews, storage,
};
use serde_json::{Value, json};
use std::{fs, process::Command};
struct AmbiguousThenExplicit {
    inner: FakeModel,
}
impl GenerativeModel for AmbiguousThenExplicit {
    fn descriptor(&self) -> &ModelDescriptor {
        self.inner.descriptor()
    }
    fn generate<'a>(
        &'a self,
        request: &'a GenerationRequest,
    ) -> ModelFuture<'a, GenerationResponse> {
        Box::pin(async move {
            let input: Value = serde_json::from_str(&request.input).unwrap();
            let mut response = self.inner.generate(request).await?;
            if input["task"] == "reconcile"
                && input["assertion"]["statement"]
                    .as_str()
                    .unwrap()
                    .contains("calls for changing")
            {
                let target = input["candidates"].as_array().unwrap().iter().find(|c| {
                    c["statement"]
                        .as_str()
                        .unwrap()
                        .contains("MySQL is the selected")
                });
                if let Some(t) = target {
                    response.text=json!({"equivalent_to":"","uncertain":true,"relations":[{"target_id":t["id"],"kind":"uncertain","quote":"","reason":"Replacement is not established by this short assertion alone."}]}).to_string();
                }
            }
            Ok(response)
        })
    }
}
async fn setup() -> (
    tempfile::TempDir,
    lore::config::ResolvedConfig,
    AmbiguousThenExplicit,
) {
    let (temp, cfg, fake) = project();
    let model = AmbiguousThenExplicit { inner: fake };
    put(
        &cfg,
        "ADR-001.md",
        "DECISION ledger: MySQL is the selected database.\n",
    );
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    put(
        &cfg,
        "ADR-027.md",
        "DECISION migration: The accepted decision calls for changing the ledger database.\nDECISION migration: PostgreSQL replaces ADR-001 for the production database.\n",
    );
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    (temp, cfg, model)
}
#[tokio::test]
async fn exact_relationship_reviews_resolve_and_withdrawn_evidence_reopens() {
    let (_temp, cfg, model) = setup().await;
    let conn = storage::read_only(&cfg.state.join("state.db")).unwrap();
    let all = reviews::list(&conn, true).unwrap();
    let resolved = all
        .iter()
        .find(|r| r.category.as_deref() == Some("relationship"))
        .unwrap();
    assert_eq!(resolved.status, "resolved");
    let history = reviews::events(&conn, &resolved.id).unwrap();
    let ev = history.last().unwrap().evidence_id.clone().unwrap();
    assert_eq!(
        history.last().unwrap().reason_code,
        "documented_relationship"
    );
    assert!(
        storage::relation_facts(&conn)
            .unwrap()
            .iter()
            .any(|f| f.evidence_id == ev && f.kind == "supersedes" && f.active)
    );
    assert!(
        all.iter()
            .any(|r| r.category.as_deref() == Some("ambiguity") && r.status == "pending")
    );
    let id = resolved.id.clone();
    let evidence_count: i64 = conn
        .query_row("SELECT count(*) FROM evidence_snapshots", [], |r| r.get(0))
        .unwrap();
    drop(conn);
    let before = fs::read(cfg.wiki.join("reviews.md")).unwrap();
    assert!(
        engine::update(&cfg, &model, None, UpdateOptions::default())
            .await
            .unwrap()
            .no_op
    );
    assert_eq!(before, fs::read(cfg.wiki.join("reviews.md")).unwrap());
    fs::remove_file(cfg.base.join("docs/ADR-027.md")).unwrap();
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let conn = storage::read_only(&cfg.state.join("state.db")).unwrap();
    assert_eq!(
        reviews::list(&conn, true)
            .unwrap()
            .iter()
            .find(|r| r.id == id)
            .unwrap()
            .status,
        "pending"
    );
    let events = reviews::events(&conn, &id).unwrap();
    assert_eq!(events.len(), 3);
    assert_eq!(events.last().unwrap().reason_code, "resolution_withdrawn");
    assert_eq!(
        evidence_count,
        conn.query_row("SELECT count(*) FROM evidence_snapshots", [], |r| r
            .get::<_, i64>(0))
            .unwrap()
    );
}
#[tokio::test]
async fn manual_cli_dispositions_publish_without_inference_or_graph_edits() {
    let (_temp, cfg, model) = setup().await;
    let conn = storage::read_only(&cfg.state.join("state.db")).unwrap();
    let pending = reviews::list(&conn, false).unwrap()[0].id.clone();
    let snapshots: i64 = conn
        .query_row("SELECT count(*) FROM evidence_snapshots", [], |r| r.get(0))
        .unwrap();
    let revisions: i64 = conn
        .query_row("SELECT count(*) FROM knowledge_revisions", [], |r| r.get(0))
        .unwrap();
    let ledger = fs::read(cfg.wiki.join("topics/ledger.md")).unwrap();
    drop(conn);
    let call = |action: &str, reason: &str| {
        Command::new(env!("CARGO_BIN_EXE_lore"))
            .arg("--config")
            .arg(&cfg.config_path)
            .arg("--json")
            .args([
                "review",
                action,
                &pending,
                "--reason",
                reason,
                "--actor",
                "reviewer-fixture",
            ])
            .output()
            .unwrap()
    };
    let out = call(
        "resolve",
        "Checked the exact ADR evidence; retain the historical question.",
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    let report: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(report["model_calls"], 0);
    assert_eq!(report["status"], "resolved");
    assert_eq!(ledger, fs::read(cfg.wiki.join("topics/ledger.md")).unwrap());
    let conn = storage::read_only(&cfg.state.join("state.db")).unwrap();
    assert_eq!(
        snapshots,
        conn.query_row("SELECT count(*) FROM evidence_snapshots", [], |r| r
            .get::<_, i64>(0))
            .unwrap()
    );
    assert_eq!(
        revisions,
        conn.query_row("SELECT count(*) FROM knowledge_revisions", [], |r| r
            .get::<_, i64>(0))
            .unwrap()
    );
    assert_eq!(
        reviews::events(&conn, &pending)
            .unwrap()
            .last()
            .unwrap()
            .actor,
        "reviewer-fixture"
    );
    drop(conn);
    assert!(
        engine::update(&cfg, &model, None, UpdateOptions::default())
            .await
            .unwrap()
            .no_op
    );
    let old = fs::read(cfg.wiki.join("reviews.md")).unwrap();
    assert!(!call("reopen", "").status.success());
    assert_eq!(old, fs::read(cfg.wiki.join("reviews.md")).unwrap());
    assert!(
        call("reopen", "New evidence needs inspection.")
            .status
            .success()
    );
    assert!(
        call("dismiss", "Duplicate question; no implementation verified.")
            .status
            .success()
    );
    let conn = storage::read_only(&cfg.state.join("state.db")).unwrap();
    let history = reviews::events(&conn, &pending).unwrap();
    assert_eq!(history.len(), 4);
    assert_eq!(history.last().unwrap().to_status, "dismissed");
    drop(conn);
    fs::write(cfg.wiki.join("index.md"), "A human's edit").unwrap();
    assert!(!call("reopen", "Check again.").status.success());
    assert_eq!(
        fs::read_to_string(cfg.wiki.join("index.md")).unwrap(),
        "A human's edit"
    );
}
#[test]
fn migration_retains_legacy_queue_and_events_are_append_only() {
    let db = rusqlite::Connection::open_in_memory().unwrap();
    db.execute_batch(storage::SCHEMA_V1).unwrap();
    db.execute_batch(storage::SCHEMA_V2).unwrap();
    db.execute_batch(storage::SCHEMA_V3).unwrap();
    db.execute_batch("INSERT INTO projects VALUES('p','P','2026'); INSERT INTO review_items VALUES('legacy','p','Retain this old question.','pending');").unwrap();
    storage::migrate(&db).unwrap();
    reviews::backfill(&db).unwrap();
    let old = reviews::show(&db, "legacy").unwrap();
    assert_eq!(old["review"]["category"], Value::Null);
    assert_eq!(old["history"][0]["actor_type"], "migration");
    assert!(reviews::manual(&db, "legacy", "resolved", "Manually inspected.", "tester").unwrap());
    assert!(!reviews::manual(&db, "legacy", "resolved", "Idempotent.", "tester").unwrap());
    assert!(db.execute("DELETE FROM review_events", []).is_err());
    assert!(
        db.execute("UPDATE review_events SET note='overwrite'", [])
            .is_err()
    );
    assert_eq!(reviews::events(&db, "legacy").unwrap().len(), 2);
    assert!(reviews::manual(&db, "missing", "resolved", "Checked.", "tester").is_err());
}
