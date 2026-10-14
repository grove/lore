mod common;

use lore::{
    config::{Config, ImportSource, OutputMode, ResolvedConfig},
    context::{self, ContextOptions},
    domain::ImportKind,
    engine::{self, UpdateOptions, UpdateProgress},
    imports::{
        self, Inventory, SourceBatch,
        adapters::{AdapterRecord, ImportBatch, NativeEvidence, NativeRelationship},
        relationships::{self, CrossSourceEndpoint, CrossSourceRelation},
    },
    reviews, storage, util,
};
use rusqlite::{Connection, params};
use serde_json::{Value, json};
use std::{fs, path::Path, process::Command};

fn legacy_database(path: &Path, project: &str) -> Connection {
    let conn = Connection::open(path).unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    for sql in [
        storage::SCHEMA_V1,
        storage::SCHEMA_V2,
        storage::SCHEMA_V3,
        storage::SCHEMA_V4,
        storage::SCHEMA_V5,
        storage::SCHEMA_V6,
        storage::SCHEMA_V7,
        storage::SCHEMA_V8,
        storage::SCHEMA_V9,
    ] {
        conn.execute_batch(sql).unwrap();
    }
    conn.execute(
        "INSERT INTO projects VALUES(?1,'Project','2026-10-01')",
        [project],
    )
    .unwrap();
    conn
}

// Normalized legacy snapshots exercise storage compatibility without retaining a retired parser.
fn inventory(statement: &str, withdrawn: bool) -> Inventory {
    let mut memory = AdapterRecord::new(
        "observation:17",
        statement,
        json!({"id":17,"title":"Retry discovery","content":statement,"extension":{"kept":true}}),
    );
    memory.lifecycle = "active".into();
    memory.evidence.push(NativeEvidence {
        locator: "engram://observation:17".into(),
        revision: None,
        field: Some("/content".into()),
    });
    memory.relationships.push(NativeRelationship {
        native_id: Some("relation-1".into()), target_native_id: "old-memory".into(), kind: "supersedes".into(),
        upstream_status: Some("judged".into()), active: true,
        native_record: json!({"sync_id":"relation-1","relation":"supersedes","judgment_status":"judged"}),
    });
    let mut old = memory.clone();
    old.native_id = "old-memory".into();
    old.lifecycle = "deleted".into();
    old.relationships.clear();
    let mut work = AdapterRecord::new(
        "work-1",
        "Review retries",
        json!({"title":"Review retries"}),
    );
    work.kind = imports::adapters::ObservationKind::WorkState;
    work.evidence.push(NativeEvidence {
        locator: "beads://work-1".into(),
        revision: None,
        field: Some("/title".into()),
    });
    let sources = [
        (
            "memory",
            ImportKind::Engram,
            "engram-json/0.2.0",
            if withdrawn {
                vec![memory]
            } else {
                vec![memory, old]
            },
        ),
        ("work", ImportKind::Beads, "beads-issue-jsonl", vec![work]),
    ]
    .into_iter()
    .map(|(id, kind, format, records)| SourceBatch {
        id: id.into(),
        kind,
        path: format!("/offline/{id}.json").into(),
        digest: statement.into(),
        batch: ImportBatch {
            format: format.into(),
            records,
            warnings: if kind == ImportKind::Engram {
                vec!["Source export omits deleted history.".into()]
            } else {
                vec![]
            },
        },
    })
    .collect();
    Inventory {
        sources,
        ..Inventory::default()
    }
}

fn snapshots(conn: &Connection) -> Vec<(String, Value)> {
    conn.prepare("SELECT evidence_id,record_json FROM native_snapshots ORDER BY evidence_id")
        .unwrap()
        .query_map([], |r| {
            Ok((
                r.get(0)?,
                serde_json::from_str(&r.get::<_, String>(1)?).unwrap(),
            ))
        })
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

fn comparison(
    conn: &Connection,
    name: &str,
    from: &imports::ImportedObservation,
    to: Option<&imports::ImportedObservation>,
) {
    let endpoint = |o: &imports::ImportedObservation| CrossSourceEndpoint {
        kind: "observation".into(),
        id: o.id.clone(),
        revision_id: o.snapshot_id.clone(),
    };
    let kind = "potential_discrepancy";
    let relation = CrossSourceRelation {
        id: format!("xrel_{}", &util::digest(format!("{name}:{kind}"))[7..]),
        input_signature: name.into(),
        from: endpoint(from),
        to: to.map(endpoint),
        kind: kind.into(),
        reason: "Reported sources may disagree; verify policy.".into(),
        qualifications: vec!["Recollection is not accepted policy.".into()],
        evidence_ids: std::iter::once(from.evidence_id.clone())
            .chain(to.map(|o| o.evidence_id.clone()))
            .collect(),
        upstream_kind: None,
        upstream_status: None,
        upstream_active: None,
        review_id: None,
        active: true,
    };
    conn.execute("INSERT INTO cross_source_evaluations VALUES(?1,?1,'observation',?2,?3,?4,?5,?6,?7,'2026-10-01')",
        params![name, from.id, from.snapshot_id, to.map(|_| "observation"), to.map(|o| &o.id), to.map(|o| &o.snapshot_id), kind]).unwrap();
    conn.execute(
        "INSERT INTO cross_source_relations VALUES(?1,?2,?3)",
        params![relation.id, name, serde_json::to_string(&relation).unwrap()],
    )
    .unwrap();
    conn.execute("INSERT INTO cross_source_current VALUES(?1,?1)", [name])
        .unwrap();
}

#[test]
fn schema_nine_migration_preserves_every_revision_withdrawal_and_relationship_in_both_directions() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().canonicalize().unwrap().join("state.db");
    let conn = legacy_database(&path, "project");
    imports::storage::persist(&conn, "project", &inventory("Retries were five", false)).unwrap();
    imports::storage::persist(
        &conn,
        "project",
        &inventory("Retries need policy review", true),
    )
    .unwrap();
    let before = snapshots(&conn);
    let observations = imports::views(&conn).unwrap();
    let memory = observations
        .iter()
        .find(|o| o.record.native_id == "observation:17")
        .unwrap();
    let work = observations
        .iter()
        .find(|o| o.origin == ImportKind::Beads)
        .unwrap();
    comparison(&conn, "from-engram", memory, Some(work));
    comparison(&conn, "to-engram", work, Some(memory));
    comparison(&conn, "unrelated", work, None);
    reviews::record(&conn, "project", "engram-history", "Review recollection").unwrap();
    let review = reviews::list(&conn, true).unwrap().remove(0);
    conn.execute(
        "INSERT INTO cross_source_review_context VALUES(?1,'from-engram')",
        [&review.id],
    )
    .unwrap();
    reviews::manual(
        &conn,
        &review.id,
        "dismissed",
        "Historical recollection only",
        "reviewer",
    )
    .unwrap();
    let events = serde_json::to_value(reviews::events(&conn, &review.id).unwrap()).unwrap();
    let relations = relationships::history(&conn).unwrap();
    assert!(imports::storage::audit(&conn).unwrap().is_empty());
    assert!(relationships::audit(&conn).unwrap().is_empty());
    drop(conn);
    let backup = fs::read(&path).unwrap();

    let readonly = storage::read_only(&path).unwrap();
    for (id, _) in &before {
        imports::evidence(&readonly, id).unwrap();
    }
    assert_eq!(fs::read(&path).unwrap(), backup);
    drop(readonly);
    let conn = Connection::open(&path).unwrap();
    storage::migrate(&conn).unwrap();
    storage::migrate(&conn).unwrap();
    assert_eq!(snapshots(&conn), before);
    for (id, record) in &before {
        let observation = imports::evidence(&conn, id).unwrap();
        assert_eq!(serde_json::to_value(&observation.record).unwrap(), *record);
        if observation.origin == ImportKind::Engram {
            assert!(!observation.current);
        }
    }
    assert!(
        imports::views(&conn)
            .unwrap()
            .iter()
            .any(|o| o.origin == ImportKind::Beads && o.current)
    );
    assert_eq!(relationships::relations(&conn).unwrap().len(), 1);
    for mut relation in relations {
        relation.active = relation.input_signature == "unrelated";
        assert!(relationships::history(&conn).unwrap().contains(&relation));
    }
    assert_eq!(
        serde_json::to_value(reviews::events(&conn, &review.id).unwrap()).unwrap(),
        events
    );
    assert_eq!(reviews::list(&conn, true).unwrap()[0].status, "dismissed");
    assert!(imports::storage::audit(&conn).unwrap().is_empty());
    assert!(relationships::audit(&conn).unwrap().is_empty());
    let context = context::build_context(
        &conn,
        &ContextOptions {
            task: "Retries observation:17".into(),
            paths: vec![],
            max_tokens: 6000,
        },
    )
    .unwrap();
    assert!(context.imported_observations.iter().any(|o| {
        o.origin == ImportKind::Engram
            && !o.current
            && o.qualifications
                .iter()
                .any(|q| q.contains("read-only history"))
    }));
    drop(conn);

    // Rollback restores the matched pre-upgrade backup, never just user_version.
    fs::write(&path, &backup).unwrap();
    let conn = storage::read_only(&path).unwrap();
    assert_eq!(
        conn.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        9
    );
    assert_eq!(snapshots(&conn), before);
    assert_eq!(relationships::relations(&conn).unwrap().len(), 3);
    assert!(
        imports::evidence(&conn, &memory.evidence_id)
            .unwrap()
            .current
    );
    assert!(
        !imports::views(&conn)
            .unwrap()
            .iter()
            .find(|o| o.record.native_id == "old-memory")
            .unwrap()
            .current
    );
}

#[tokio::test]
async fn existing_project_upgrades_without_reading_engram_and_failed_staging_preserves_backup() {
    let directory = tempfile::tempdir().unwrap();
    let config_path = directory.path().join("lore.yml");
    let mut config = Config {
        schema_version: 2,
        ..Config::default()
    };
    config.output.mode = Some(OutputMode::RegistryOnly);
    config.sources.roots.clear();
    config.imports.push(ImportSource {
        id: "memory".into(),
        kind: ImportKind::Engram,
        path: "missing.json".into(),
        project: None,
        include_memories: false,
    });
    fs::write(&config_path, serde_yaml::to_string(&config).unwrap()).unwrap();
    let config = ResolvedConfig::load(&config_path).unwrap();
    let error = imports::scan(&config).unwrap_err().to_string();
    assert!(error.contains("retired") && error.contains("Markdown"));
    let first_contact = Command::new(env!("CARGO_BIN_EXE_lore"))
        .current_dir(&config.base)
        .args(["--json", "context", "Retries"])
        .output()
        .unwrap();
    assert!(
        first_contact.status.success(),
        "{}",
        String::from_utf8_lossy(&first_contact.stderr)
    );
    let first_contact: Value = serde_json::from_slice(&first_contact.stdout).unwrap();
    assert!(
        first_contact["limitations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|warning| warning
                .as_str()
                .unwrap()
                .contains("Native Engram imports are retired"))
    );
    assert!(!config.state.exists());
    drop(lore::publish::ProjectLock::acquire(&config).unwrap());
    let path = config.state.join("state.db");
    let conn = legacy_database(&path, &config.project_id);
    imports::storage::persist(
        &conn,
        &config.project_id,
        &inventory("Retries need review", true),
    )
    .unwrap();
    storage::set_meta(&conn, "initialized", "1").unwrap();
    storage::set_meta(&conn, "output_mode", "registry_only").unwrap();
    storage::set_meta(&conn, "config_digest", &config.fingerprint).unwrap();
    let before = snapshots(&conn);
    drop(conn);
    let backup = fs::read(&path).unwrap();
    let id = &before[0].0;
    let read = Command::new(env!("CARGO_BIN_EXE_lore"))
        .args([
            "--config",
            config.config_path.to_str().unwrap(),
            "--json",
            "evidence",
            id,
        ])
        .output()
        .unwrap();
    assert!(
        read.status.success(),
        "{}",
        String::from_utf8_lossy(&read.stderr)
    );
    let evidence: Value = serde_json::from_slice(&read.stdout).unwrap();
    assert_eq!(evidence["evidence_id"], *id);
    assert_eq!(fs::read(&path).unwrap(), backup);
    // Abort after the staged migration/persist, before anything can publish.
    let model = common::FakeModel::new();
    let failed = engine::update_with_progress(
        &config,
        &model,
        None,
        UpdateOptions::default(),
        |progress| {
            if progress == UpdateProgress::Phase("Extracting source sections") {
                anyhow::bail!("fixture interruption");
            }
            Ok(())
        },
    )
    .await;
    assert!(
        failed
            .unwrap_err()
            .to_string()
            .contains("fixture interruption")
    );
    assert_eq!(fs::read(&path).unwrap(), backup);
    assert!(engine::status(&config).unwrap().needs_update);
    engine::update(&config, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let conn = storage::read_only(&path).unwrap();
    assert_eq!(snapshots(&conn), before);
    assert!(
        imports::warnings(&conn)
            .unwrap()
            .iter()
            .any(|warning| warning.contains("omits deleted history"))
    );
    drop(conn);
    assert!(
        engine::audit(&config).unwrap()["issues"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    for (id, _) in &before {
        assert_eq!(engine::evidence(&config, id).unwrap()["current"], false);
    }
    fs::write(config.base.join("missing.json"), "malformed and ignored").unwrap();
    assert!(
        engine::update(&config, &model, None, UpdateOptions::default())
            .await
            .unwrap()
            .no_op
    );
    let mut fresh = config.config.clone();
    fresh.imports[0].id = "new-memory".into();
    let fresh = ResolvedConfig::resolve(fresh, &config_path).unwrap();
    assert!(
        imports::scan(&fresh)
            .unwrap_err()
            .to_string()
            .contains("retired")
    );
    let mut without_engram = config.config.clone();
    without_engram.imports.clear();
    without_engram.sources.roots = Config::default().sources.roots;
    fs::create_dir(config.base.join("docs")).unwrap();
    fs::write(
        &config_path,
        serde_yaml::to_string(&without_engram).unwrap(),
    )
    .unwrap();
    let without_engram = ResolvedConfig::load_for_read(&config_path).unwrap();
    engine::update(&without_engram, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    for (id, _) in &before {
        assert_eq!(
            engine::evidence(&without_engram, id).unwrap()["current"],
            false
        );
    }
    assert!(
        imports::warnings(&storage::read_only(&path).unwrap())
            .unwrap()
            .iter()
            .any(|warning| warning.contains("omits deleted history"))
    );
}
