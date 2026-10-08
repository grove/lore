use lore::storage::migrate;
use rusqlite::Connection;
#[test]
fn migration_is_versioned_and_enforces_foreign_keys() {
    let db = Connection::open_in_memory().unwrap();
    migrate(&db).unwrap();
    migrate(&db).unwrap();
    let version: i64 = db
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, 3);
    let fk: i64 = db
        .pragma_query_value(None, "foreign_keys", |r| r.get(0))
        .unwrap();
    assert_eq!(fk, 1);
    let integrity: String = db
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .unwrap();
    assert_eq!(integrity, "ok");
}
#[test]
fn historical_evidence_is_immutable_and_referentially_valid() {
    let db = Connection::open_in_memory().unwrap();
    migrate(&db).unwrap();
    db.execute_batch("INSERT INTO projects VALUES('p','Project','2026-10-08'); INSERT INTO source_roots VALUES('r','p','./docs'); INSERT INTO sources(id,root_id,relative_path) VALUES('s','r','adr/1.md'); INSERT INTO source_revisions VALUES('rev1','s','adr/1.md','hash1','2026-10-08'); INSERT INTO evidence_snapshots(id,source_id,source_revision_id,exact_excerpt,excerpt_digest,captured_at) VALUES('ev1','s','rev1','We decided to use MySQL.','hash2','2026-10-08');").unwrap();
    assert!(
        db.execute(
            "UPDATE evidence_snapshots SET exact_excerpt='changed' WHERE id='ev1'",
            []
        )
        .is_err()
    );
    assert!(
        db.execute("DELETE FROM evidence_snapshots WHERE id='ev1'", [])
            .is_err()
    );
    assert!(db.execute("INSERT INTO evidence_snapshots(id,source_id,source_revision_id,exact_excerpt,excerpt_digest,captured_at) VALUES('bad','s','missing','X','hash','2026-10-08')",[]).is_err());
    db.execute(
        "UPDATE sources SET removed_at='2026-10-09' WHERE id='s'",
        [],
    )
    .unwrap();
    let text: String = db
        .query_row(
            "SELECT exact_excerpt FROM evidence_snapshots WHERE id='ev1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(text, "We decided to use MySQL.");
}
#[test]
fn rejects_future_database_versions() {
    let db = Connection::open_in_memory().unwrap();
    db.pragma_update(None, "user_version", 99).unwrap();
    assert!(migrate(&db).is_err());
}
