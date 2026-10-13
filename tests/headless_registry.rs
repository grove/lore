//! Headless compilation, legacy publication migration, and exact evidence safety.
mod common;

use common::*;
use lore::{
    config::{OutputMode, ResolvedConfig},
    context,
    engine::{self, UpdateOptions, UpdateProgress},
    publish, reviews, storage,
};
use serde_json::Value;
use std::{
    fs,
    process::{Command, Output},
    sync::atomic::Ordering,
};

fn headless_project() -> (tempfile::TempDir, ResolvedConfig, FakeModel) {
    let (dir, mut config, model) = project();
    config.config.output.mode = Some(OutputMode::RegistryOnly);
    fs::write(
        &config.config_path,
        serde_yaml::to_string(&config.config).unwrap(),
    )
    .unwrap();
    let config = ResolvedConfig::load(&config.config_path).unwrap();
    (dir, config, model)
}

fn cli(dir: &std::path::Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_lore"))
        .current_dir(dir)
        .args(["--json"])
        .args(args)
        .env_remove("OPENAI_API_KEY")
        .env_remove("TYPESAFE_API_KEY")
        .env_remove("LORE_ALLOW_CHECKOUT_EGRESS")
        .env_remove("LORE_ALLOW_HOSTED_EGRESS")
        .env_remove("LORE_INSPECTION_ROOT")
        .output()
        .unwrap()
}

fn read_json(out: Output) -> Value {
    assert!(
        out.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap()
}

#[test]
fn new_cli_config_is_headless_but_explicit_markdown_is_available() {
    for (flag, mode) in [
        (false, OutputMode::RegistryOnly),
        (true, OutputMode::Markdown),
    ] {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("docs")).unwrap();
        fs::write(
            dir.path().join("docs/decision.md"),
            "DECISION queue: Preserve queue order.\n",
        )
        .unwrap();
        let mut args = vec!["init", "--configure-only"];
        if flag {
            args.push("--publish-markdown");
        }
        let configured = read_json(cli(dir.path(), &args));
        assert_eq!(configured["configured"], true);
        let cfg = ResolvedConfig::load_for_read(&dir.path().join("lore.yml")).unwrap();
        assert_eq!(cfg.config.output.mode(), mode);
        assert!(!cfg.wiki.exists());
        assert!(!cfg.state.exists());
    }
}

#[tokio::test]
async fn headless_registry_has_all_read_only_cli_surfaces_and_genuine_no_op() {
    let (_dir, cfg, model) = headless_project();
    put(
        &cfg,
        "decision.md",
        "DECISION queue: Preserve the accepted FIFO admission order.\n",
    );
    let first = engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    assert!(!first.no_op && first.changed_pages == 0);
    assert!(!cfg.wiki.exists(), "headless compilation wrote a wiki");
    let db = cfg.state.join("state.db");
    let conn = storage::read_only(&db).unwrap();
    assert_eq!(
        publish::stored_mode(&conn).unwrap(),
        OutputMode::RegistryOnly
    );
    assert!(storage::pages(&conn).unwrap().is_empty());
    assert!(!storage::views(&conn).unwrap().is_empty());
    let evidence = storage::views(&conn).unwrap()[0].evidence[0].id.clone();
    drop(conn);

    let before = fs::read(&db).unwrap();
    let calls = model.calls.load(Ordering::SeqCst);
    let next = engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    assert!(next.no_op && next.model_calls == 0 && next.changed_pages == 0);
    assert_eq!(calls, model.calls.load(Ordering::SeqCst));
    assert_eq!(before, fs::read(&db).unwrap());
    assert!(!cfg.wiki.exists());

    let root = &cfg.base;
    for args in [
        vec!["context", "Preserve queue order", "--fast"],
        vec![
            "context",
            "Preserve queue order",
            "--no-inspect",
            "--no-cache",
        ],
        vec!["evidence", evidence.as_str()],
        vec!["decisions", "queue"],
        vec!["cases", "queue"],
        vec!["search", "queue"],
        vec!["review", "list"],
        vec!["status"],
        vec!["audit"],
    ] {
        let value = read_json(cli(root, &args));
        assert!(value.is_object(), "{args:?}");
    }
    let state = engine::status(&cfg).unwrap();
    assert!(state.initialized && !state.needs_update && !state.output_modified);
    assert_eq!(state.output_mode, OutputMode::RegistryOnly);
    assert_eq!(fs::read(&db).unwrap(), before);
}

#[tokio::test]
async fn legacy_wiki_is_preserved_until_explicit_headless_migration() {
    let (_dir, mut cfg, model) = project(); // Old output.mode is absent.
    put(
        &cfg,
        "queue.md",
        "DECISION queue: The queue must preserve order.\n",
    );
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    assert!(cfg.wiki.join("index.md").exists());
    let wiki_before = fs::read(cfg.wiki.join("index.md")).unwrap();
    let db_before = fs::read(cfg.state.join("state.db")).unwrap();

    cfg.config.output.mode = Some(OutputMode::RegistryOnly);
    fs::write(
        &cfg.config_path,
        serde_yaml::to_string(&cfg.config).unwrap(),
    )
    .unwrap();
    let cfg = ResolvedConfig::load(&cfg.config_path).unwrap();
    let state = engine::status(&cfg).unwrap();
    assert!(state.mode_change_required && state.needs_update);
    let calls = model.calls.load(Ordering::SeqCst);
    assert!(
        engine::update(&cfg, &model, None, UpdateOptions::default())
            .await
            .is_err()
    );
    assert_eq!(calls, model.calls.load(Ordering::SeqCst));
    assert_eq!(db_before, fs::read(cfg.state.join("state.db")).unwrap());
    assert_eq!(wiki_before, fs::read(cfg.wiki.join("index.md")).unwrap());

    let migrated = engine::update(
        &cfg,
        &model,
        None,
        UpdateOptions {
            migrate_output: true,
            ..UpdateOptions::default()
        },
    )
    .await
    .unwrap();
    assert!(!migrated.no_op && migrated.changed_pages == 0);
    assert_eq!(wiki_before, fs::read(cfg.wiki.join("index.md")).unwrap());
    assert_eq!(
        publish::stored_mode(&storage::read_only(&cfg.state.join("state.db")).unwrap()).unwrap(),
        OutputMode::RegistryOnly
    );
    assert!(publish::restore(&cfg).is_err());
    put(
        &cfg,
        "new.md",
        "DECISION queue: Emergency drains are exceptions.\n",
    );
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    assert_eq!(wiki_before, fs::read(cfg.wiki.join("index.md")).unwrap());
    assert!(!engine::status(&cfg).unwrap().needs_update);
}

#[tokio::test]
async fn headless_to_markdown_is_explicit_and_restorable() {
    let (_dir, mut cfg, model) = headless_project();
    put(
        &cfg,
        "queue.md",
        "DECISION queue: Preserve accepted order.\n",
    );
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    assert!(!cfg.wiki.exists());
    cfg.config.output.mode = Some(OutputMode::Markdown);
    fs::write(
        &cfg.config_path,
        serde_yaml::to_string(&cfg.config).unwrap(),
    )
    .unwrap();
    let cfg = ResolvedConfig::load(&cfg.config_path).unwrap();
    assert!(engine::status(&cfg).unwrap().mode_change_required);
    assert!(
        engine::update(&cfg, &model, None, UpdateOptions::default())
            .await
            .is_err()
    );
    assert!(
        engine::update(
            &cfg,
            &model,
            None,
            UpdateOptions {
                migrate_output: true,
                ..UpdateOptions::default()
            }
        )
        .await
        .is_err(),
        "switching to Markdown requires explicit rebuild"
    );
    engine::update(
        &cfg,
        &model,
        None,
        UpdateOptions {
            migrate_output: true,
            rebuild: true,
            ..UpdateOptions::default()
        },
    )
    .await
    .unwrap();
    assert!(cfg.wiki.join("index.md").exists());
    assert_eq!(
        publish::stored_mode(&storage::read_only(&cfg.state.join("state.db")).unwrap()).unwrap(),
        OutputMode::Markdown
    );
    assert!(publish::restore(&cfg).is_ok());
}

#[tokio::test]
async fn interrupted_and_source_mutated_headless_updates_cannot_commit_partial_registry() {
    let (_dir, cfg, model) = headless_project();
    put(
        &cfg,
        "rule.md",
        "DECISION queue: Preserve the original bound.\n",
    );
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let db = cfg.state.join("state.db");
    let before = fs::read(&db).unwrap();
    put(
        &cfg,
        "rule.md",
        "DECISION queue: A new bound is proposed.\n",
    );

    let interrupted =
        engine::update_with_progress(&cfg, &model, None, UpdateOptions::default(), |event| {
            if event == UpdateProgress::Phase("Validating retained knowledge") {
                anyhow::bail!("simulated cancellation before commit");
            }
            Ok(())
        })
        .await;
    assert!(interrupted.is_err());
    assert_eq!(fs::read(&db).unwrap(), before);
    assert!(!publish::has_pending(&cfg));
    assert!(!cfg.wiki.exists());

    let mutated =
        engine::update_with_progress(&cfg, &model, None, UpdateOptions::default(), |event| {
            if event == UpdateProgress::Phase("Validating retained knowledge") {
                put(
                    &cfg,
                    "rule.md",
                    "DECISION queue: Mutated during compilation.\n",
                );
            }
            Ok(())
        })
        .await;
    assert!(mutated.is_err());
    assert_eq!(fs::read(&db).unwrap(), before);
    assert!(!publish::has_pending(&cfg));
    assert!(!cfg.wiki.exists());
    let remaining = context::build_context(
        &storage::read_only(&db).unwrap(),
        &context::ContextOptions {
            task: "Preserve the original bound".into(),
            paths: vec![],
            max_tokens: 6000,
        },
    )
    .unwrap();
    assert!(
        serde_json::to_string(&remaining)
            .unwrap()
            .contains("original bound")
    );
}

#[tokio::test]
async fn headless_review_disposition_commits_registry_without_publication() {
    let (_dir, cfg, model) = headless_project();
    put(
        &cfg,
        "queue.md",
        "DECISION queue: Preserve the accepted queue order.\n",
    );
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let db = cfg.state.join("state.db");
    let initial = storage::read_only(&db).unwrap();
    let evidence = storage::views(&initial).unwrap()[0].evidence[0].id.clone();
    drop(initial);
    {
        // Exercise the same pending-review lifecycle in a project with no
        // generated overview/review pages.
        let conn = rusqlite::Connection::open(&db).unwrap();
        storage::review(
            &conn,
            &cfg.project_id,
            "manual:headless-queue-order",
            "The accepted queue constraint needs a scoped review",
        )
        .unwrap();
    }
    let conn = storage::read_only(&db).unwrap();
    let id = reviews::list(&conn, false).unwrap()[0].id.clone();
    drop(conn);
    let calls = model.calls.load(Ordering::SeqCst);
    let result = engine::change_review(
        &cfg,
        &id,
        "resolved",
        "The documented policy remains unchanged; production still needs testing.",
        "fixture-reviewer",
    )
    .unwrap();
    assert_eq!(result["changed"], true);
    assert_eq!(calls, model.calls.load(Ordering::SeqCst));
    assert!(!cfg.wiki.exists());
    let conn = storage::read_only(&db).unwrap();
    assert!(storage::evidence_snapshot(&conn, &evidence).is_ok());
    assert_eq!(
        publish::stored_mode(&conn).unwrap(),
        OutputMode::RegistryOnly
    );
    assert!(storage::pages(&conn).unwrap().is_empty());
    assert!(reviews::events(&conn, &id).unwrap().len() >= 2);
    assert!(!engine::status(&cfg).unwrap().needs_update);
}
