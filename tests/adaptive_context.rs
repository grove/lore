mod common;

use lore::{
    context::{
        self, ContextOptions,
        adaptive::{self, HostGrants},
        decision::runtime::{DecisionContextResult, RunOptions},
    },
    engine::{self, UpdateOptions},
    inference::{
        ExecutionLocation, GenerationRequest, GenerationResponse, GenerativeModel, ModelDescriptor,
        ModelFuture, Provider,
    },
    storage,
};
use std::fs;
use std::{
    future::{Future, poll_fn},
    sync::atomic::{AtomicUsize, Ordering},
    task::Poll,
};

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
    let actual = context::count_tokens(&(serde_json::to_string(&result).unwrap() + "\n"))
        .max(context::count_tokens(&adaptive::render(&result)));
    assert!(actual <= result.budget.used_tokens);
    assert!(result.budget.used_tokens <= options.max_tokens);
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
    let actual = context::count_tokens(&(serde_json::to_string(&result).unwrap() + "\n"))
        .max(context::count_tokens(&adaptive::render(&result)));
    assert!(actual <= result.budget.used_tokens);
    assert!(result.budget.used_tokens <= options.max_tokens);
}

#[tokio::test]
async fn adaptive_preselection_must_match_current_task_paths_and_retained_facts() {
    let (_dir, config, model) = common::project();
    common::put(
        &config,
        "queue.md",
        "DECISION queue: QUEUE_CAPACITY is 128 entries.\n",
    );
    engine::update(&config, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let conn = storage::read_only(&config.state.join("state.db")).unwrap();
    let options = ContextOptions {
        task: "What is QUEUE_CAPACITY?".into(),
        paths: vec![],
        max_tokens: 6_000,
    };
    let selected = context::build_context(&conn, &options).unwrap();
    let calls = model.calls.load(Ordering::SeqCst);
    let run = RunOptions {
        no_inspect: true,
        no_cache: true,
        ..RunOptions::default()
    };
    let mut tampered = selected.clone();
    tampered.evidence[0].excerpt = "QUEUE_CAPACITY is 999 entries.".into();
    let mut other_task = options.clone();
    other_task.task = "Why does the queue need a bounded capacity?".into();
    let mut other_path = options.clone();
    other_path.paths = vec!["unrelated/billing.rs".into()];
    for (options, selection) in [
        (&options, tampered),
        (&other_task, selected.clone()),
        (&other_path, selected),
    ] {
        let error = adaptive::build(&conn, &config, options, selection, Some(&model), &run)
            .await
            .unwrap_err();
        assert_eq!(
            error.downcast_ref::<context::ContextError>().unwrap().code,
            "invalid_selection"
        );
        assert!(conn.is_autocommit());
    }
    assert_eq!(model.calls.load(Ordering::SeqCst), calls);
    assert!(!config.state.join("decision-cache").exists());
}

#[tokio::test]
async fn new_counterevidence_invalidates_preselection_even_when_old_citations_still_exist() {
    let (_dir, config, model) = common::project();
    common::put(
        &config,
        "queue.md",
        "DECISION queue: QUEUE_CAPACITY is 128 entries.\n",
    );
    engine::update(&config, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let path = config.state.join("state.db");
    let options = ContextOptions {
        task: "What is QUEUE_CAPACITY?".into(),
        paths: vec![],
        max_tokens: 6_000,
    };
    let conn = storage::read_only(&path).unwrap();
    let selected = context::build_context(&conn, &options).unwrap();
    drop(conn);
    common::put(
        &config,
        "exception.md",
        "DECISION queue: QUEUE_CAPACITY must be 32 entries for memory-constrained workers.\n",
    );
    engine::update(&config, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let conn = storage::read_only(&path).unwrap();
    for evidence in &selected.evidence {
        let retained = storage::evidence_snapshot(&conn, &evidence.id).unwrap();
        assert_eq!(retained.excerpt, evidence.excerpt);
    }
    let error = adaptive::build(
        &conn,
        &config,
        &options,
        selected,
        None,
        &RunOptions {
            no_inspect: true,
            no_cache: true,
            ..RunOptions::default()
        },
    )
    .await
    .unwrap_err();
    assert_eq!(
        error.downcast_ref::<context::ContextError>().unwrap().code,
        "invalid_selection"
    );
    assert!(conn.is_autocommit());
}

struct PendingDecisionModel {
    descriptor: ModelDescriptor,
    calls: AtomicUsize,
}

impl GenerativeModel for PendingDecisionModel {
    fn descriptor(&self) -> &ModelDescriptor {
        &self.descriptor
    }

    fn generate<'a>(&'a self, _: &'a GenerationRequest) -> ModelFuture<'a, GenerationResponse> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Box::pin(std::future::pending())
    }
}

#[tokio::test]
async fn dropping_pending_adaptive_generation_releases_only_its_read_snapshot() {
    let (_dir, mut config, compiler) = common::project();
    common::put(
        &config,
        "workers.md",
        "DECISION workers: Worker queue events use one durable queue.\n",
    );
    engine::update(&config, &compiler, None, UpdateOptions::default())
        .await
        .unwrap();
    config.config.models.generative.model = "pending-fixture".into();
    config.config.context.cache = false;
    let path = config.state.join("state.db");
    let before = fs::read(&path).unwrap();
    let conn = storage::read_only(&path).unwrap();
    let options = ContextOptions {
        task: "Refactor worker queue scheduling".into(),
        paths: vec![],
        max_tokens: 6_000,
    };
    let selected = context::build_context(&conn, &options).unwrap();
    let model = PendingDecisionModel {
        descriptor: ModelDescriptor {
            provider: Provider::Ollama,
            model: "pending-fixture".into(),
            location: ExecutionLocation::Local,
        },
        calls: AtomicUsize::new(0),
    };
    let run = RunOptions {
        no_inspect: true,
        no_cache: true,
        ..RunOptions::default()
    };
    for caller_transaction in [false, true] {
        if caller_transaction {
            conn.execute_batch("BEGIN").unwrap();
        }
        let calls_before = model.calls.load(Ordering::SeqCst);
        let mut pending = Box::pin(adaptive::build(
            &conn,
            &config,
            &options,
            selected.clone(),
            Some(&model),
            &run,
        ));
        poll_fn(|cx| {
            assert!(
                pending.as_mut().poll(cx).is_pending(),
                "model should remain pending until the caller cancels"
            );
            Poll::Ready(())
        })
        .await;
        assert_eq!(model.calls.load(Ordering::SeqCst), calls_before + 1);
        assert!(
            !conn.is_autocommit(),
            "shared source snapshot must remain stable during generation"
        );
        drop(pending);
        assert_eq!(conn.is_autocommit(), !caller_transaction);
        tokio::task::yield_now().await;
        assert_eq!(
            model.calls.load(Ordering::SeqCst),
            calls_before + 1,
            "cancellation must not schedule follow-up generation"
        );
        if caller_transaction {
            conn.execute_batch("ROLLBACK").unwrap();
        }
    }
    assert_eq!(fs::read(path).unwrap(), before);
    assert!(!config.state.join("decision-cache").exists());
}

#[tokio::test]
async fn failed_adaptive_request_releases_its_read_snapshot() {
    let (_dir, config, compiler) = common::project();
    common::put(
        &config,
        "workers.md",
        "DECISION workers: Worker queue events use one durable queue.\n",
    );
    engine::update(&config, &compiler, None, UpdateOptions::default())
        .await
        .unwrap();
    let conn = storage::read_only(&config.state.join("state.db")).unwrap();
    let result = adaptive::run(
        &config,
        &conn,
        &ContextOptions {
            task: "Refactor worker queue scheduling".into(),
            paths: vec![],
            max_tokens: 256,
        },
        &RunOptions::default(),
    )
    .await;
    assert!(
        result.is_err(),
        "the complete shared envelope cannot fit this budget"
    );
    assert!(conn.is_autocommit());
}
