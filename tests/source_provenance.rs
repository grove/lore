mod common;
use common::*;
use lore::{
    config::{ResolvedConfig, SourceRoot},
    domain::{AssertionProposal, EvidenceView, SourceMaterial},
    engine::{self, UpdateOptions},
    sources, storage,
};
use rusqlite::{Connection, params};
use std::{fs, sync::atomic::Ordering};

#[test]
fn provenance_defaults_are_compatible_and_invalid_metadata_is_rejected() {
    let legacy: SourceRoot = serde_yaml::from_str("id: docs\npath: ./docs\n").unwrap();
    assert_eq!(legacy.material, SourceMaterial::Primary);
    assert!(legacy.origin.is_none());
    assert!(
        serde_yaml::from_str::<SourceRoot>("id: wiki\npath: ./wiki\nmaterial: verified\n").is_err()
    );
    let legacy_evidence: EvidenceView = serde_json::from_value(serde_json::json!({
        "id":"ev", "assertion_id":"ar", "source_id":"src", "source":"docs:notes.md",
        "excerpt":"Original passage.", "captured_at":"2026-10-09", "active":true,
    }))
    .unwrap();
    assert_eq!(legacy_evidence.material, SourceMaterial::Primary);
    assert!(legacy_evidence.root_path.is_none());
    let (_dir, cfg, _) = project();
    for origin in [
        "".to_owned(),
        "repo\nforged-metadata".to_owned(),
        "x".repeat(2049),
    ] {
        let mut config = cfg.config.clone();
        config.sources.roots[0].origin = Some(origin);
        assert!(ResolvedConfig::resolve(config, &cfg.config_path).is_err());
    }
}

#[test]
fn read_only_configuration_needs_no_working_inference_or_online_sources() {
    let (_dir, cfg, _) = project();
    let mut config = cfg.config.clone();
    config.models.generative.enabled = false;
    config.models.generative.provider = "not-installed".into();
    config.processing.max_context_bytes = 0;
    config.processing.max_section_bytes = 0;
    fs::remove_dir(cfg.base.join("docs")).unwrap();
    fs::write(&cfg.config_path, serde_yaml::to_string(&config).unwrap()).unwrap();
    assert!(ResolvedConfig::load(&cfg.config_path).is_err());
    let read = ResolvedConfig::load_for_read(&cfg.config_path).unwrap();
    assert_eq!(read.config.project.name, cfg.config.project.name);
    assert!(!read.roots[0].1.exists());
    config.output.wiki_dir = ".".into();
    fs::write(&cfg.config_path, serde_yaml::to_string(&config).unwrap()).unwrap();
    assert!(ResolvedConfig::load_for_read(&cfg.config_path).is_err());
}

#[test]
fn provenance_changes_invalidate_sections_even_with_identical_markdown() {
    let (_dir, cfg, _) = project();
    put(&cfg, "guide.md", "# Guide\nThe worker runs serially.\n");
    let original = sources::scan(&cfg).unwrap();
    let mut config = cfg.config.clone();
    config.sources.roots[0].material = SourceMaterial::Derived;
    config.sources.roots[0].origin = Some("repository:worker-system".into());
    let derived = ResolvedConfig::resolve(config.clone(), &cfg.config_path).unwrap();
    let imported = sources::scan(&derived).unwrap();
    assert_eq!(original.documents[0].digest, imported.documents[0].digest);
    assert_ne!(original.digest, imported.digest);
    assert_ne!(
        original.documents[0].chunks[0].input_digest,
        imported.documents[0].chunks[0].input_digest
    );
    assert_eq!(imported.documents[0].material, SourceMaterial::Derived);
    config.sources.roots[0].origin = Some("repository:another-worker-system".into());
    let relocated = ResolvedConfig::resolve(config, &cfg.config_path).unwrap();
    let relocated = sources::scan(&relocated).unwrap();
    assert_ne!(
        imported.documents[0].chunks[0].input_digest,
        relocated.documents[0].chunks[0].input_digest
    );
}

#[tokio::test]
async fn ordinary_openwiki_markdown_preserves_multi_root_identity_and_captured_paths() {
    let (_dir, cfg, model) = project();
    let text = "# Worker guide\nDECISION workers: Run each queue with one consumer.\n";
    put(&cfg, "overview.md", text);
    fs::create_dir(cfg.base.join("openwiki")).unwrap();
    fs::write(cfg.base.join("openwiki/overview.md"), text).unwrap();
    let mut config = cfg.config.clone();
    config.sources.roots[0].origin = Some("repository:service-a".into());
    config.sources.roots.push(SourceRoot {
        id: "openwiki".into(),
        path: "./openwiki".into(),
        material: SourceMaterial::Derived,
        origin: Some("repository:service-b".into()),
    });
    let cfg = ResolvedConfig::resolve(config, &cfg.config_path).unwrap();
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let db = storage::read_only(&cfg.state.join("state.db")).unwrap();
    let units = storage::views(&db).unwrap();
    let evidence = units.iter().flat_map(|u| &u.evidence).collect::<Vec<_>>();
    let primary = evidence.iter().find(|e| e.root_id == "docs").unwrap();
    let derived = evidence.iter().find(|e| e.root_id == "openwiki").unwrap();
    assert_ne!(primary.source_id, derived.source_id);
    assert_eq!(primary.material, SourceMaterial::Primary);
    assert_eq!(derived.material, SourceMaterial::Derived);
    assert_eq!(derived.origin.as_deref(), Some("repository:service-b"));
    assert_eq!(derived.source, "openwiki:overview.md");
    assert_eq!(derived.line_start, Some(2));
    let derived_id = derived.id.clone();
    let derived_source = derived.source_id.clone();
    let primary_id = primary.id.clone();
    let exposed = engine::evidence(&cfg, &derived_id).unwrap();
    assert_eq!(exposed["material"], "derived");
    assert_eq!(exposed["root"], "openwiki");
    assert!(
        exposed["qualification"]
            .as_str()
            .unwrap()
            .contains("not independent primary evidence")
    );
    drop(db);

    fs::create_dir(cfg.base.join("openwiki/moved")).unwrap();
    fs::rename(
        cfg.base.join("openwiki/overview.md"),
        cfg.base.join("openwiki/moved/overview.md"),
    )
    .unwrap();
    let moved = engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    assert_eq!(moved.processed_sections, 0);
    let db = storage::read_only(&cfg.state.join("state.db")).unwrap();
    let current = storage::source_heads(&db).unwrap();
    let current = current.iter().find(|h| h.id == derived_source).unwrap();
    assert_eq!(current.path, "moved/overview.md");
    let snapshot = storage::evidence_snapshot(&db, &derived_id).unwrap();
    assert_eq!(snapshot.path, "overview.md");
    assert_eq!(snapshot.root_id, "openwiki");
    assert_eq!(snapshot.material, SourceMaterial::Derived);
    let wiki = fs::read_to_string(cfg.wiki.join("topics/workers.md")).unwrap();
    assert!(wiki.contains("openwiki/moved/overview.md"));
    assert!(wiki.contains("captured as openwiki:overview.md"));
    drop(db);

    fs::remove_file(cfg.base.join("docs/overview.md")).unwrap();
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let db = storage::read_only(&cfg.state.join("state.db")).unwrap();
    let units = storage::views(&db).unwrap();
    let evidence = units.iter().flat_map(|u| &u.evidence).collect::<Vec<_>>();
    assert!(
        evidence
            .iter()
            .any(|e| e.id == primary_id && !e.active && e.material == SourceMaterial::Primary)
    );
    assert!(
        evidence
            .iter()
            .filter(|e| e.active)
            .all(|e| e.material == SourceMaterial::Derived)
    );
    assert_eq!(
        storage::evidence_snapshot(&db, &derived_id)
            .unwrap()
            .source_id,
        derived_source
    );
}

#[tokio::test]
async fn changing_declared_material_recaptures_without_rewriting_old_provenance() {
    let (_dir, cfg, model) = project();
    put(
        &cfg,
        "guide.md",
        "# Worker guide\nDECISION workers: Run each queue with one consumer.\n",
    );
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let db = storage::read_only(&cfg.state.join("state.db")).unwrap();
    let original = storage::views(&db).unwrap()[0].evidence[0].clone();
    drop(db);
    let mut config = cfg.config.clone();
    config.sources.roots[0].material = SourceMaterial::Derived;
    config.sources.roots[0].origin = Some("generated:worker-guide".into());
    let cfg = ResolvedConfig::resolve(config, &cfg.config_path).unwrap();
    let status = engine::status(&cfg).unwrap();
    assert_eq!(status.changed_files, vec!["docs:guide.md"]);
    let changed = engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    assert_eq!(changed.processed_sections, 1);
    let db = storage::read_only(&cfg.state.join("state.db")).unwrap();
    let units = storage::views(&db).unwrap();
    let evidence = units.iter().flat_map(|u| &u.evidence).collect::<Vec<_>>();
    let old = evidence.iter().find(|e| e.id == original.id).unwrap();
    let new = evidence.iter().find(|e| e.active).unwrap();
    assert!(!old.active);
    assert_eq!(old.material, SourceMaterial::Primary);
    assert_eq!(old.origin, None);
    assert_eq!(new.material, SourceMaterial::Derived);
    assert_eq!(new.origin.as_deref(), Some("generated:worker-guide"));
    assert_eq!(old.source_id, new.source_id);
    assert_ne!(old.source_revision_id, new.source_revision_id);
    assert_eq!(old.excerpt, new.excerpt);
    let snapshot = storage::evidence_snapshot(&db, &old.id).unwrap();
    assert_eq!(snapshot.material, SourceMaterial::Primary);
    assert_eq!(snapshot.origin, None);
    drop(db);
    let calls = model.calls.load(Ordering::SeqCst);
    let unchanged = engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    assert!(unchanged.no_op);
    assert_eq!(unchanged.model_calls, 0);
    assert_eq!(calls, model.calls.load(Ordering::SeqCst));
}

#[test]
fn version_four_reads_and_migration_preserve_legacy_unknown_provenance() {
    let (_dir, cfg, _) = project();
    let quote = "Use one consumer for each queue.";
    put(&cfg, "guide.md", &format!("# Worker guide\n{quote}\n"));
    let inventory = sources::scan(&cfg).unwrap();
    let document = &inventory.documents[0];
    let chunk = &document.chunks[0];
    let db = Connection::open_in_memory().unwrap();
    for schema in [
        storage::SCHEMA_V1,
        storage::SCHEMA_V2,
        storage::SCHEMA_V3,
        storage::SCHEMA_V4,
    ] {
        db.execute_batch(schema).unwrap();
    }
    db.execute_batch("INSERT INTO projects VALUES('project','Legacy','2026-10-09'); INSERT INTO source_roots VALUES('docs','project','./docs'); INSERT INTO sources(id,root_id,relative_path) VALUES('source','docs','guide.md'); INSERT INTO source_revisions VALUES('revision','source','guide.md','digest','2026-10-09'); INSERT INTO source_current VALUES('source','revision','digest');").unwrap();
    storage::set_meta(&db, "initialized", "1").unwrap();
    let (section, section_revision) =
        storage::begin_section(&db, "source", "revision", chunk).unwrap();
    let proposal = AssertionProposal {
        topic: "workers".into(),
        topic_title: "Workers".into(),
        subject: "queues".into(),
        statement: quote.into(),
        kind: "constraint".into(),
        lifecycle: "active".into(),
        scope: "each queue".into(),
        effective_at: String::new(),
        quote: quote.into(),
    };
    let (assertion, evidence) = storage::capture_assertion(
        &db,
        storage::AssertionCapture {
            document,
            chunk,
            proposal: &proposal,
            source: "source",
            source_revision: "revision",
            section: &section,
            section_revision: &section_revision,
            model: "fixture",
        },
    )
    .unwrap();
    storage::create_unit(&db, "project", &assertion, &proposal).unwrap();
    let old = storage::evidence_snapshot(&db, &evidence).unwrap();
    assert_eq!(old.root_id, "docs");
    assert_eq!(old.path, "guide.md");
    assert_eq!(old.material, SourceMaterial::Primary);
    assert!(old.root_path.is_none());
    assert!(old.origin.is_none());
    let head = storage::source_heads(&db).unwrap().remove(0);
    assert!(head.root_path.is_none());
    let old_view = storage::views(&db).unwrap().remove(0);
    assert_eq!(old_view.evidence[0].source, "docs:guide.md");
    assert!(old_view.evidence[0].root_path.is_none());
    let version: i64 = db
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(
        version, 4,
        "read-only helpers must not migrate the database"
    );
    storage::migrate(&db).unwrap();
    let migrated = storage::evidence_snapshot(&db, &evidence).unwrap();
    assert!(migrated.root_path.is_none());
    assert_eq!(migrated.material, SourceMaterial::Primary);
    let count: i64 = db
        .query_row("SELECT count(*) FROM source_revision_provenance", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(
        count, 0,
        "migration must not fabricate previously unrecorded provenance"
    );

    assert!(db.execute("INSERT INTO source_revision_provenance VALUES('revision','docs','./docs','derived',?1)", ["generated:legacy-guide"]).is_err(), "previously unrecorded provenance cannot be inserted after evidence is sealed");
    db.execute("INSERT INTO source_revisions VALUES('new-revision','source','guide.md','digest','2026-10-10')", []).unwrap();
    db.execute("INSERT INTO source_revision_provenance VALUES('new-revision','docs','./docs','derived',?1)", ["generated:guide"]).unwrap();
    let changed = db
        .execute(
            "UPDATE source_revision_provenance SET material='primary' WHERE source_revision_id=?1",
            ["new-revision"],
        )
        .unwrap_err();
    assert!(changed.to_string().contains("immutable source provenance"));
    assert!(
        db.execute(
            "DELETE FROM source_revision_provenance WHERE source_revision_id=?1",
            ["new-revision"]
        )
        .is_err()
    );
    assert!(db.execute("INSERT OR REPLACE INTO source_revision_provenance VALUES('new-revision','docs','./docs','primary',NULL)", []).is_err());
    assert!(
        db.execute(
            "INSERT INTO source_revision_provenance VALUES(?1,'docs','./docs','primary',NULL)",
            params!["missing"]
        )
        .is_err()
    );
}
