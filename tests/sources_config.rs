mod common;
use common::*;
use lore::{
    config::{ResolvedConfig, SourceRoot},
    sources::{scan, split_markdown},
    util,
};
use std::fs;
#[tokio::test]
async fn renaming_an_initialized_project_preserves_identity_and_ownership() {
    use lore::{engine::{self, UpdateOptions}, publish, storage};

    let (_dir, mut cfg, model) = project();
    cfg.project_id = format!("project_{}", &util::digest(&cfg.config.project.name)[7..31]);
    put(&cfg, "ADR.md", "DECISION retry: Keep retries bounded.\n");
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let database = cfg.state.join("state.db");
    let before = fs::read(&database).unwrap();
    let owner_before = fs::read(cfg.state.join(".lore-owned.json")).unwrap();
    let mut renamed = cfg.config.clone();
    renamed.project.name = "HarborPay".into();
    fs::write(&cfg.config_path, serde_yaml::to_string(&renamed).unwrap()).unwrap();
    let loaded = ResolvedConfig::load(&cfg.config_path).unwrap();
    let read = ResolvedConfig::load_for_read(&cfg.config_path).unwrap();
    assert_eq!(loaded.project_id, cfg.project_id);
    assert_eq!(read.project_id, cfg.project_id);
    assert_eq!(read.config.project.name, "HarborPay");
    assert_eq!(fs::read(&database).unwrap(), before);
    assert_eq!(
        fs::read(cfg.state.join(".lore-owned.json")).unwrap(),
        owner_before
    );
    let lock = publish::ProjectLock::acquire(&loaded).unwrap();
    drop(lock);

    let (_other_dir, other, other_model) = project();
    put(
        &other,
        "ADR.md",
        "DECISION storage: Keep storage separate.\n",
    );
    engine::update(&other, &other_model, None, UpdateOptions::default())
        .await
        .unwrap();
    let mut mismatched = loaded.config.clone();
    mismatched.output.wiki_dir = other.wiki.clone();
    let mismatched = ResolvedConfig::resolve(mismatched, &cfg.config_path).unwrap();
    let conn = storage::read_only(&database).unwrap();
    let pages = storage::pages(&conn).unwrap();
    assert!(!pages.is_empty());
    // Publication ownership remains enforced without the browser reader.
    assert!(
        publish::validate_existing(&mismatched, &pages, false)
            .unwrap_err()
            .to_string()
            .contains("another Lore project")
    );
    let knowledge_before = lore::knowledge::build(&conn, &lore::knowledge::ZoomOptions::default())
        .unwrap()
        .knowledge;
    drop(conn);
    engine::update(&loaded, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let conn = storage::read_only(&database).unwrap();
    let (project_id, name): (String, String) = conn
        .query_row("SELECT id,name FROM projects", [], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .unwrap();
    assert_eq!(project_id, cfg.project_id);
    assert_eq!(name, "HarborPay");
    let knowledge_after = lore::knowledge::build(&conn, &lore::knowledge::ZoomOptions::default())
        .unwrap()
        .knowledge;
    assert_eq!(
        knowledge_after
            .iter()
            .map(|record| &record.id)
            .collect::<Vec<_>>(),
        knowledge_before
            .iter()
            .map(|record| &record.id)
            .collect::<Vec<_>>()
    );
}
#[test]
fn new_project_identity_does_not_depend_on_the_display_name() {
    let (_dir, cfg, _) = project();
    let mut renamed = cfg.config.clone();
    renamed.project.name = "Renamed".into();
    assert_eq!(
        ResolvedConfig::resolve(renamed, &cfg.config_path)
            .unwrap()
            .project_id,
        cfg.project_id
    );
    let (_other_dir, other, _) = project();
    assert_ne!(other.project_id, cfg.project_id);
}
#[test]
fn stored_identity_must_be_valid_bounded_and_not_a_symlink() {
    let (_dir, cfg, _) = project();
    fs::create_dir(&cfg.state).unwrap();
    let marker = cfg.state.join(".lore-owned.json");
    for contents in [
        "not json".to_string(),
        r#"{"project_id":"foreign","generation":""}"#.to_string(),
        " ".repeat(4097),
    ] {
        fs::write(&marker, contents).unwrap();
        assert!(ResolvedConfig::load(&cfg.config_path).is_err());
        assert!(ResolvedConfig::load_for_read(&cfg.config_path).is_err());
    }
    #[cfg(unix)]
    {
        fs::remove_file(&marker).unwrap();
        let outside = tempfile::NamedTempFile::new().unwrap();
        fs::write(
            outside.path(),
            serde_json::json!({"project_id":cfg.project_id,"generation":""}).to_string(),
        )
        .unwrap();
        std::os::unix::fs::symlink(outside.path(), &marker).unwrap();
        assert!(ResolvedConfig::load(&cfg.config_path).is_err());
    }
}
#[test]
fn markdown_parser_ignores_fenced_headings_and_disambiguates_repeated_ones() {
    let text =
        "# Intro\nText.\n```markdown\n# Not a heading\n```\n## Repeated\nOne.\n## Repeated\nTwo.\n";
    let chunks = split_markdown(text, "docs", "sample.md", 1000).unwrap();
    assert_eq!(chunks.len(), 3);
    assert_ne!(chunks[1].key, chunks[2].key);
    assert!(chunks[0].text.contains("Not a heading"));
}
#[test]
fn splits_large_unicode_markdown_without_losing_bytes() {
    let text = format!("# Notes\n{}", "Æøå project observations.\n".repeat(500));
    let chunks = split_markdown(&text, "docs", "notes.md", 512).unwrap();
    assert!(chunks.len() > 2);
    assert_eq!(
        chunks.iter().map(|c| c.text.as_str()).collect::<String>(),
        text
    );
    assert!(chunks.iter().all(|c| c.text.len() <= 512));
}
#[test]
fn missing_root_is_an_error_not_a_mass_deletion() {
    let (_dir, cfg, _) = project();
    fs::remove_dir(cfg.base.join("docs")).unwrap();
    assert!(scan(&cfg).is_err());
}
#[test]
fn generated_output_is_never_ingested_when_source_is_project_root() {
    let (_dir, cfg, _) = project();
    put(&cfg, "original.md", "Original evidence");
    fs::create_dir(&cfg.wiki).unwrap();
    fs::write(cfg.wiki.join("generated.md"), "Never source this").unwrap();
    fs::create_dir(&cfg.state).unwrap();
    fs::write(cfg.state.join("secret.md"), "Never source this either").unwrap();
    let mut config = cfg.config.clone();
    config.sources.roots = vec![SourceRoot {
        id: "project".into(),
        path: ".".into(),
        material: Default::default(),
        origin: None,
    }];
    let c = ResolvedConfig::resolve(config, &cfg.config_path).unwrap();
    let inventory = scan(&c).unwrap();
    assert_eq!(inventory.documents.len(), 1);
    assert_eq!(inventory.documents[0].relative_path, "docs/original.md");
}
#[test]
fn rejects_unsafe_output_paths_and_overlapping_source_ownership() {
    let (_dir, cfg, _) = project();
    let mut config = cfg.config.clone();
    config.output.wiki_dir = ".".into();
    assert!(ResolvedConfig::resolve(config, &cfg.config_path).is_err());
    let mut config = cfg.config.clone();
    config.sources.roots.push(SourceRoot {
        id: "nested".into(),
        path: "docs/nested".into(),
        material: Default::default(),
        origin: None,
    });
    assert!(ResolvedConfig::resolve(config, &cfg.config_path).is_err());
    assert!(util::safe_slug("../escape").is_err());
    assert!(util::safe_slug("index").is_err());
    assert!(util::safe_slug("database-decisions").is_ok());
}
#[cfg(unix)]
#[test]
fn refuses_source_and_output_symlink_traversal() {
    use std::os::unix::fs::symlink;
    let (_dir, cfg, _) = project();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("secret.md"), "secret").unwrap();
    symlink(outside.path(), cfg.base.join("docs/link")).unwrap();
    assert!(scan(&cfg).unwrap().documents.is_empty());
    symlink(outside.path(), &cfg.wiki).unwrap();
    assert!(ResolvedConfig::resolve(cfg.config.clone(), &cfg.config_path).is_err());
}
