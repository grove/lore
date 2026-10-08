mod common;
use common::*;
use lore::{
    config::{ResolvedConfig, SourceRoot},
    sources::{scan, split_markdown},
    util,
};
use std::fs;
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
