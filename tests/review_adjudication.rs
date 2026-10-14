mod common;
use common::*;
use lore::{
    context,
    engine::{self, UpdateOptions},
    reviews::{
        self,
        adjudication::{self, Request},
    },
    storage,
};
use serde_json::{Value, json};
use std::{fs, process::Command};

async fn setup() -> (
    tempfile::TempDir,
    lore::config::ResolvedConfig,
    FakeModel,
    String,
) {
    let (tmp, cfg, model) = project();
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
        "ADR-002.md",
        "DECISION ledger: PostgreSQL is the selected database.\n",
    );
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let conn = storage::read_only(&cfg.state.join("state.db")).unwrap();
    let id = reviews::list(&conn, true)
        .unwrap()
        .into_iter()
        .find(|r| r.category.as_deref() == Some("conflict"))
        .unwrap()
        .id;
    (tmp, cfg, model, id)
}
fn request<'a>(digest: Option<&'a str>) -> Request<'a> {
    Request {
        decision: "keep_separate",
        relation: None,
        evidence: None,
        reason: "Preserve both documented choices for investigation.",
        actor: "reviewer-fixture",
        actor_kind: "agent",
        expect_digest: digest,
    }
}
fn original(conn: &rusqlite::Connection) -> Value {
    let mut q = conn
        .prepare("SELECT id,exact_excerpt,excerpt_digest FROM evidence_snapshots ORDER BY id")
        .unwrap();
    let evidence = q
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    json!({"evidence":evidence,"knowledge":storage::views(conn).unwrap(),"relations":storage::relation_facts(conn).unwrap()})
}
#[tokio::test]
async fn preview_apply_retry_and_projections_preserve_documentary_history() {
    let (_tmp, cfg, _model, id) = setup().await;
    let before = fs::read(cfg.state.join("state.db")).unwrap();
    let preview = engine::adjudicate_review(&cfg, &id, &request(None), false).unwrap();
    assert_eq!(before, fs::read(cfg.state.join("state.db")).unwrap());
    assert!(engine::adjudicate_review(&cfg, &id, &request(Some("stale")), true).is_err());
    assert_eq!(before, fs::read(cfg.state.join("state.db")).unwrap());
    let conn = storage::read_only(&cfg.state.join("state.db")).unwrap();
    let history = original(&conn);
    let snapshot = storage::registry_revision(&conn).unwrap();
    drop(conn);
    let digest = preview["context_digest"].as_str().unwrap();
    let applied = engine::adjudicate_review(&cfg, &id, &request(Some(digest)), true).unwrap();
    assert_eq!(applied["changed"], true);
    assert_eq!(applied["actor_authenticated"], false);
    let published = fs::read(cfg.state.join("state.db")).unwrap();
    assert_eq!(
        engine::adjudicate_review(&cfg, &id, &request(Some(digest)), true).unwrap()["changed"],
        false
    );
    assert_eq!(published, fs::read(cfg.state.join("state.db")).unwrap());
    let conn = storage::read_only(&cfg.state.join("state.db")).unwrap();
    assert_eq!(history, original(&conn));
    assert_ne!(snapshot, storage::registry_revision(&conn).unwrap());
    assert_eq!(adjudication::history(&conn, &id).unwrap().len(), 1);
    let ctx = context::build_context(
        &conn,
        &context::ContextOptions {
            task: "ledger database".into(),
            paths: vec![],
            max_tokens: 6000,
        },
    )
    .unwrap();
    assert!(
        serde_json::to_string(&ctx)
            .unwrap()
            .contains("manual_adjudication")
    );
    assert!(
        fs::read_to_string(cfg.wiki.join("reviews.md"))
            .unwrap()
            .contains("manual_adjudication")
    );
    assert_eq!(
        engine::audit(&cfg).unwrap()["manual_adjudications"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    for action in ["decisions", "cases"] {
        let out = Command::new(env!("CARGO_BIN_EXE_lore"))
            .arg("--config")
            .arg(&cfg.config_path)
            .args(["--json", action, "ledger database"])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stdout)
        );
        assert!(String::from_utf8_lossy(&out.stdout).contains("manual_adjudication"));
    }
    drop(conn);
    let db = rusqlite::Connection::open(cfg.state.join("state.db")).unwrap();
    assert!(
        db.execute("DELETE FROM review_adjudication_events", [])
            .is_err()
    );
    assert!(
        db.execute(
            "UPDATE review_adjudication_events SET event_kind='invalidated'",
            []
        )
        .is_err()
    );
}
#[tokio::test]
async fn changed_review_and_source_reversion_cannot_replay_an_old_interpretation() {
    let (_tmp, cfg, model, id) = setup().await;
    let preview = engine::adjudicate_review(&cfg, &id, &request(None), false).unwrap();
    engine::change_review(
        &cfg,
        &id,
        "dismissed",
        "Concurrent disposition",
        "another-caller",
    )
    .unwrap();
    assert!(
        engine::adjudicate_review(
            &cfg,
            &id,
            &request(preview["context_digest"].as_str()),
            true
        )
        .is_err()
    );
    let preview = engine::adjudicate_review(&cfg, &id, &request(None), false).unwrap();
    engine::adjudicate_review(
        &cfg,
        &id,
        &request(preview["context_digest"].as_str()),
        true,
    )
    .unwrap();
    fs::remove_file(cfg.base.join("docs/ADR-002.md")).unwrap();
    assert!(engine::adjudicate_review(&cfg, &id, &request(None), false).is_err());
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    put(
        &cfg,
        "ADR-002.md",
        "DECISION ledger: PostgreSQL is the selected database.\n",
    );
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let conn = storage::read_only(&cfg.state.join("state.db")).unwrap();
    assert!(adjudication::projection(&conn).unwrap().is_empty());
    let events = adjudication::history(&conn, &id).unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[1]["event_kind"], "invalidated");
    assert_eq!(
        reviews::list(&conn, true)
            .unwrap()
            .into_iter()
            .find(|r| r.id == id)
            .unwrap()
            .status,
        "pending"
    );
    assert!(
        engine::adjudicate_review(
            &cfg,
            &id,
            &request(preview["context_digest"].as_str()),
            true
        )
        .is_err()
    );
}
#[tokio::test]
async fn acceptance_requires_exact_active_evidence_and_never_invents_authority() {
    let (_tmp, cfg, _model, id) = setup().await;
    let preview = engine::adjudicate_review(&cfg, &id, &request(None), false).unwrap();
    let conn = storage::read_only(&cfg.state.join("state.db")).unwrap();
    let original = original(&conn);
    let facts = storage::relation_facts(&conn).unwrap();
    let fact = facts
        .iter()
        .find(|f| f.kind == "contradicts" && f.active)
        .unwrap();
    let mut req = request(None);
    req.decision = "accept_relation";
    req.relation = Some("contradicts");
    req.evidence = Some(&fact.evidence_id);
    let accepted = engine::adjudicate_review(&cfg, &id, &req, false).unwrap();
    assert_eq!(
        accepted["authority"],
        "existing_documentary_relation_confirmed"
    );
    req.relation = Some("supersedes");
    assert!(engine::adjudicate_review(&cfg, &id, &req, false).is_err());
    req.relation = Some("contradicts");
    req.evidence = Some("missing");
    assert!(engine::adjudicate_review(&cfg, &id, &req, false).is_err());
    let wrong = preview["evidence_ids"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .find(|e| {
            storage::evidence_snapshot(&conn, e).unwrap().source_id
                != preview["binding"]["source_id"]
        })
        .unwrap();
    req.evidence = Some(wrong);
    assert!(engine::adjudicate_review(&cfg, &id, &req, false).is_err());
    assert_eq!(original, self::original(&conn));
}
#[test]
fn migration_from_eight_is_transactional_and_read_only_does_not_upgrade() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().canonicalize().unwrap().join("state.db");
    let db = rusqlite::Connection::open(&path).unwrap();
    for schema in [
        storage::SCHEMA_V1,
        storage::SCHEMA_V2,
        storage::SCHEMA_V3,
        storage::SCHEMA_V4,
        storage::SCHEMA_V5,
        storage::SCHEMA_V6,
        storage::SCHEMA_V7,
        storage::SCHEMA_V8,
    ] {
        db.execute_batch(schema).unwrap();
    }
    drop(db);
    let before = fs::read(&path).unwrap();
    let ro = storage::read_only(&path).unwrap();
    assert!(adjudication::projection(&ro).unwrap().is_empty());
    drop(ro);
    assert_eq!(before, fs::read(&path).unwrap());
    let db = rusqlite::Connection::open(path).unwrap();
    db.execute_batch("CREATE VIEW review_adjudications AS SELECT 1;")
        .unwrap();
    assert!(storage::migrate(&db).is_err());
    assert_eq!(
        db.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        8
    );
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM sqlite_master WHERE name='review_adjudication_events'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    db.execute_batch("DROP VIEW review_adjudications;").unwrap();
    storage::migrate(&db).unwrap();
    assert_eq!(
        db.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        storage::SCHEMA_VERSION
    );
}

struct Ambiguous(FakeModel);
impl lore::inference::GenerativeModel for Ambiguous {
    fn descriptor(&self) -> &lore::inference::ModelDescriptor {
        self.0.descriptor()
    }
    fn generate<'a>(
        &'a self,
        r: &'a lore::inference::GenerationRequest,
    ) -> lore::inference::ModelFuture<'a, lore::inference::GenerationResponse> {
        Box::pin(async move {
            let input: Value = serde_json::from_str(&r.input).unwrap();
            let mut response = self.0.generate(r).await?;
            if input["task"] == "extract" && input["text"].as_str().unwrap().contains("# staging") {
                let mut parsed: Value = serde_json::from_str(&response.text).unwrap();
                for assertion in parsed["assertions"].as_array_mut().unwrap() {
                    assertion["scope"] = json!("staging");
                }
                response.text = parsed.to_string();
            }
            if input["task"] == "reconcile" && !input["candidates"].as_array().unwrap().is_empty() {
                response.text=json!({"equivalent_to":"","uncertain":true,"relations":input["candidates"].as_array().unwrap().iter().map(|t|json!({"target_id":t["id"],"kind":"uncertain","quote":"","reason":"Scope and acceptance require documentary inspection."})).collect::<Vec<_>>()}).to_string();
            }
            Ok(response)
        })
    }
}
#[tokio::test]
async fn ambiguous_and_proposed_inputs_cannot_manufacture_accepted_supersession() {
    let (_tmp, cfg, fake) = project();
    let model = Ambiguous(fake);
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
        "plan.md",
        "PLAN ledger: PostgreSQL may replace the ledger database.\n",
    );
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let conn = storage::read_only(&cfg.state.join("state.db")).unwrap();
    let all = reviews::list(&conn, true).unwrap();
    let generic = all
        .iter()
        .find(|r| r.category.as_deref() == Some("ambiguity"))
        .unwrap();
    assert!(engine::adjudicate_review(&cfg, &generic.id, &request(None), false).is_err());
    let exact = all
        .iter()
        .find(|r| r.category.as_deref() == Some("relationship"))
        .unwrap();
    let id = exact.id.clone();
    let before = original(&conn);
    let preview = engine::adjudicate_review(&cfg, &id, &request(None), false).unwrap();
    let ev = preview["evidence_ids"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .find(|e| {
            storage::evidence_snapshot(&conn, e).unwrap().source_id
                == preview["binding"]["source_id"]
        })
        .unwrap();
    let mut req = request(None);
    req.decision = "accept_relation";
    req.relation = Some("supersedes");
    req.evidence = Some(ev);
    let accepted = engine::adjudicate_review(&cfg, &id, &req, false).unwrap();
    assert_eq!(accepted["authority"], "human_advisory_interpretation");
    drop(conn);
    req.expect_digest = accepted["context_digest"].as_str();
    engine::adjudicate_review(&cfg, &id, &req, true).unwrap();
    let conn = storage::read_only(&cfg.state.join("state.db")).unwrap();
    assert_eq!(before, original(&conn));
    assert!(storage::relation_facts(&conn).unwrap().is_empty());
    let context = context::build_context(
        &conn,
        &context::ContextOptions {
            task: "ledger database".into(),
            paths: vec![],
            max_tokens: 12_000,
        },
    )
    .unwrap();
    let rendered = serde_json::to_string(&context).unwrap();
    assert!(rendered.contains("human_advisory_interpretation"));
    let views = storage::views(&conn).unwrap();
    assert!(
        views
            .iter()
            .any(|view| view.kind == "decision" && view.lifecycle == "accepted")
    );
    assert!(
        views
            .iter()
            .any(|view| view.kind == "proposal" && view.lifecycle == "proposed")
    );
}

#[tokio::test]
async fn unchanged_eight_project_requires_update_and_migrates_without_new_model_calls() {
    let (_tmp, cfg, model, _id) = setup().await;
    let db = rusqlite::Connection::open(cfg.state.join("state.db")).unwrap();
    db.execute_batch("DROP VIEW review_adjudications; DROP TABLE review_adjudication_events; PRAGMA user_version=8;").unwrap();
    drop(db);
    let calls = model.calls.load(std::sync::atomic::Ordering::SeqCst);
    assert!(engine::status(&cfg).unwrap().needs_update);
    let before = fs::read(cfg.state.join("state.db")).unwrap();
    assert!(engine::status(&cfg).unwrap().needs_update);
    assert_eq!(before, fs::read(cfg.state.join("state.db")).unwrap());
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    assert!(!engine::status(&cfg).unwrap().needs_update);
    assert_eq!(calls, model.calls.load(std::sync::atomic::Ordering::SeqCst));
}

#[tokio::test]
async fn same_text_different_scopes_stay_separate_and_only_one_candidate_is_dismissed() {
    let (_tmp, cfg, fake) = project();
    let model = Ambiguous(fake);
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
        "staging.md",
        "# staging\nDECISION ledger: MySQL is the selected database.\n",
    );
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    put(
        &cfg,
        "third.md",
        "PLAN ledger: Investigate another ledger database.\n",
    );
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let conn = storage::read_only(&cfg.state.join("state.db")).unwrap();
    let before = original(&conn);
    let views = storage::views(&conn).unwrap();
    assert_eq!(
        views
            .iter()
            .filter(|v| v.statement == "MySQL is the selected database.")
            .count(),
        2
    );
    let pending = reviews::list(&conn, false).unwrap();
    let exact = pending
        .iter()
        .find(|r| r.category.as_deref() == Some("relationship"))
        .unwrap();
    let id = exact.id.clone();
    drop(conn);
    let mut req = request(None);
    req.decision = "reject_relation";
    let preview = engine::adjudicate_review(&cfg, &id, &req, false).unwrap();
    req.expect_digest = preview["context_digest"].as_str();
    engine::adjudicate_review(&cfg, &id, &req, true).unwrap();
    let conn = storage::read_only(&cfg.state.join("state.db")).unwrap();
    assert_eq!(before, original(&conn));
    assert_eq!(
        reviews::list(&conn, false).unwrap().len(),
        pending.len() - 1
    );
    assert_eq!(adjudication::projection(&conn).unwrap().len(), 1);
}
