mod common;
use common::*;
use lore::{
    engine::{self, UpdateOptions},
    reviews, storage,
};
use rusqlite::Connection;

#[tokio::test]
async fn same_topic_or_source_without_an_accepted_decision_does_not_resolve_review() {
    let (_temp, cfg, model) = project();
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
        "DECISION replacement: PostgreSQL replaces ADR-001 for the production database.\nPLAN replacement: Investigate a future engine variant.\n",
    );
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let conn = Connection::open(cfg.state.join("state.db")).unwrap();
    storage::migrate(&conn).unwrap();
    let original = storage::views(&conn)
        .unwrap()
        .into_iter()
        .find(|v| v.topic == "ledger")
        .unwrap();
    let proposal:String=conn.query_row("SELECT ar.id FROM active_assertions a JOIN assertion_revisions ar ON ar.id=a.assertion_revision_id WHERE ar.modality='proposal'",[],|r|r.get(0)).unwrap();
    let original_assertion = original
        .evidence
        .iter()
        .find(|e| e.active)
        .unwrap()
        .assertion_id
        .clone();
    storage::review(
        &conn,
        &cfg.project_id,
        &format!("relationship:{proposal}:{}", original.id),
        "A proposal in the same source is not the accepted replacement.",
    )
    .unwrap();
    storage::review(
        &conn,
        &cfg.project_id,
        &format!("relationship:{original_assertion}:{}", original.id),
        "A question from a different source revision is not settled by topic similarity.",
    )
    .unwrap();
    reviews::refresh(&conn).unwrap();
    let items = reviews::list(&conn, true).unwrap();
    for assertion in [&proposal, &original_assertion] {
        let item = items
            .iter()
            .find(|r| {
                r.category.as_deref() == Some("relationship")
                    && r.assertion_revision_id.as_ref() == Some(assertion)
            })
            .unwrap();
        assert_eq!(item.status, "pending");
    }
}

#[tokio::test]
async fn manual_resolution_reopens_only_when_bound_evidence_changes() {
    let (_temp, cfg, model) = project();
    put(
        &cfg,
        "ADR-001.md",
        "DECISION ledger: MySQL is the selected database.\n",
    );
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let conn = Connection::open(cfg.state.join("state.db")).unwrap();
    storage::migrate(&conn).unwrap();
    let original = storage::views(&conn).unwrap().pop().unwrap();
    let assertion = &original.evidence[0].assertion_id;
    storage::review(
        &conn,
        &cfg.project_id,
        &format!("ambiguous:{assertion}"),
        "A retained test ambiguity.",
    )
    .unwrap();
    let review = reviews::list(&conn, false).unwrap()[0].id.clone();
    reviews::manual(
        &conn,
        &review,
        "resolved",
        "Inspected the original source.",
        "fixture-reviewer",
    )
    .unwrap();
    assert!(
        conn.execute(
            "UPDATE review_items SET status='pending' WHERE id=?1",
            [&review]
        )
        .is_err()
    );
    drop(conn);
    put(
        &cfg,
        "cache.md",
        "DECISION caching: Redis is selected for caching.\n",
    );
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let conn = storage::read_only(&cfg.state.join("state.db")).unwrap();
    assert_eq!(
        reviews::list(&conn, true)
            .unwrap()
            .iter()
            .find(|r| r.id == review)
            .unwrap()
            .status,
        "resolved"
    );
    drop(conn);
    put(
        &cfg,
        "ADR-001.md",
        "DECISION ledger: MySQL is the selected database.\nThe source was revised with additional context.\n",
    );
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let conn = storage::read_only(&cfg.state.join("state.db")).unwrap();
    let events = reviews::events(&conn, &review).unwrap();
    assert_eq!(events.last().unwrap().to_status, "pending");
    assert_eq!(events.last().unwrap().reason_code, "evidence_changed");
    assert_eq!(events[1].actor, "fixture-reviewer");
}
