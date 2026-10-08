mod common;
use common::*;
use lore::{
    engine::{self, UpdateOptions},
    publish, storage, util,
};
use std::{fs, sync::atomic::Ordering};

#[tokio::test]
async fn compiles_and_then_proves_a_zero_call_noop() {
    let (_dir, cfg, model) = project();
    put(
        &cfg,
        "ADR-001.md",
        "# Database\nDECISION database: MySQL is the selected database.\n",
    );
    let first = engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    assert!(!first.no_op);
    assert_eq!(first.knowledge_units, 1);
    let bytes = fs::read(cfg.wiki.join("topics/database.md")).unwrap();
    let calls = model.calls.load(Ordering::SeqCst);
    let second = engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    assert!(second.no_op);
    assert_eq!(second.model_calls, 0);
    assert_eq!(model.calls.load(Ordering::SeqCst), calls);
    assert_eq!(
        bytes,
        fs::read(cfg.wiki.join("topics/database.md")).unwrap()
    );
    assert_eq!(engine::audit(&cfg).unwrap()["ok"], true);
}
#[tokio::test]
async fn equivalent_sources_share_knowledge_but_keep_distinct_provenance() {
    let (_dir, cfg, model) = project();
    put(
        &cfg,
        "ADR-001.md",
        "# Database\nDECISION database: MySQL is the selected database.\n",
    );
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let old = storage::views(&storage::read_only(&cfg.state.join("state.db")).unwrap()).unwrap()[0]
        .id
        .clone();
    put(
        &cfg,
        "notes.md",
        "# Notes\nDECISION database: MySQL remains our database.\n",
    );
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let views = storage::views(&storage::read_only(&cfg.state.join("state.db")).unwrap()).unwrap();
    assert_eq!(views.len(), 1);
    assert_eq!(views[0].id, old);
    assert_eq!(views[0].evidence.iter().filter(|e| e.active).count(), 2);
    assert_ne!(
        views[0].evidence[0].source_id,
        views[0].evidence[1].source_id
    );
}
#[tokio::test]
async fn proposal_does_not_supersede_decision_and_unrelated_topic_is_unchanged() {
    let (_dir, cfg, model) = project();
    put(
        &cfg,
        "database.md",
        "# Database\nDECISION database: MySQL is the selected database.\n",
    );
    put(
        &cfg,
        "auth.md",
        "# Auth\nDECISION authentication: Access tokens expire after one hour.\n",
    );
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let auth = fs::read(cfg.wiki.join("topics/authentication.md")).unwrap();
    put(
        &cfg,
        "idea.md",
        "# Idea\nPLAN database: Consider a PostgreSQL migration.\n",
    );
    let report = engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    assert_eq!(report.processed_sections, 1);
    assert_eq!(
        auth,
        fs::read(cfg.wiki.join("topics/authentication.md")).unwrap()
    );
    let conn = storage::read_only(&cfg.state.join("state.db")).unwrap();
    let units = storage::views(&conn).unwrap();
    assert!(
        units
            .iter()
            .any(|u| u.kind == "proposal" && u.lifecycle == "proposed")
    );
    assert!(storage::relations(&conn).unwrap().is_empty());
}
#[tokio::test]
async fn new_document_can_supersede_unchanged_old_evidence_and_withdrawal_is_not_resurrection() {
    let (_dir, cfg, model) = project();
    put(
        &cfg,
        "ADR-001.md",
        "# Database\nDECISION database: MySQL is the selected database.\n",
    );
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    put(
        &cfg,
        "ADR-002.md",
        "# Replacement\nDECISION database: ADR-002 replaces ADR-001: PostgreSQL is the selected database.\n",
    );
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    {
        let conn = storage::read_only(&cfg.state.join("state.db")).unwrap();
        let views = storage::views(&conn).unwrap();
        assert!(
            views
                .iter()
                .any(|u| u.statement.contains("MySQL") && u.lifecycle == "superseded")
        );
        assert!(
            storage::relations(&conn)
                .unwrap()
                .iter()
                .any(|r| r.kind == "supersedes" && r.active)
        );
    }
    fs::remove_file(cfg.base.join("docs/ADR-002.md")).unwrap();
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let conn = storage::read_only(&cfg.state.join("state.db")).unwrap();
    let views = storage::views(&conn).unwrap();
    assert!(
        views
            .iter()
            .any(|u| u.statement.contains("MySQL") && u.support_state == "needs_review")
    );
    assert!(
        views
            .iter()
            .any(|u| u.statement.contains("PostgreSQL") && u.support_state == "historical_only")
    );
}
#[tokio::test]
async fn deleting_one_source_retains_other_support_and_deleting_all_preserves_snapshots() {
    let (_dir, cfg, model) = project();
    let text = "# Database\nDECISION database: MySQL is the selected database.\n";
    put(&cfg, "one.md", text);
    put(&cfg, "two.md", text);
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    fs::remove_file(cfg.base.join("docs/one.md")).unwrap();
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    {
        let conn = storage::read_only(&cfg.state.join("state.db")).unwrap();
        let u = storage::views(&conn).unwrap();
        assert_eq!(u.len(), 1);
        assert_eq!(u[0].support_state, "current_documentary_support");
    }
    fs::remove_file(cfg.base.join("docs/two.md")).unwrap();
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let conn = storage::read_only(&cfg.state.join("state.db")).unwrap();
    let u = storage::views(&conn).unwrap();
    assert_eq!(u[0].support_state, "historical_only");
    assert_eq!(u[0].evidence.len(), 2);
    let e = engine::evidence(&cfg, &u[0].evidence[0].id).unwrap();
    assert!(e["excerpt"].as_str().unwrap().contains("MySQL"));
}
#[tokio::test]
async fn failed_synthesis_preserves_database_and_wiki_and_retry_reuses_valid_work() {
    let (_dir, cfg, model) = project();
    put(
        &cfg,
        "base.md",
        "# Database\nDECISION database: MySQL is the selected database.\n",
    );
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let db = fs::read(cfg.state.join("state.db")).unwrap();
    let page = fs::read(cfg.wiki.join("topics/database.md")).unwrap();
    put(
        &cfg,
        "idea.md",
        "# Idea\nPLAN database: Consider PostgreSQL.\n",
    );
    model.fail_synthesis.store(true, Ordering::SeqCst);
    assert!(
        engine::update(&cfg, &model, None, UpdateOptions::default())
            .await
            .is_err()
    );
    assert_eq!(db, fs::read(cfg.state.join("state.db")).unwrap());
    assert_eq!(page, fs::read(cfg.wiki.join("topics/database.md")).unwrap());
    assert!(!publish::has_pending(&cfg));
    model.fail_synthesis.store(false, Ordering::SeqCst);
    let retry = engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    assert!(retry.cache_hits > 0);
}
#[tokio::test]
async fn evidence_quote_repair_receives_specific_validation_feedback() {
    let (_dir, cfg, model) = project();
    put(
        &cfg,
        "doc.md",
        "# Database\nDECISION database: MySQL is the selected database.\n",
    );
    model.invalid_quote.store(true, Ordering::SeqCst);
    model.repair_quote.store(true, Ordering::SeqCst);
    let report = engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    assert_eq!(report.extracted_assertions, 1);
    assert_eq!(engine::audit(&cfg).unwrap()["ok"], true);
}
#[test]
fn extraction_schema_only_offers_unique_verbatim_source_passages() {
    let (_dir, cfg, _) = project();
    put(
        &cfg,
        "doc.md",
        "# Database\n\nDECISION database: Use `MySQL`.\r\nPreserve this qualifier.\n\nRepeated.\n\nRepeated.\n",
    );
    let inventory = lore::sources::scan(&cfg).unwrap();
    let document = &inventory.documents[0];
    let chunk = &document.chunks[0];
    let schema = lore::domain::extraction_schema_for(&chunk.text);
    let properties = &schema["properties"]["assertions"]["items"]["properties"];
    let quotes = properties["quote"]["enum"].as_array().unwrap();
    assert!(quotes.iter().any(|quote| quote == &chunk.text));
    assert!(
        quotes
            .iter()
            .any(|quote| quote == "DECISION database: Use `MySQL`.")
    );
    assert!(!quotes.iter().any(|quote| quote == "Repeated."));
    assert!(
        quotes
            .iter()
            .any(|quote| quote.as_str().unwrap().contains("\r\nPreserve"))
    );
    for quote in quotes {
        lore::sources::locate_quote(document, chunk, quote.as_str().unwrap()).unwrap();
    }
    assert!(properties["topic"]["pattern"].is_string());
    assert!(properties["kind"]["enum"].is_array());
    assert!(properties["lifecycle"]["enum"].is_array());
    let reconciliation = lore::domain::reconciliation_schema();
    assert_eq!(
        reconciliation["properties"]["relations"]["items"]["properties"]["kind"]["enum"],
        serde_json::json!([
            "elaborates",
            "contradicts",
            "supersedes",
            "reaffirms",
            "uncertain"
        ])
    );
    let targets = ["unit-example".to_owned()].into_iter().collect();
    let reconciliation = lore::domain::reconciliation_schema_for(&targets, &chunk.text);
    assert_eq!(
        reconciliation["properties"]["equivalent_to"]["enum"],
        serde_json::json!(["", "unit-example"])
    );
    let properties = &reconciliation["properties"]["relations"]["items"]["properties"];
    assert_eq!(
        properties["target_id"]["enum"],
        serde_json::json!(["unit-example"])
    );
    for quote in properties["quote"]["enum"].as_array().unwrap() {
        let quote = quote.as_str().unwrap();
        if !quote.is_empty() {
            lore::sources::locate_quote(document, chunk, quote).unwrap();
        }
    }
}
#[tokio::test]
#[ignore = "requires local Ollama and LORE_LIVE_CONFIG/LORE_LIVE_SECTION"]
async fn local_ollama_extracts_selected_source_section() {
    let path = std::env::var("LORE_LIVE_CONFIG").unwrap();
    let heading = std::env::var("LORE_LIVE_SECTION").unwrap();
    let original = lore::config::ResolvedConfig::load(std::path::Path::new(&path)).unwrap();
    assert!(original.config.privacy.local_only);
    let inventory = lore::sources::scan(&original).unwrap();
    let chunk = inventory
        .documents
        .iter()
        .flat_map(|document| &document.chunks)
        .find(|chunk| chunk.heading == heading)
        .expect("requested source section must exist");
    let (_dir, fixture, _) = project();
    let mut config = original.config.clone();
    config.sources = fixture.config.sources.clone();
    config.output = fixture.config.output.clone();
    config.models.decision = None;
    let config = lore::config::ResolvedConfig::resolve(config, &fixture.config_path).unwrap();
    put(&config, "probe.md", &chunk.text);
    let model = lore::http::HttpModel::new(&config, &config.config.models.generative).unwrap();
    let report = engine::update(&config, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    assert!(report.extracted_assertions > 0);
    assert_eq!(engine::audit(&config).unwrap()["ok"], true);
}
#[tokio::test]
async fn invented_evidence_is_rejected_before_publication() {
    let (_dir, cfg, model) = project();
    put(
        &cfg,
        "doc.md",
        "# Database\nDECISION database: MySQL is the selected database.\n",
    );
    model.invalid_quote.store(true, Ordering::SeqCst);
    assert!(
        engine::update(&cfg, &model, None, UpdateOptions::default())
            .await
            .is_err()
    );
    assert!(!cfg.wiki.exists());
    assert!(!cfg.state.join("state.db").exists());
}
#[tokio::test]
async fn reordered_sections_and_renamed_files_preserve_identity_without_reextraction() {
    let (_dir, cfg, model) = project();
    let a = "# Database\nDECISION database: MySQL is the selected database.\n";
    let b = "# Auth\nDECISION authentication: Access tokens expire.\n";
    put(&cfg, "notes.md", &format!("{a}{b}"));
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let original = storage::source_heads(&storage::read_only(&cfg.state.join("state.db")).unwrap())
        .unwrap()[0]
        .id
        .clone();
    put(&cfg, "notes.md", &format!("{b}{a}"));
    let report = engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    assert_eq!(report.processed_sections, 0);
    fs::create_dir(cfg.base.join("docs/moved")).unwrap();
    fs::rename(
        cfg.base.join("docs/notes.md"),
        cfg.base.join("docs/moved/notes.md"),
    )
    .unwrap();
    let report = engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    assert_eq!(report.processed_sections, 0);
    let heads =
        storage::source_heads(&storage::read_only(&cfg.state.join("state.db")).unwrap()).unwrap();
    assert_eq!(heads[0].id, original);
}
#[tokio::test]
async fn frontmatter_change_invalidates_descendant_sections() {
    let (_dir, cfg, model) = project();
    let body = "# Database\nDECISION database: MySQL is the selected database.\n";
    put(
        &cfg,
        "doc.md",
        &format!("---\nstatus: accepted\n---\n{body}"),
    );
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    put(
        &cfg,
        "doc.md",
        &format!("---\nstatus: rejected\n---\n{body}"),
    );
    let r = engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    assert!(r.processed_sections >= 2);
    let units = storage::views(&storage::read_only(&cfg.state.join("state.db")).unwrap()).unwrap();
    assert!(units.iter().any(|u| u.base_lifecycle == "rejected"));
}
#[tokio::test]
async fn detects_output_edits_and_requires_explicit_rebuild() {
    let (_dir, cfg, model) = project();
    put(
        &cfg,
        "doc.md",
        "# Database\nDECISION database: MySQL is the selected database.\n",
    );
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    fs::write(cfg.wiki.join("topics/database.md"), "human edit").unwrap();
    let calls = model.calls.load(Ordering::SeqCst);
    assert!(engine::status(&cfg).unwrap().output_modified);
    assert!(
        engine::update(&cfg, &model, None, UpdateOptions::default())
            .await
            .is_err()
    );
    assert_eq!(calls, model.calls.load(Ordering::SeqCst));
    engine::update(
        &cfg,
        &model,
        None,
        UpdateOptions {
            rebuild: true,
            ..UpdateOptions::default()
        },
    )
    .await
    .unwrap();
    assert_ne!(
        fs::read_to_string(cfg.wiki.join("topics/database.md")).unwrap(),
        "human edit"
    );
}
#[tokio::test]
async fn audit_search_and_purge_operate_without_models() {
    let (_dir, cfg, model) = project();
    put(
        &cfg,
        "doc.md",
        "# Database\nDECISION database: MySQL is the selected database.\n",
    );
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    {
        let conn = storage::read_only(&cfg.state.join("state.db")).unwrap();
        assert_eq!(storage::search(&conn, "MySQL", 10).unwrap().len(), 1);
    }
    assert_eq!(engine::audit(&cfg).unwrap()["ok"], true);
    publish::purge_all(&cfg).unwrap();
    assert!(!cfg.wiki.exists());
    assert!(!cfg.state.join("state.db").exists());
    assert!(!cfg.state.join("cache").exists());
    assert!(cfg.base.join("docs/doc.md").exists());
}
#[tokio::test]
async fn recovers_crash_after_wiki_switch_before_database_switch() {
    let (_dir, cfg, model) = project();
    put(
        &cfg,
        "doc.md",
        "# Database\nDECISION database: MySQL is the selected database.\n",
    );
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let generation = "run_crashfixture";
    let stage = publish::stage_dir(&cfg, generation);
    util::private_dir(&stage).unwrap();
    {
        let conn =
            storage::stage_database(&cfg.state.join("state.db"), &stage.join("state.db")).unwrap();
        storage::set_meta(&conn, "generation", generation).unwrap();
    }
    let wiki_stage = cfg
        .wiki
        .parent()
        .unwrap()
        .join(format!(".lore-stage-{generation}"));
    util::private_dir(&wiki_stage).unwrap();
    {
        let conn = storage::read_only(&stage.join("state.db")).unwrap();
        for p in storage::pages(&conn).unwrap().values() {
            util::atomic_write(
                &publish::page_path(&wiki_stage, &p.path).unwrap(),
                p.content.as_bytes(),
            )
            .unwrap();
        }
    }
    fs::write(
        wiki_stage.join(".lore-owned.json"),
        serde_json::json!({"project_id":cfg.project_id,"generation":generation}).to_string(),
    )
    .unwrap();
    fs::write(
        cfg.state.join("publication.json"),
        serde_json::json!({"project_id":cfg.project_id,"generation":generation,"wiki":cfg.wiki})
            .to_string(),
    )
    .unwrap();
    let backup = cfg
        .wiki
        .parent()
        .unwrap()
        .join(format!(".lore-backup-{generation}"));
    fs::rename(&cfg.wiki, &backup).unwrap();
    fs::rename(wiki_stage, &cfg.wiki).unwrap();
    assert!(publish::recover(&cfg).unwrap());
    assert!(!publish::has_pending(&cfg));
    assert!(!backup.exists());
    assert_eq!(
        storage::meta(
            &storage::read_only(&cfg.state.join("state.db")).unwrap(),
            "generation"
        )
        .unwrap()
        .as_deref(),
        Some(generation)
    );
}
