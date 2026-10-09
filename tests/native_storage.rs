mod common;

use lore::{
    config::{Config, ImportSource, ResolvedConfig},
    domain::ImportKind,
    engine::{self, UpdateOptions},
    imports::{
        self, Inventory, SourceBatch,
        adapters::{AdapterRecord, ImportBatch, NativeEvidence},
    },
    storage, util,
};
use rusqlite::Connection;
use serde_json::json;
use std::{fs, path::PathBuf};

fn database() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    storage::migrate(&conn).unwrap();
    conn.execute(
        "INSERT INTO projects VALUES('project','Project','2026-10-09')",
        [],
    )
    .unwrap();
    conn
}

fn inventory(statement: &str) -> Inventory {
    let native = json!({"id":"work-7","title":statement,"status":"open","extension":{"kept":true}});
    let mut record = AdapterRecord::new("work-7", statement, native);
    record.evidence.push(NativeEvidence {
        locator: "beads://work-7".into(),
        revision: None,
        field: Some("/title".into()),
    });
    let batch = ImportBatch {
        format: "beads-issue-jsonl".into(),
        records: vec![record],
        warnings: vec![],
    };
    let digest = util::json_digest(&batch).unwrap();
    Inventory {
        sources: vec![SourceBatch {
            id: "work".into(),
            kind: ImportKind::Beads,
            path: PathBuf::from("/project/imports/work.jsonl"),
            batch,
            digest: digest.clone(),
        }],
        digest,
        warnings: vec![],
    }
}

#[test]
fn preserves_native_identity_immutable_evidence_and_withdrawn_history() {
    let conn = database();
    let first = inventory("Investigate the retry policy");
    assert_eq!(
        imports::storage::persist(&conn, "project", &first)
            .unwrap()
            .changed,
        1
    );
    let original = imports::views(&conn).unwrap().remove(0);
    assert_eq!(
        original.content_hash,
        util::json_digest(&original.record.native_record).unwrap()
    );
    assert_eq!(original.record.native_record["extension"]["kept"], true);
    assert_eq!(
        imports::storage::persist(&conn, "project", &first)
            .unwrap()
            .changed,
        0
    );
    assert_eq!(
        serde_json::to_value(imports::views(&conn).unwrap().remove(0)).unwrap(),
        serde_json::to_value(&original).unwrap()
    );
    let second = inventory("Investigate the revised retry policy");
    assert_eq!(
        imports::storage::persist(&conn, "project", &second)
            .unwrap()
            .changed,
        1
    );
    let revised = imports::views(&conn).unwrap().remove(0);
    assert_eq!(revised.id, original.id);
    assert_ne!(revised.snapshot_id, original.snapshot_id);
    let old = imports::evidence(&conn, &original.evidence_id).unwrap();
    assert!(!old.current);
    assert_eq!(old.record.statement, "Investigate the retry policy");
    assert!(imports::storage::audit(&conn).unwrap().is_empty());
    assert!(
        conn.execute("UPDATE native_snapshots SET record_json='{}'", [])
            .is_err()
    );
    assert!(conn.execute("DELETE FROM native_snapshots", []).is_err());
    assert!(
        conn.execute("UPDATE native_records SET native_id='other'", [])
            .is_err()
    );
    let empty = Inventory::default();
    assert_eq!(
        imports::storage::persist(&conn, "project", &empty)
            .unwrap()
            .retired,
        1
    );
    assert!(!imports::views(&conn).unwrap()[0].current);
    assert_eq!(
        imports::evidence(&conn, &revised.evidence_id)
            .unwrap()
            .record
            .statement,
        revised.record.statement
    );
}

#[test]
fn compact_native_entry_point_handles_large_source_diagnostics() {
    let conn = database();
    let mut imported = inventory("Inspect the retry policy");
    imported.sources[0].batch.warnings = (0..1_000)
        .map(|i| format!("Source {i}: {}", "x".repeat(5_000)))
        .collect();
    imports::storage::persist(&conn, "project", &imported).unwrap();
    let mut pages = std::collections::BTreeMap::new();
    imports::render::augment(&conn, &mut pages).unwrap();
    let page = &pages["imports.md"];
    assert!(
        page.content.len() < 4_000_000,
        "the entry point must remain readable under the managed-page limit"
    );
    assert!(
        page.content
            .contains("source diagnostics were omitted or shortened")
    );
    assert_eq!(imports::warnings(&conn).unwrap().len(), 1_000);
}

#[test]
fn oversized_native_relationship_cannot_break_the_managed_page_limit() {
    use imports::relationships::{CrossSourceEndpoint, CrossSourceRelation};
    let conn = database();
    imports::storage::persist(&conn, "project", &inventory("Inspect retries")).unwrap();
    let observation = imports::views(&conn).unwrap().remove(0);
    let relation = CrossSourceRelation {
        id: "large-relation".into(),
        input_signature: "large-input".into(),
        from: CrossSourceEndpoint {
            kind: "observation".into(),
            id: observation.id.clone(),
            revision_id: observation.snapshot_id.clone(),
        },
        to: None,
        kind: "uncertain".into(),
        reason: "A source-native relationship with a very large description. ".repeat(80_000),
        qualifications: vec![],
        evidence_ids: vec![observation.evidence_id.clone()],
        upstream_kind: None,
        upstream_status: None,
        upstream_active: None,
        review_id: None,
        active: true,
    };
    conn.execute(
        "INSERT INTO cross_source_evaluations(input_signature,pair_key,from_kind,from_id,from_revision_id,disposition,recorded_at) VALUES('large-input','large-pair','observation',?1,?2,'uncertain','2026-10-09')",
        rusqlite::params![observation.id, observation.snapshot_id],
    ).unwrap();
    conn.execute(
        "INSERT INTO cross_source_relations VALUES('large-relation','large-input',?1)",
        [serde_json::to_string(&relation).unwrap()],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO cross_source_current VALUES('large-pair','large-input')",
        [],
    )
    .unwrap();
    let mut pages = std::collections::BTreeMap::new();
    imports::render::augment(&conn, &mut pages).unwrap();
    assert!(pages["imports.md"].content.len() < 4_000_000);
    assert!(
        pages["imports.md"]
            .content
            .contains("1 additional questions were omitted")
    );
    assert!(
        !pages["imports.md"]
            .content
            .contains(&observation.evidence_id)
    );
    assert_eq!(
        imports::relationships::relations(&conn).unwrap()[0],
        relation
    );
    assert!(imports::evidence(&conn, &observation.evidence_id).is_ok());
}

#[test]
fn rejects_dangling_record_pointers_and_cross_record_current_pointers() {
    let conn = database();
    let mut malformed = inventory("A recorded statement");
    malformed.sources[0].batch.records[0].evidence[0].field = Some("/missing".into());
    assert!(imports::storage::persist(&conn, "project", &malformed).is_err());
    let first = inventory("A recorded statement");
    imports::storage::persist(&conn, "project", &first).unwrap();
    let original = imports::views(&conn).unwrap().remove(0);
    conn.execute(
        "INSERT INTO native_records VALUES('other','work','other')",
        [],
    )
    .unwrap();
    assert!(
        conn.execute(
            "INSERT INTO native_current VALUES('other',?1)",
            [&original.snapshot_id]
        )
        .is_err()
    );
}

#[test]
fn source_kind_changes_need_a_new_native_namespace() {
    let conn = database();
    let first = inventory("Recorded observation");
    imports::storage::persist(&conn, "project", &first).unwrap();
    let mut next = first.clone();
    next.sources[0].kind = ImportKind::Engram;
    assert!(imports::storage::persist(&conn, "project", &next).is_err());
}

#[test]
fn version_five_registry_remains_readable_and_migrates_without_touching_history() {
    let conn = Connection::open_in_memory().unwrap();
    for sql in [
        storage::SCHEMA_V1,
        storage::SCHEMA_V2,
        storage::SCHEMA_V3,
        storage::SCHEMA_V4,
        storage::SCHEMA_V5,
    ] {
        conn.execute_batch(sql).unwrap();
    }
    assert!(imports::views(&conn).unwrap().is_empty());
    assert!(imports::warnings(&conn).unwrap().is_empty());
    storage::migrate(&conn).unwrap();
    storage::migrate(&conn).unwrap();
    assert_eq!(
        conn.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        storage::SCHEMA_VERSION
    );
}

fn native_project() -> (tempfile::TempDir, ResolvedConfig) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("lore.yml");
    let mut config = Config::default();
    config.schema_version = 2;
    config.sources.roots.clear();
    config.imports.push(ImportSource {
        id: "work".into(),
        kind: ImportKind::Beads,
        path: "issues.jsonl".into(),
        project: None,
        include_memories: false,
    });
    fs::write(&path, serde_yaml::to_string(&config).unwrap()).unwrap();
    fs::write(
        directory.path().join("issues.jsonl"),
        "{\"id\":\"work-7\",\"title\":\"Investigate retry policy\",\"status\":\"open\"}\n",
    )
    .unwrap();
    let resolved = ResolvedConfig::load(&path).unwrap();
    (directory, resolved)
}

#[tokio::test]
async fn native_only_update_is_idempotent_and_missing_input_never_retires_records() {
    let (_directory, config) = native_project();
    let model = common::FakeModel::new();
    let first = engine::update(&config, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    assert_eq!(first.imported_records, 1);
    assert_eq!(first.changed_imported_records, 1);
    let state = fs::read(config.state.join("state.db")).unwrap();
    let second = engine::update(&config, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    assert!(second.no_op);
    assert_eq!(second.model_calls, 0);
    assert_eq!(second.changed_imported_records, 0);
    assert_eq!(fs::read(config.state.join("state.db")).unwrap(), state);
    assert!(!engine::status(&config).unwrap().needs_update);
    fs::remove_file(config.base.join("issues.jsonl")).unwrap();
    assert!(
        engine::update(&config, &model, None, UpdateOptions::default())
            .await
            .is_err()
    );
    assert_eq!(fs::read(config.state.join("state.db")).unwrap(), state);
}

#[tokio::test]
async fn malformed_import_is_rejected_before_any_published_change() {
    let (_directory, config) = native_project();
    let model = common::FakeModel::new();
    engine::update(&config, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let baseline = fs::read(config.state.join("state.db")).unwrap();
    fs::write(config.base.join("issues.jsonl"), "{broken json}\n").unwrap();
    assert!(
        engine::update(&config, &model, None, UpdateOptions::default())
            .await
            .is_err()
    );
    assert_eq!(baseline, fs::read(config.state.join("state.db")).unwrap());
}

#[test]
fn import_config_is_versioned_optional_and_rejects_overlapping_ownership() {
    let (_directory, config) = native_project();
    let mut bad = config.config.clone();
    bad.schema_version = 1;
    assert!(ResolvedConfig::resolve(bad, &config.config_path).is_err());
    let mut bad = config.config.clone();
    bad.imports.push(bad.imports[0].clone());
    assert!(ResolvedConfig::resolve(bad, &config.config_path).is_err());
    let mut bad = config.config.clone();
    bad.imports[0].path = config.state.clone();
    assert!(ResolvedConfig::resolve(bad, &config.config_path).is_err());
    let mut bad = config.config.clone();
    bad.imports[0].project = Some("not-a-beads-filter".into());
    assert!(ResolvedConfig::resolve(bad, &config.config_path).is_err());
    let (_legacy, legacy, _) = common::project();
    assert!(legacy.config.imports.is_empty());
}
