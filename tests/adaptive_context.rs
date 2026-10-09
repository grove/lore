mod common;

use lore::{
    context::{
        self, ContextOptions,
        adaptive::{self, HostGrants},
        decision::runtime::{DecisionContextResult, RunOptions},
    },
    engine::{self, UpdateOptions},
    storage,
};
use std::fs;

#[tokio::test]
async fn exact_retained_constant_bypasses_model_checkout_and_cache() {
    let (_dir, config, model) = common::project();
    common::put(
        &config,
        "queue.md",
        "# Queue\nDECISION queue: QUEUE_CAPACITY is 128 entries.\n",
    );
    engine::update(&config, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let calls = model.calls.load(std::sync::atomic::Ordering::SeqCst);
    let conn = storage::read_only(&config.state.join("state.db")).unwrap();
    let options = ContextOptions {
        task: "What is QUEUE_CAPACITY?".into(),
        paths: vec![],
        max_tokens: 6_000,
    };
    let selected = context::build_context(&conn, &options).unwrap();
    let result = adaptive::build(
        &conn,
        &config,
        &options,
        selected,
        Some(&model),
        &RunOptions {
            inspect: true,
            ..RunOptions::default()
        },
    )
    .await
    .unwrap();
    let DecisionContextResult::FastFallback(reference) = result.intelligence else {
        panic!("exact reference should bypass decision generation")
    };
    assert_eq!(reference.mode, "reference");
    assert_eq!(reference.context.model_calls, 0);
    assert_eq!(reference.inspection_status, "not_needed");
    assert!(
        reference
            .context
            .sections
            .items()
            .any(|item| item.statement.contains("128"))
    );
    assert_eq!(model.calls.load(std::sync::atomic::Ordering::SeqCst), calls);
    assert!(!config.state.join("context-cache").exists());
}

#[test]
fn project_configuration_cannot_grant_host_capabilities() {
    let (_dir, mut config, _model) = common::project();
    config.config.context.inspection.enabled = true;
    config.config.privacy.local_only = false;
    config.config.privacy.allow_checkout_egress = true;
    let (effective, run, caps) =
        adaptive::authorize(&config, &RunOptions::default(), &HostGrants::default()).unwrap();
    assert!(!effective.config.context.inspection.enabled);
    assert!(!run.investigate);
    assert!(effective.config.privacy.local_only);
    assert!(!caps.hosted_egress && !caps.checkout_egress);
    assert!(!caps.execution && !caps.source_write);
    assert_eq!(caps.inspection, "not_granted");
}

#[test]
fn standing_read_grant_enables_automatic_investigation_with_narrower_project_scope() {
    let (_dir, mut config, _model) = common::project();
    config.config.context.inspection.root = Some("docs".into());
    let grants = HostGrants {
        inspection_root: Some(config.base.clone()),
        ..HostGrants::default()
    };
    let (effective, run, caps) =
        adaptive::authorize(&config, &RunOptions::default(), &grants).unwrap();
    assert!(run.inspect && run.investigate && !run.no_inspect);
    assert_eq!(
        effective.config.context.inspection.root,
        Some("docs".into())
    );
    assert_eq!(caps.inspection, "granted");
    assert!(!caps.checkout_egress);
}

#[test]
fn project_root_cannot_escape_standing_grant_and_caller_denial_wins() {
    let (_dir, mut config, _model) = common::project();
    let grants = HostGrants {
        inspection_root: Some(config.base.join("docs")),
        ..HostGrants::default()
    };
    config.config.context.inspection.root = Some(config.base.clone());
    let (_, run, caps) = adaptive::authorize(&config, &RunOptions::default(), &grants).unwrap();
    assert!(!run.inspect && !run.investigate);
    assert_eq!(caps.inspection, "outside_grant");
    let (_, run, caps) = adaptive::authorize(
        &config,
        &RunOptions {
            no_inspect: true,
            ..RunOptions::default()
        },
        &grants,
    )
    .unwrap();
    assert!(!run.inspect && !run.investigate);
    assert_eq!(caps.inspection, "disabled_by_caller");
}

#[test]
fn local_only_remains_a_ceiling_on_explicit_hosted_grants() {
    let (_dir, config, _model) = common::project();
    let grants = HostGrants {
        hosted_egress: true,
        checkout_egress: true,
        ..HostGrants::default()
    };
    let (effective, run, caps) = adaptive::authorize(
        &config,
        &RunOptions {
            allow_checkout_egress: true,
            ..RunOptions::default()
        },
        &grants,
    )
    .unwrap();
    assert!(effective.config.privacy.local_only);
    assert!(!run.allow_checkout_egress);
    assert!(!caps.hosted_egress && !caps.checkout_egress);
}

#[tokio::test]
async fn new_evidence_changes_registry_identity_even_when_original_dependencies_do_not() {
    let (_dir, config, model) = common::project();
    common::put(
        &config,
        "ADR-001.md",
        "# Queue policy\nDECISION dispatch: Preserve per-key ordering.\n",
    );
    engine::update(&config, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let first = storage::read_only(&config.state.join("state.db")).unwrap();
    let before = storage::registry_revision(&first).unwrap();
    let old = storage::source_heads(&first).unwrap();
    drop(first);
    common::put(
        &config,
        "ADR-002.md",
        "# Queue exception\nDECISION dispatch: Ordering does not apply to independent partitions.\n",
    );
    engine::update(&config, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let second = storage::read_only(&config.state.join("state.db")).unwrap();
    assert_ne!(before, storage::registry_revision(&second).unwrap());
    let current = storage::source_heads(&second).unwrap();
    let unchanged = current.iter().find(|head| head.id == old[0].id).unwrap();
    assert_eq!(unchanged.digest, old[0].digest);
    assert_eq!(unchanged.revision, old[0].revision);
    assert_eq!(
        storage::registry_revision(&second).unwrap(),
        storage::registry_revision(&second).unwrap()
    );
}

#[tokio::test]
async fn shared_static_result_has_exact_evidence_and_makes_no_source_or_registry_writes() {
    let (_dir, mut config, model) = common::project();
    common::put(
        &config,
        "dispatch.md",
        "# Dispatch\nDECISION dispatch: Preserve per-key ordering in src/queue.rs.\n",
    );
    fs::create_dir(config.base.join("src")).unwrap();
    fs::write(
        config.base.join("src/queue.rs"),
        "pub const QUEUE_CAPACITY: usize = 128;\n",
    )
    .unwrap();
    engine::update(&config, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    config.config.context.inspection.root = Some(config.base.clone());
    config.config.context.inspection.max_elapsed_ms = 5_000;
    let db = config.state.join("state.db");
    let before = fs::read(&db).unwrap();
    let source_before = fs::read(config.base.join("src/queue.rs")).unwrap();
    let conn = storage::read_only(&db).unwrap();
    let options = ContextOptions {
        task: "Refactor dispatch queue allocation".into(),
        paths: vec!["src/queue.rs".into()],
        max_tokens: 6_000,
    };
    let selected = context::build_context(&conn, &options).unwrap();
    let result = adaptive::build(
        &conn,
        &config,
        &options,
        selected,
        None,
        &RunOptions {
            inspect: true,
            no_cache: true,
            ..RunOptions::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(result.schema_version, 5);
    assert_eq!(
        result.snapshot.registry_revision,
        storage::registry_revision(&conn).unwrap()
    );
    assert!(!result.capabilities.execution);
    let DecisionContextResult::Brief(brief) = &result.intelligence else {
        panic!("static decision should fit")
    };
    assert_eq!(brief.schema_version, 4);
    assert_eq!(brief.model_calls, 0);
    assert!(!brief.inspection.observations.is_empty());
    assert!(
        brief
            .inspection
            .observations
            .iter()
            .all(|observation| observation.kind == "static_source")
    );
    for evidence in &brief.evidence {
        assert!(storage::evidence_snapshot(&conn, &evidence.id).is_ok());
    }
    assert_eq!(fs::read(&db).unwrap(), before);
    assert_eq!(
        fs::read(config.base.join("src/queue.rs")).unwrap(),
        source_before
    );
    assert!(!config.state.join("context-cache").exists());
    assert!(context::count_tokens(&serde_json::to_string(&result).unwrap()) <= options.max_tokens);
    assert!(context::count_tokens(&adaptive::render(&result)) <= options.max_tokens);
}
