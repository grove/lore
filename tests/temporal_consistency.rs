mod common;
use common::*;
use lore::{
    engine::{self, UpdateOptions},
    storage,
};
use rusqlite::Connection;
use std::fs;

#[tokio::test]
async fn documentary_decision_timeline_invalidates_related_topics() {
    let (_directory, cfg, fake) = project();
    put(
        &cfg,
        "ADR-001.md",
        "DECISION legacy: MySQL is the selected database.\n",
    );
    engine::update(&cfg, &fake, None, UpdateOptions::default())
        .await
        .unwrap();
    let before = fs::read_to_string(cfg.wiki.join("topics/legacy.md")).unwrap();
    put(
        &cfg,
        "review.md",
        "DECISION reaffirmation: The review reaffirms ADR-001: MySQL remains the selected database.\n",
    );
    engine::update(&cfg, &fake, None, UpdateOptions::default())
        .await
        .unwrap();
    let reassessed = fs::read_to_string(cfg.wiki.join("topics/legacy.md")).unwrap();
    assert_ne!(before, reassessed);
    assert!(reassessed.contains("reaffirms"));
    assert!(reassessed.contains("reaffirmation.md"));

    put(
        &cfg,
        "ADR-027.md",
        "DECISION replacement: PostgreSQL replaces ADR-001 for the production database.\n",
    );
    let update = engine::update(&cfg, &fake, None, UpdateOptions::default())
        .await
        .unwrap();
    assert!(!update.no_op);
    let legacy = fs::read_to_string(cfg.wiki.join("topics/legacy.md")).unwrap();
    assert!(legacy.contains("explicitly supersedes"));
    assert!(legacy.contains("replacement.md"));
    assert!(legacy.contains("not independent verification of deployment"));
    let index = fs::read_to_string(cfg.wiki.join("index.md")).unwrap();
    assert!(index.contains("explicitly supersedes"));
    assert!(index.contains("reaffirms"));

    let conn = Connection::open(cfg.state.join("state.db")).unwrap();
    let relations = storage::relations(&conn).unwrap();
    assert_eq!(
        relations
            .iter()
            .filter(|r| r.kind == "reaffirms" && r.active)
            .count(),
        1
    );
    assert!(relations.iter().any(|r| r.kind == "supersedes" && r.active));
    let noop = engine::update(&cfg, &fake, None, UpdateOptions::default())
        .await
        .unwrap();
    assert!(noop.no_op);
    assert_eq!(noop.model_calls, 0);
}

#[test]
fn v2_database_can_upgrade_without_losing_records() {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(storage::SCHEMA_V1).unwrap();
    db.execute_batch(storage::SCHEMA_V2).unwrap();
    db.execute_batch("INSERT INTO projects VALUES('test','Test','2026-01-01');")
        .unwrap();
    storage::migrate(&db).unwrap();
    let version: i64 = db
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, 3);
    let count: i64 = db
        .query_row("SELECT count(*) FROM projects", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1);
    let exists: i64 = db
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE name='reaffirmation_links'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(exists, 1);
}

#[test]
fn publication_date_is_not_decision_effective_date() {
    use lore::domain::effective_time_grounded;
    assert!(!effective_time_grounded(
        "We selected PostgreSQL for the ledger.",
        "# ADR-027\nDate: 2026-07-01\nStatus: Accepted",
        "2026-07-01"
    ));
    assert!(effective_time_grounded(
        "The service went live on 2026-08-18.",
        "# Deployment\nDate: 2026-08-20",
        "2026-08-18"
    ));
    assert!(effective_time_grounded(
        "PostgreSQL is selected.",
        "Effective date: 2026-07-15",
        "2026-07-15"
    ));
    assert!(effective_time_grounded(
        "Any source text",
        "Date: 2026-07-01",
        ""
    ));
}
