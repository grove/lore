mod common;

use lore::{
    companion::{self, Baseline, ChangeOptions},
    context::{self, decision::runtime::RunOptions},
    domain::{AssertionProposal, ImportKind},
    engine::{self, UpdateOptions},
    imports::{
        self, Inventory, SourceBatch,
        adapters::{
            AdapterRecord, ImportBatch, NativeEvidence, NativeRelationship, ObservationKind,
            ObservationScope,
        },
        relationships::{CrossSourceEndpoint, CrossSourceRelation},
    },
    reviews,
    sources::{self, Document},
    storage, util,
};
use std::{fs, sync::atomic::Ordering};

fn options() -> ChangeOptions {
    ChangeOptions {
        since: "joined".into(),
        task: None,
        max_tokens: 8_000,
    }
}

fn rewrite(config: &lore::config::ResolvedConfig, mut baseline: Baseline) {
    baseline.content_hash = util::json_digest(&(
        baseline.schema_version,
        &baseline.name,
        &baseline.captured_at,
        &baseline.snapshot,
        &baseline.source_scope,
        &baseline.records,
        &baseline.relationships,
        &baseline.imported,
        &baseline.cross_source_relationships,
    ))
    .unwrap();
    fs::write(
        config.state.join("baselines/joined.json"),
        serde_json::to_vec(&baseline).unwrap(),
    )
    .unwrap();
}

#[tokio::test]
async fn explicit_baseline_and_cosmetic_source_change_do_not_create_consequential_alerts() {
    let (_dir, config, model) = common::project();
    common::put(
        &config,
        "queue.md",
        "# Queue\nDECISION queue: Queue admission preserves arrival order except emergency drains.\n",
    );
    engine::update(&config, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let db = fs::read(config.state.join("state.db")).unwrap();
    let conn = storage::read_only(&config.state.join("state.db")).unwrap();
    let saved = companion::save_baseline(&config, &conn, "joined", false).unwrap();
    assert!(conn.is_autocommit());
    let same = companion::changes(&config, &conn, &options()).unwrap();
    assert!(!same.registry_changed && same.changes.is_empty());
    assert_eq!(
        saved.snapshot.registry_revision,
        same.snapshot.registry_revision
    );
    assert_eq!(db, fs::read(config.state.join("state.db")).unwrap());
    drop(conn);
    common::put(
        &config,
        "queue.md",
        "# Queue\nDECISION queue: Queue admission preserves arrival order except emergency drains.\n\nEditorial note.\n",
    );
    engine::update(&config, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let conn = storage::read_only(&config.state.join("state.db")).unwrap();
    let changed = companion::changes(&config, &conn, &options()).unwrap();
    assert!(changed.registry_changed);
    assert!(changed.changes.is_empty(), "{:?}", changed.changes);
    let calls = model.calls.load(Ordering::SeqCst);
    let advisory = companion::guard(&config, &conn, &options(), &RunOptions::default())
        .await
        .unwrap();
    assert_eq!(advisory.assessment_status, "no_documented_change");
    assert!(advisory.advisories.is_empty() && advisory.intelligence.is_none());
    assert!(!advisory.execution && !advisory.source_write);
    assert_eq!(calls, model.calls.load(Ordering::SeqCst));
}

#[tokio::test]
async fn supersession_report_keeps_original_policy_status_scope_and_relation_evidence() {
    let (_dir, config, model) = common::project();
    common::put(
        &config,
        "ADR-001.md",
        "DECISION database: MySQL is the selected database.\n",
    );
    engine::update(&config, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    {
        let conn = storage::read_only(&config.state.join("state.db")).unwrap();
        companion::save_baseline(&config, &conn, "joined", false).unwrap();
    }
    common::put(
        &config,
        "ADR-002.md",
        "DECISION database: PostgreSQL replaces ADR-001; preserve MySQL exports for legacy clients.\n",
    );
    engine::update(&config, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let conn = storage::read_only(&config.state.join("state.db")).unwrap();
    let report = companion::changes(&config, &conn, &options()).unwrap();
    assert!(report.changes.iter().any(|c| {
        c.change_kind == "lifecycle_changed"
            && c.before.as_ref().is_some_and(|r| r.lifecycle == "accepted")
            && c.after
                .as_ref()
                .is_some_and(|r| r.lifecycle == "superseded")
    }));
    assert!(report.changes.iter().any(|c| {
        c.after
            .as_ref()
            .is_some_and(|r| r.statement.contains("preserve MySQL exports"))
    }));
    assert_eq!(report.relationship_changes.len(), 1);
    let relation = report.relationship_changes[0].after.as_ref().unwrap();
    assert!(relation.active && relation.kind == "supersedes");
    for id in [&relation.from, &relation.to] {
        assert!(report.related_knowledge.iter().any(|r| &r.id == id));
    }
    assert!(report.evidence.iter().any(|e| e.id == relation.evidence_id));
    for evidence in &report.evidence {
        let original = storage::evidence_snapshot(&conn, &evidence.id).unwrap();
        assert_eq!(evidence.excerpt, original.excerpt);
        assert_eq!(evidence.digest, original.digest);
    }
    assert!(
        report
            .changes
            .iter()
            .all(|c| c.implication_basis == "inferred_from_documented_change")
    );
    assert!(!report.live_checkout_assessed);
}

#[tokio::test]
async fn edited_baseline_cannot_fabricate_accepted_history_scope_or_evidence() {
    let (_dir, config, model) = common::project();
    common::put(
        &config,
        "queue.md",
        "DECISION queue: Queue capacity is 128.\nDECISION identity: Identity tokens expire after 30 minutes.\n",
    );
    engine::update(&config, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let conn = storage::read_only(&config.state.join("state.db")).unwrap();
    let original = companion::save_baseline(&config, &conn, "joined", false).unwrap();
    for field in ["statement", "scope", "evidence", "missing_evidence"] {
        let mut edited = original.clone();
        let different = edited.records.values().last().unwrap().evidence_ids[0].clone();
        let first = edited.records.values_mut().next().unwrap();
        match field {
            "statement" => first.statement = "Invented accepted policy".into(),
            "scope" => first.scope = "every deployment".into(),
            "missing_evidence" => first.evidence_ids.clear(),
            _ => first.evidence_ids = vec![different],
        }
        rewrite(&config, edited);
        assert!(
            companion::changes(&config, &conn, &options()).is_err(),
            "{field}"
        );
        assert!(conn.is_autocommit());
    }
    rewrite(&config, original);
    let mut narrowed = config.clone();
    narrowed.config.sources.roots[0].path = "different".into();
    assert!(companion::changes(&narrowed, &conn, &options()).is_err());
}

#[tokio::test]
async fn baselines_require_explicit_replacement_and_have_bounded_local_retention() {
    let (_dir, config, model) = common::project();
    common::put(
        &config,
        "rule.md",
        "DECISION queue: Queue admission is bounded.\n",
    );
    engine::update(&config, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let conn = storage::read_only(&config.state.join("state.db")).unwrap();
    companion::save_baseline(&config, &conn, "joined", false).unwrap();
    assert!(companion::save_baseline(&config, &conn, "joined", false).is_err());
    companion::save_baseline(&config, &conn, "joined", true).unwrap();
    for invalid in ["../escape", "../", "", "notes/file"] {
        assert!(companion::save_baseline(&config, &conn, invalid, false).is_err());
    }
    for number in 1..16 {
        companion::save_baseline(&config, &conn, &format!("point-{number}"), false).unwrap();
    }
    assert!(companion::save_baseline(&config, &conn, "overflow", false).is_err());
    companion::remove_baseline(&config, "joined").unwrap();
    assert!(companion::changes(&config, &conn, &options()).is_err());
    assert!(config.base.join("docs/rule.md").is_file());
    companion::save_baseline(&config, &conn, "joined", false).unwrap();
}

#[tokio::test]
async fn constrained_reports_omit_complete_records_and_account_for_all_output() {
    let (_dir, config, model) = common::project();
    common::put(
        &config,
        "rule.md",
        "DECISION queue: Queue admission is bounded.\n",
    );
    engine::update(&config, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    {
        let conn = storage::read_only(&config.state.join("state.db")).unwrap();
        companion::save_baseline(&config, &conn, "joined", false).unwrap();
    }
    common::put(
        &config,
        "exception.md",
        &format!(
            "DECISION queue: Queue admission excludes {}.\n",
            "rare exception 日本語 ".repeat(100)
        ),
    );
    engine::update(&config, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let conn = storage::read_only(&config.state.join("state.db")).unwrap();
    let result = companion::changes(
        &config,
        &conn,
        &ChangeOptions {
            max_tokens: 700,
            ..options()
        },
    )
    .unwrap();
    assert!(result.omitted_changes > 0);
    let json = serde_json::to_string(&result).unwrap() + "\n";
    let markdown = companion::render_changes(&result);
    assert!(context::count_tokens(&json) <= 700);
    assert!(context::count_tokens(&markdown) <= 700);
    assert!(
        result.budget.used_tokens
            >= context::count_tokens(&json).max(context::count_tokens(&markdown))
    );
    assert!(result.budget.used_tokens <= 700);
}

#[tokio::test]
async fn guardian_preserves_useful_shared_fallback_and_never_claims_clean_checkout() {
    let (_dir, mut config, model) = common::project();
    common::put(
        &config,
        "queue.md",
        "DECISION queue: Queue admission preserves arrival order except emergency drains.\n",
    );
    engine::update(&config, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    config.config.models.generative.enabled = false;
    let conn = storage::read_only(&config.state.join("state.db")).unwrap();
    companion::save_baseline(&config, &conn, "joined", false).unwrap();
    let before = fs::read(config.state.join("state.db")).unwrap();
    let query = ChangeOptions {
        task: Some("Change queue admission diagnostics without changing order".into()),
        ..options()
    };
    let result = companion::guard(
        &config,
        &conn,
        &query,
        &RunOptions {
            no_inspect: true,
            no_cache: true,
            ..RunOptions::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(result.assessment_status, "partial_static_guidance");
    assert!(!result.execution && !result.source_write && result.advisories.is_empty());
    let shared = result.intelligence.as_ref().unwrap();
    assert_eq!(
        result.snapshot.registry_revision,
        shared.snapshot.registry_revision
    );
    assert_eq!(shared.capabilities.inspection, "disabled_by_caller");
    assert!(
        serde_json::to_string(shared)
            .unwrap()
            .contains("emergency drains")
    );
    assert!(
        result
            .warnings
            .iter()
            .any(|s| s.contains("not a clean bill"))
    );
    assert_eq!(before, fs::read(config.state.join("state.db")).unwrap());
    assert!(conn.is_autocommit());
    let json = serde_json::to_string(&result).unwrap() + "\n";
    let markdown = companion::render_guard(&result);
    assert!(
        result.budget.used_tokens
            >= context::count_tokens(&json).max(context::count_tokens(&markdown))
    );
    assert!(result.budget.used_tokens <= query.max_tokens);
}

#[cfg(unix)]
#[tokio::test]
async fn interrupted_or_symlinked_baselines_never_touch_a_source() {
    let (_dir, config, model) = common::project();
    common::put(
        &config,
        "queue.md",
        "DECISION queue: Queue admission is bounded.\n",
    );
    engine::update(&config, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let conn = storage::read_only(&config.state.join("state.db")).unwrap();
    companion::save_baseline(&config, &conn, "joined", false).unwrap();
    let path = config.state.join("baselines/joined.json");
    fs::write(&path, "{partial").unwrap();
    assert!(companion::changes(&config, &conn, &options()).is_err());
    fs::remove_file(&path).unwrap();
    let source = config.base.join("docs/queue.md");
    let original = fs::read(&source).unwrap();
    std::os::unix::fs::symlink(&source, &path).unwrap();
    assert!(companion::changes(&config, &conn, &options()).is_err());
    assert!(companion::save_baseline(&config, &conn, "joined", true).is_err());
    assert!(companion::remove_baseline(&config, "joined").is_err());
    assert_eq!(original, fs::read(&source).unwrap());
}

fn captured_support(
    config: &lore::config::ResolvedConfig,
    conn: &rusqlite::Connection,
    statement: &str,
    quote: &str,
    existing: Option<&str>,
) -> (String, String) {
    let (root, path) = config.roots.first().unwrap();
    let text = format!("# Queue rule\n{quote}\n");
    let document = Document {
        root_id: root.clone(),
        root_path: path.clone(),
        material: Default::default(),
        origin: None,
        relative_path: "captured.md".into(),
        physical_path: path.join("captured.md"),
        digest: util::digest(&text),
        chunks: sources::split_markdown(&text, root, "captured.md", 8_000).unwrap(),
        text,
    };
    let proposal = AssertionProposal {
        topic: "queue".into(),
        topic_title: "Queue".into(),
        subject: "queue ordering".into(),
        statement: statement.into(),
        quote: quote.into(),
        kind: "decision".into(),
        lifecycle: "accepted".into(),
        scope: "production".into(),
        effective_at: String::new(),
    };
    let heads = storage::source_heads(conn).unwrap();
    let previous = heads
        .iter()
        .find(|head| head.root == *root && head.path == "captured.md");
    let (source, revision) = storage::begin_source(conn, &document, previous).unwrap();
    let chunk = document
        .chunks
        .iter()
        .find(|chunk| chunk.text.contains(quote))
        .unwrap();
    let (section, section_revision) =
        storage::begin_section(conn, &source, &revision, chunk).unwrap();
    let (assertion, evidence) = storage::capture_assertion(
        conn,
        storage::AssertionCapture {
            document: &document,
            chunk,
            proposal: &proposal,
            source: &source,
            source_revision: &revision,
            section: &section,
            section_revision: &section_revision,
            model: "fixture-explicit-same-statement",
        },
    )
    .unwrap();
    let id = if let Some(id) = existing {
        storage::assign(conn, &assertion, id).unwrap();
        id.to_owned()
    } else {
        storage::create_unit(conn, &config.project_id, &assertion, &proposal).unwrap()
    };
    storage::refresh_knowledge(conn, &config.project_id).unwrap();
    (id, evidence)
}

#[tokio::test]
async fn changed_exact_exception_is_visible_with_unchanged_summary_and_old_baseline_stays_valid() {
    let (_dir, config, model) = common::project();
    common::put(
        &config,
        "initial.md",
        "DECISION identity: Identity checks remain required.\n",
    );
    engine::update(&config, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let conn = rusqlite::Connection::open(config.state.join("state.db")).unwrap();
    let statement = "Queue admission preserves arrival order.";
    let original_quote = "Queue admission preserves arrival order except emergency drains.";
    let (id, original_evidence) = captured_support(&config, &conn, statement, original_quote, None);
    let original_baseline = companion::save_baseline(&config, &conn, "joined", false).unwrap();
    assert_eq!(
        original_baseline.records[&id].evidence_ids,
        vec![original_evidence]
    );
    let changed_quote = "Queue admission preserves arrival order except emergency drains only after every tenant key is acknowledged.";
    let (_, new_evidence) = captured_support(&config, &conn, statement, changed_quote, Some(&id));
    let report = companion::changes(&config, &conn, &options()).unwrap();
    let change = report
        .changes
        .iter()
        .find(|change| change.knowledge_id == id)
        .unwrap();
    assert_eq!(change.before.as_ref().unwrap().statement, statement);
    assert_eq!(change.after.as_ref().unwrap().statement, statement);
    assert_eq!(change.change_kind, "understanding_changed");
    assert!(
        report
            .evidence
            .iter()
            .any(|e| e.id == new_evidence && e.excerpt == changed_quote)
    );
    let markdown = companion::render_changes(&report);
    assert!(markdown.contains("every tenant key is acknowledged"));
    assert!(markdown.contains("documentary evidence, not independent verification"));
    assert!(markdown.contains("Scope: production"));
    let mut forged = original_baseline;
    forged.records.get_mut(&id).unwrap().evidence_ids = vec![new_evidence];
    rewrite(&config, forged);
    assert!(
        companion::changes(&config, &conn, &options()).is_err(),
        "later valid support for the same record cannot be substituted into an older sealed revision"
    );
}

// GUARD-REVERSION-001: cumulative historical support is identical on B -> A,
// while the source-current exception changes. This is a three-checkpoint
// regression, not a model effectiveness experiment.
#[tokio::test]
async fn restored_source_condition_is_visible_after_a_b_a_with_unchanged_summary() {
    let (_dir, config, model) = common::project();
    common::put(
        &config,
        "initial.md",
        "DECISION identity: Identity checks remain required.\n",
    );
    engine::update(&config, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let conn = rusqlite::Connection::open(config.state.join("state.db")).unwrap();
    let summary = "Queue admission preserves arrival order.";
    let original = "Queue admission preserves arrival order except emergency drains.";
    let changed = "Queue admission preserves arrival order except emergency drains after tenant acknowledgement.";
    let (id, _) = captured_support(&config, &conn, summary, original, None);
    for quote in [changed, original] {
        companion::save_baseline(&config, &conn, "joined", true).unwrap();
        let head = storage::source_heads(&conn)
            .unwrap()
            .into_iter()
            .find(|head| head.path == "captured.md")
            .unwrap();
        for (_, (section, _)) in storage::current_chunks(&conn, &head.id).unwrap() {
            storage::retire_section(&conn, &section).unwrap();
        }
        let (_, evidence) = captured_support(&config, &conn, summary, quote, Some(&id));
        let report = companion::changes(&config, &conn, &options()).unwrap();
        let change = report.changes.iter().find(|change| change.knowledge_id == id)
            .expect("Restoring earlier source-current support must not disappear behind historical quotations");
        assert_eq!(change.before.as_ref().unwrap().statement, summary);
        assert_eq!(change.after.as_ref().unwrap().statement, summary);
        assert_eq!(
            change
                .after
                .as_ref()
                .unwrap()
                .current_evidence_ids
                .as_ref()
                .unwrap(),
            &vec![evidence]
        );
        assert!(
            report
                .evidence
                .iter()
                .any(|evidence| evidence.excerpt == quote)
        );
    }
    companion::save_baseline(&config, &conn, "joined", true).unwrap();
    assert!(
        companion::changes(&config, &conn, &options())
            .unwrap()
            .changes
            .is_empty()
    );
}

#[tokio::test]
async fn schema_one_baselines_without_current_membership_keep_original_checksum_and_load() {
    let (_dir, config, model) = common::project();
    common::put(
        &config,
        "queue.md",
        "DECISION queue: Queue capacity is 128.\n",
    );
    engine::update(&config, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let conn = storage::read_only(&config.state.join("state.db")).unwrap();
    let mut baseline = companion::save_baseline(&config, &conn, "joined", false).unwrap();
    for record in baseline.records.values_mut() {
        record.current_evidence_ids = None;
    }
    rewrite(&config, baseline);
    let raw = fs::read_to_string(config.state.join("baselines/joined.json")).unwrap();
    assert!(!raw.contains("current_evidence_ids"));
    let round_trip: Baseline = serde_json::from_str(&raw).unwrap();
    assert!(
        round_trip
            .records
            .values()
            .all(|state| state.current_evidence_ids.is_none())
    );
    let report = companion::changes(&config, &conn, &options()).unwrap();
    assert!(report.changes.is_empty());
}

#[tokio::test]
async fn captured_current_membership_cannot_name_another_revision_or_duplicate_support() {
    let (_dir, config, model) = common::project();
    common::put(
        &config,
        "queue.md",
        "DECISION queue: Queue capacity is 128.\n",
    );
    engine::update(&config, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let conn = storage::read_only(&config.state.join("state.db")).unwrap();
    let baseline = companion::save_baseline(&config, &conn, "joined", false).unwrap();
    for duplicate in [false, true] {
        let mut edited = baseline.clone();
        let record = edited.records.values_mut().next().unwrap();
        record.current_evidence_ids = Some(if duplicate {
            vec![
                record.evidence_ids[0].clone(),
                record.evidence_ids[0].clone(),
            ]
        } else {
            vec!["ev_unrelated_source".into()]
        });
        rewrite(&config, edited);
        assert!(companion::changes(&config, &conn, &options()).is_err());
    }
}

// GUARD-SCOPE-001: high-impact scope must survive the six-topic limit even
// when proposal names sort earlier alphabetically.
#[tokio::test]
async fn guardian_selects_accepted_constraints_before_alphabetical_proposal_noise() {
    let (_dir, mut config, model) = common::project();
    common::put(
        &config,
        "initial.md",
        "DECISION identity: Identity checks remain required.\n",
    );
    engine::update(&config, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    {
        let conn = storage::read_only(&config.state.join("state.db")).unwrap();
        companion::save_baseline(&config, &conn, "joined", false).unwrap();
    }
    for index in 0..6 {
        common::put(
            &config,
            &format!("proposal-{index}.md"),
            &format!(
                "PLAN alpha-{index}: A proposed demonstration widget {index} may change its heading.\n"
            ),
        );
    }
    common::put(
        &config,
        "security.md",
        "DECISION zulu-security: Production exports must redact all identity tokens before persistence.\n",
    );
    engine::update(&config, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    config.config.models.generative.enabled = false;
    let conn = storage::read_only(&config.state.join("state.db")).unwrap();
    let guarded = companion::guard(
        &config,
        &conn,
        &options(),
        &RunOptions {
            no_inspect: true,
            no_cache: true,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let shared = serde_json::to_value(guarded.intelligence.unwrap()).unwrap();
    let task = shared["intelligence"]["task"].as_str().unwrap();
    assert!(task.contains("zulu-security"), "{task}");
    assert!(
        guarded
            .warnings
            .iter()
            .any(|warning| warning.contains("omitted 1 other topic"))
    );
}

fn native_work(
    config: &lore::config::ResolvedConfig,
    conn: &rusqlite::Connection,
    statement: &str,
) {
    let mut record = AdapterRecord::new(
        "queue-42",
        statement,
        serde_json::json!({"id":"queue-42","status":"closed","statement":statement}),
    );
    record.kind = ObservationKind::WorkState;
    record.subject = "queue ordering".into();
    record.title = "Queue incident follow-up".into();
    record.lifecycle = "closed".into();
    record.scope = ObservationScope {
        environment: Some("staging".into()),
        ..Default::default()
    };
    record.evidence = vec![NativeEvidence {
        locator: "record://queue-42".into(),
        revision: Some("work-revision-3".into()),
        field: Some("/statement".into()),
    }];
    let batch = ImportBatch {
        format: "fixture-work-v1".into(),
        records: vec![record],
        warnings: vec![],
    };
    let source = SourceBatch {
        id: "work".into(),
        kind: ImportKind::Beads,
        path: config.base.join("work.jsonl"),
        digest: util::json_digest(&batch).unwrap(),
        batch,
    };
    imports::storage::persist(
        conn,
        &config.project_id,
        &Inventory {
            sources: vec![source],
            digest: "fixture".into(),
            warnings: vec![],
        },
    )
    .unwrap();
}

#[tokio::test]
async fn imported_work_change_keeps_native_scope_and_prevents_false_no_change_guardian() {
    let (_dir, mut config, model) = common::project();
    common::put(
        &config,
        "queue.md",
        "DECISION queue: Queue ordering must be preserved.\n",
    );
    engine::update(&config, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    config.config.models.generative.enabled = false;
    let conn = rusqlite::Connection::open(config.state.join("state.db")).unwrap();
    companion::save_baseline(&config, &conn, "joined", false).unwrap();
    let statement = "The queue ordering fix is closed after a staging-only replay; production has not been checked.";
    native_work(&config, &conn, statement);
    let report = companion::changes(&config, &conn, &options()).unwrap();
    assert!(report.registry_changed);
    assert!(report.changes.is_empty() && report.relationship_changes.is_empty());
    assert_eq!(report.imported_changes.len(), 1);
    assert_eq!(report.imported_evidence.len(), 1);
    assert_eq!(report.imported_evidence[0].record.lifecycle, "closed");
    assert_eq!(
        report.imported_evidence[0]
            .record
            .scope
            .environment
            .as_deref(),
        Some("staging")
    );
    assert_eq!(report.imported_evidence[0].record.statement, statement);
    assert!(companion::render_changes(&report).contains("production has not been checked"));
    let guarded = companion::guard(
        &config,
        &conn,
        &options(),
        &RunOptions {
            no_inspect: true,
            no_cache: true,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_ne!(guarded.assessment_status, "no_documented_change");
    assert!(guarded.intelligence.is_some());
    assert!(!guarded.execution && !guarded.source_write);
    assert!(
        guarded
            .warnings
            .iter()
            .any(|warning| warning.contains("not a clean bill"))
    );
}

#[tokio::test]
async fn omitted_native_conditions_produce_an_explicit_unassessed_guardian_status() {
    let (_dir, mut config, model) = common::project();
    common::put(
        &config,
        "queue.md",
        "DECISION queue: Queue ordering must be preserved.\n",
    );
    engine::update(&config, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    config.config.models.generative.enabled = false;
    let conn = rusqlite::Connection::open(config.state.join("state.db")).unwrap();
    companion::save_baseline(&config, &conn, "joined", false).unwrap();
    native_work(
        &config,
        &conn,
        &format!(
            "Queue recovery scope: {}",
            "only isolated staging; retain original tokens. ".repeat(100)
        ),
    );
    let limit = ChangeOptions {
        max_tokens: 1_024,
        ..options()
    };
    let report = companion::changes(&config, &conn, &limit).unwrap();
    assert!(report.imported_changes.is_empty());
    assert_eq!(report.omitted_imported_changes, 1);
    assert!(
        report.imported_evidence.is_empty(),
        "omission removes the complete native group"
    );
    let guarded = companion::guard(&config, &conn, &limit, &RunOptions::default())
        .await
        .unwrap();
    assert_eq!(guarded.assessment_status, "change_budget_exhausted");
    assert!(guarded.intelligence.is_none());
    assert!(!guarded.execution && !guarded.source_write);
    let used = context::count_tokens(&(serde_json::to_string(&guarded).unwrap() + "\n"))
        .max(context::count_tokens(&companion::render_guard(&guarded)));
    assert!(guarded.budget.used_tokens >= used && guarded.budget.used_tokens <= limit.max_tokens);
}

#[tokio::test]
async fn relationship_only_revisions_and_withdrawal_keep_original_caveats_and_current_guard() {
    let (_dir, mut config, model) = common::project();
    common::put(
        &config,
        "queue.md",
        "DECISION queue: Queue ordering must be preserved.\n",
    );
    engine::update(&config, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    config.config.models.generative.enabled = false;
    let conn = rusqlite::Connection::open(config.state.join("state.db")).unwrap();
    native_work(
        &config,
        &conn,
        "A staging-only queue replay is reported closed; production ordering has not been checked.",
    );
    let original = companion::save_baseline(&config, &conn, "joined", false).unwrap();
    assert!(original.cross_source_relationships.is_empty());
    let knowledge = storage::views(&conn).unwrap().remove(0);
    let observation = imports::views(&conn).unwrap().remove(0);
    let from = CrossSourceEndpoint {
        kind: "observation".into(),
        id: observation.id,
        revision_id: observation.snapshot_id,
    };
    let to = CrossSourceEndpoint {
        kind: "knowledge".into(),
        id: knowledge.id,
        revision_id: knowledge.revision_id,
    };
    let kind = "potential_discrepancy";
    let signature = util::json_digest(&(&from, &to, kind)).unwrap();
    let id = format!("xrel_{}", &util::digest(format!("{signature}:{kind}"))[7..]);
    let qualification = "Potential relationship only; production ordering and policy replacement have not been independently verified.";
    reviews::record(
        &conn,
        &config.project_id,
        "companion-source-review",
        qualification,
    )
    .unwrap();
    let review_id = reviews::list(&conn, true)
        .unwrap()
        .into_iter()
        .find(|review| review.reason == qualification)
        .unwrap()
        .id;
    let relation = CrossSourceRelation {
        id: id.clone(),
        input_signature: signature.clone(),
        from: from.clone(),
        to: Some(to.clone()),
        kind: kind.into(),
        reason: "The retained queue requirement and reported staging work need a scope check."
            .into(),
        qualifications: vec![qualification.into()],
        evidence_ids: vec![observation.evidence_id, knowledge.evidence[0].id.clone()],
        upstream_kind: None,
        upstream_status: None,
        upstream_active: None,
        review_id: Some(review_id.clone()),
        active: true,
    };
    conn.execute(
        "INSERT INTO cross_source_evaluations VALUES(?1,?1,?2,?3,?4,?5,?6,?7,?8,'2026-10-09')",
        rusqlite::params![
            signature,
            from.kind,
            from.id,
            from.revision_id,
            to.kind,
            to.id,
            to.revision_id,
            kind
        ],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO cross_source_current VALUES(?1,?1)",
        [&signature],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO cross_source_relations VALUES(?1,?2,?3)",
        rusqlite::params![id, signature, serde_json::to_string(&relation).unwrap()],
    )
    .unwrap();
    imports::relationships::validate_current_relation(&conn, &relation).unwrap();

    let report = companion::changes(&config, &conn, &options()).unwrap();
    assert!(report.registry_changed && report.cross_source_relationships_changed);
    assert!(report.changes.is_empty() && report.relationship_changes.is_empty());
    assert!(report.imported_changes.is_empty());
    assert_eq!(report.cross_source_changes.len(), 1);
    let added = &report.cross_source_changes[0];
    assert_eq!(added.change_kind, "interpretation_added");
    assert!(added.before.is_none());
    assert_eq!(added.after.as_ref().unwrap().relation, relation);
    assert_eq!(report.cross_source_knowledge.len(), 1);
    assert_eq!(report.evidence.len(), 1);
    assert_eq!(report.imported_evidence.len(), 1);
    let markdown = companion::render_changes(&report);
    assert!(markdown.contains(&util::markdown_text(&id)) && markdown.contains(qualification));
    assert!(markdown.contains("at immutable revision"));
    assert!(!markdown.contains("No consequential retained-knowledge change"));
    let guarded = companion::guard(
        &config,
        &conn,
        &options(),
        &RunOptions {
            no_inspect: true,
            no_cache: true,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_ne!(guarded.assessment_status, "no_documented_change");
    assert!(!guarded.execution && !guarded.source_write);
    let shared = serde_json::to_string(guarded.intelligence.as_ref().unwrap()).unwrap();
    assert!(shared.contains(&id) && shared.contains(qualification));
    assert!(
        guarded
            .warnings
            .iter()
            .any(|warning| warning.contains("before-and-after source payloads"))
    );

    // Reconsidering the same endpoint revisions is a paired interpretation
    // revision, not an invented documentary or native record change.
    let saved = companion::save_baseline(&config, &conn, "joined", true).unwrap();
    assert_eq!(saved.cross_source_relationships.len(), 1);
    reviews::manual(
        &conn,
        &review_id,
        "dismissed",
        "Reviewed scope; production remains unverified.",
        "fixture",
    )
    .unwrap();
    let revised_qualification = "Revised interpretation only; the emergency-drain exception and production ordering still require independent verification.";
    let mut revised = interpretation(
        relation.from.clone(),
        relation.to.clone(),
        "uncertain",
        "The scope comparison was reconsidered without changing either endpoint source revision.",
        revised_qualification,
    );
    revised.review_id = Some(review_id.clone());
    revised.evidence_ids = relation.evidence_ids.clone();
    persist_interpretation(&conn, &signature, &revised);
    imports::relationships::validate_current_relation(&conn, &revised).unwrap();
    let revised_report = companion::changes(&config, &conn, &options()).unwrap();
    assert!(revised_report.changes.is_empty() && revised_report.imported_changes.is_empty());
    assert_eq!(revised_report.cross_source_changes.len(), 1);
    let change = &revised_report.cross_source_changes[0];
    assert_eq!(change.pair_key, signature);
    assert_eq!(change.change_kind, "interpretation_revised");
    let mut previous = relation.clone();
    previous.active = false;
    assert_eq!(change.before.as_ref().unwrap().relation, previous);
    assert_eq!(change.after.as_ref().unwrap().relation, revised);
    assert!(change.before.as_ref().unwrap().present_at_checkpoint);
    assert_eq!(change.baseline_review_status, "not_captured");
    assert_eq!(change.current_reviews[0].review_id, review_id);
    assert_eq!(
        change.current_reviews[0].status.as_deref(),
        Some("dismissed")
    );
    let markdown = companion::render_changes(&revised_report);
    assert!(markdown.contains(qualification) && markdown.contains(revised_qualification));
    assert!(markdown.contains("Baseline review disposition was not captured"));
    assert!(markdown.contains("does not independently verify implementation"));
    assert_change_budget(&revised_report);

    // Withdrawal changes membership; it retains the full revised source
    // interpretation and never infers claim reversal or review resolution.
    let saved = companion::save_baseline(&config, &conn, "joined", true).unwrap();
    conn.execute(
        "DELETE FROM cross_source_current WHERE pair_key=?1",
        [&signature],
    )
    .unwrap();
    let withdrawn = companion::changes(&config, &conn, &options()).unwrap();
    assert!(withdrawn.cross_source_relationships_changed);
    assert_eq!(withdrawn.cross_source_changes.len(), 1);
    let change = &withdrawn.cross_source_changes[0];
    assert_eq!(change.change_kind, "interpretation_withdrawn");
    assert!(change.after.is_none());
    let mut original_revised = revised;
    original_revised.active = false;
    assert_eq!(change.before.as_ref().unwrap().relation, original_revised);
    assert!(
        change
            .qualification
            .contains("Withdrawal does not reverse a source claim")
    );
    assert_eq!(withdrawn.cross_source_knowledge.len(), 1);
    assert_eq!(withdrawn.evidence.len(), 1);
    assert_eq!(withdrawn.imported_evidence.len(), 1);
    assert_change_budget(&withdrawn);
    let mut edited = saved;
    edited
        .cross_source_relationships
        .insert(id, "blake3:altered".into());
    rewrite(&config, edited);
    assert!(companion::changes(&config, &conn, &options()).is_err());
}

fn interpretation(
    from: CrossSourceEndpoint,
    to: Option<CrossSourceEndpoint>,
    kind: &str,
    reason: &str,
    qualification: &str,
) -> CrossSourceRelation {
    let signature = util::json_digest(&(&from, &to, kind, reason, qualification)).unwrap();
    CrossSourceRelation {
        id: format!("xrel_{}", &util::digest(format!("{signature}:{kind}"))[7..]),
        input_signature: signature,
        from,
        to,
        kind: kind.into(),
        reason: reason.into(),
        qualifications: vec![qualification.into()],
        evidence_ids: Vec::new(),
        upstream_kind: None,
        upstream_status: None,
        upstream_active: None,
        review_id: None,
        active: true,
    }
}

fn persist_interpretation(
    conn: &rusqlite::Connection,
    pair_key: &str,
    relation: &CrossSourceRelation,
) {
    conn.execute(
        "INSERT INTO cross_source_evaluations VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,'2026-10-09')",
        rusqlite::params![
            relation.input_signature,
            pair_key,
            relation.from.kind,
            relation.from.id,
            relation.from.revision_id,
            relation.to.as_ref().map(|e| &e.kind),
            relation.to.as_ref().map(|e| &e.id),
            relation.to.as_ref().map(|e| &e.revision_id),
            relation.kind
        ],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO cross_source_relations VALUES(?1,?2,?3)",
        rusqlite::params![
            relation.id,
            relation.input_signature,
            serde_json::to_string(relation).unwrap()
        ],
    )
    .unwrap();
    conn.execute("INSERT INTO cross_source_current VALUES(?1,?2) ON CONFLICT(pair_key) DO UPDATE SET input_signature=excluded.input_signature",
        rusqlite::params![pair_key, relation.input_signature]).unwrap();
}

fn assert_change_budget(report: &companion::ChangeReport) {
    let actual = context::count_tokens(&(serde_json::to_string(report).unwrap() + "\n"))
        .max(context::count_tokens(&companion::render_changes(report)));
    assert!(
        report.budget.used_tokens >= actual
            && report.budget.used_tokens <= report.budget.max_tokens
    );
}

fn document_interpretation(
    conn: &rusqlite::Connection,
    knowledge_id: &str,
    reason: &str,
    qualification: &str,
) -> CrossSourceRelation {
    let knowledge = storage::views(conn)
        .unwrap()
        .into_iter()
        .find(|record| record.id == knowledge_id)
        .unwrap();
    let native = imports::views(conn).unwrap().remove(0);
    let mut relation = interpretation(
        CrossSourceEndpoint {
            kind: "observation".into(),
            id: native.id,
            revision_id: native.snapshot_id,
        },
        Some(CrossSourceEndpoint {
            kind: "knowledge".into(),
            id: knowledge.id,
            revision_id: knowledge.revision_id,
        }),
        "potential_discrepancy",
        reason,
        qualification,
    );
    relation.evidence_ids = std::iter::once(native.evidence_id)
        .chain(
            knowledge
                .evidence
                .iter()
                .filter(|e| e.active)
                .map(|e| e.id.clone()),
        )
        .collect();
    relation
}

#[tokio::test]
async fn historical_interpretation_endpoints_keep_their_exact_revision_evidence_after_source_updates()
 {
    let (_dir, config, model) = common::project();
    common::put(
        &config,
        "initial.md",
        "DECISION identity: Identity checks remain required.\n",
    );
    engine::update(&config, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let conn = rusqlite::Connection::open(config.state.join("state.db")).unwrap();
    let statement = "Queue admission preserves arrival order.";
    let old_quote = "Queue admission preserves arrival order except emergency drains.";
    let (id, old_evidence) = captured_support(&config, &conn, statement, old_quote, None);
    native_work(
        &config,
        &conn,
        "Only the isolated staging queue replay was checked; production remains unverified.",
    );
    let old = document_interpretation(
        &conn,
        &id,
        "The earlier exception needs a scope comparison.",
        "Historical comparison; preserve the emergency-drain condition.",
    );
    persist_interpretation(&conn, "queue-comparison", &old);
    companion::save_baseline(&config, &conn, "joined", false).unwrap();
    let new_quote = "Queue admission preserves arrival order except emergency drains only after every tenant key is acknowledged.";
    let (_, new_evidence) = captured_support(&config, &conn, statement, new_quote, Some(&id));
    let new = document_interpretation(
        &conn,
        &id,
        "The acknowledgement condition changes the comparison inputs.",
        "Current interpretation; production checks and policy replacement remain unverified.",
    );
    persist_interpretation(&conn, "queue-comparison", &new);
    let before_reads = conn.total_changes();
    let report = companion::changes(
        &config,
        &conn,
        &ChangeOptions {
            max_tokens: 20_000,
            ..options()
        },
    )
    .unwrap();
    assert_eq!(before_reads, conn.total_changes());
    assert_eq!(report.cross_source_changes.len(), 1);
    let change = &report.cross_source_changes[0];
    let old_revision = &change.before.as_ref().unwrap().knowledge_revision_ids[0];
    let new_revision = &change.after.as_ref().unwrap().knowledge_revision_ids[0];
    assert_ne!(old_revision, new_revision);
    let prior = report
        .cross_source_knowledge
        .iter()
        .find(|record| record.revision_id == *old_revision)
        .unwrap();
    let current = report
        .cross_source_knowledge
        .iter()
        .find(|record| record.revision_id == *new_revision)
        .unwrap();
    assert_eq!(prior.id, current.id);
    assert_eq!(prior.statement, current.statement);
    assert_eq!(prior.evidence_ids, vec![old_evidence.clone()]);
    assert!(current.evidence_ids.contains(&new_evidence));
    assert_eq!(
        report
            .evidence
            .iter()
            .find(|source| source.id == old_evidence)
            .unwrap()
            .excerpt,
        old_quote
    );
    assert_eq!(
        report
            .evidence
            .iter()
            .find(|source| source.id == new_evidence)
            .unwrap()
            .excerpt,
        new_quote
    );
    let markdown = companion::render_changes(&report);
    assert!(markdown.contains(old_quote) && markdown.contains(new_quote));
    assert_change_budget(&report);
    assert!(conn.is_autocommit());
}

#[tokio::test]
async fn interpretation_budget_omits_complete_pairs_and_tampered_endpoint_evidence_is_rejected() {
    let (_dir, mut config, model) = common::project();
    common::put(
        &config,
        "queue.md",
        "DECISION queue: Queue ordering is preserved except emergency drains.\n",
    );
    engine::update(&config, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    config.config.models.generative.enabled = false;
    let conn = rusqlite::Connection::open(config.state.join("state.db")).unwrap();
    native_work(
        &config,
        &conn,
        "The queue replay was staging-only; production is unverified.",
    );
    companion::save_baseline(&config, &conn, "joined", false).unwrap();
    let id = storage::views(&conn).unwrap().remove(0).id;
    let relation = document_interpretation(&conn, &id, "Preserve the full source condition before applying this interpretation.", &"Only the isolated staging replay was checked; do not infer production verification or policy replacement. ".repeat(120));
    persist_interpretation(&conn, "bounded-comparison", &relation);
    let large = companion::changes(
        &config,
        &conn,
        &ChangeOptions {
            max_tokens: 20_000,
            ..options()
        },
    )
    .unwrap();
    assert_eq!(
        large.cross_source_changes[0]
            .after
            .as_ref()
            .unwrap()
            .relation
            .qualifications,
        relation.qualifications
    );
    let limit = ChangeOptions {
        max_tokens: 1_024,
        ..options()
    };
    let small = companion::changes(&config, &conn, &limit).unwrap();
    assert!(small.cross_source_relationships_changed);
    assert!(small.cross_source_changes.is_empty() && small.cross_source_knowledge.is_empty());
    assert!(small.evidence.is_empty() && small.imported_evidence.is_empty());
    assert_eq!(small.omitted_cross_source_changes, 1);
    assert_change_budget(&small);
    let guarded = companion::guard(
        &config,
        &conn,
        &limit,
        &RunOptions {
            no_inspect: true,
            no_cache: true,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(guarded.assessment_status, "change_budget_exhausted");
    assert!(guarded.intelligence.is_none());
    assert!(!guarded.execution && !guarded.source_write);

    // A source-shaped stored payload and a recomputed checkpoint checksum do
    // not make evidence from another immutable revision an endpoint witness.
    let (_unrelated, evidence) = captured_support(
        &config,
        &conn,
        "An unrelated identity rule.",
        "An unrelated identity rule with an independent source condition.",
        None,
    );
    let mut invalid = interpretation(
        relation.from.clone(),
        relation.to.clone(),
        "uncertain",
        "Forged endpoint witness",
        "No source authority is granted by this fixture.",
    );
    invalid.evidence_ids = vec![relation.evidence_ids[0].clone(), evidence];
    persist_interpretation(&conn, "bounded-comparison", &invalid);
    assert!(companion::changes(&config, &conn, &options()).is_err());
    assert!(conn.is_autocommit());
}

fn native_upstream(
    config: &lore::config::ResolvedConfig,
    conn: &rusqlite::Connection,
    status: &str,
    active: bool,
) -> CrossSourceRelation {
    let mut source = AdapterRecord::new(
        "queue-42",
        "Staging queue follow-up; production was not verified.",
        serde_json::json!({"id":"queue-42","status":status,"active":active}),
    );
    source.kind = ObservationKind::WorkState;
    source.lifecycle = "closed".into();
    source.subject = "queue ordering".into();
    source.scope.environment = Some("staging".into());
    source.evidence = vec![NativeEvidence {
        locator: "record://queue-42".into(),
        revision: Some(status.into()),
        field: None,
    }];
    source.relationships = vec![NativeRelationship {
        native_id: Some("queue-link-1".into()),
        target_native_id: "queue-43".into(),
        kind: "relates_to".into(),
        upstream_status: Some(status.into()),
        active,
        native_record: serde_json::json!({"target":"queue-43","status":status,"active":active}),
    }];
    let mut target = AdapterRecord::new(
        "queue-43",
        "Production ordering remains an open source report, not independent verification.",
        serde_json::json!({"id":"queue-43","scope":"production"}),
    );
    target.evidence = vec![NativeEvidence {
        locator: "record://queue-43".into(),
        revision: Some("stable-target".into()),
        field: None,
    }];
    let batch = ImportBatch {
        format: "fixture-work-links-v1".into(),
        records: vec![source, target],
        warnings: vec![],
    };
    imports::storage::persist(
        conn,
        &config.project_id,
        &Inventory {
            sources: vec![SourceBatch {
                id: "work".into(),
                kind: ImportKind::Beads,
                path: config.base.join("work.jsonl"),
                digest: util::json_digest(&batch).unwrap(),
                batch,
            }],
            digest: "fixture".into(),
            warnings: vec![],
        },
    )
    .unwrap();
    let sources = imports::views(conn).unwrap();
    let from = sources
        .iter()
        .find(|source| source.record.native_id == "queue-42")
        .unwrap();
    let to = sources
        .iter()
        .find(|source| source.record.native_id == "queue-43")
        .unwrap();
    let mut relation = interpretation(
        CrossSourceEndpoint {
            kind: "observation".into(),
            id: from.id.clone(),
            revision_id: from.snapshot_id.clone(),
        },
        Some(CrossSourceEndpoint {
            kind: "observation".into(),
            id: to.id.clone(),
            revision_id: to.snapshot_id.clone(),
        }),
        "upstream_relationship",
        "Source-native queue relationship; retain its vocabulary and direction.",
        "An upstream link is not evidence of accepted policy replacement or current implementation verification.",
    );
    relation.upstream_kind = Some("relates_to".into());
    relation.upstream_status = Some(status.into());
    relation.upstream_active = Some(active);
    relation.evidence_ids = vec![from.evidence_id.clone(), to.evidence_id.clone()];
    let native = &from.record.relationships[0];
    let key = util::json_digest(&(
        "upstream",
        &from.id,
        &native.native_id,
        &native.target_native_id,
        &native.kind,
    ))
    .unwrap();
    persist_interpretation(conn, &key, &relation);
    relation
}

#[tokio::test]
async fn historical_native_relationship_status_remains_independent_of_current_membership() {
    let (_dir, config, model) = common::project();
    common::put(
        &config,
        "queue.md",
        "DECISION queue: Queue ordering remains required.\n",
    );
    engine::update(&config, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let conn = rusqlite::Connection::open(config.state.join("state.db")).unwrap();
    let old = native_upstream(&config, &conn, "pending", false);
    companion::save_baseline(&config, &conn, "joined", false).unwrap();
    let new = native_upstream(&config, &conn, "ignored", false);
    let report = companion::changes(
        &config,
        &conn,
        &ChangeOptions {
            max_tokens: 20_000,
            ..options()
        },
    )
    .unwrap();
    assert_eq!(report.cross_source_changes.len(), 1);
    let change = &report.cross_source_changes[0];
    assert_eq!(change.change_kind, "interpretation_revised");
    let before = change.before.as_ref().unwrap();
    let after = change.after.as_ref().unwrap();
    assert_eq!(before.relation.id, old.id);
    assert_eq!(after.relation.id, new.id);
    assert_eq!(before.relation.upstream_status.as_deref(), Some("pending"));
    assert_eq!(after.relation.upstream_status.as_deref(), Some("ignored"));
    assert!(!before.relation.active && after.relation.active);
    assert_eq!(after.relation.upstream_active, Some(false));
    let before_source = before
        .imported
        .iter()
        .find(|source| source.id == old.from.id)
        .unwrap();
    let after_source = after
        .imported
        .iter()
        .find(|source| source.id == new.from.id)
        .unwrap();
    assert!(
        before_source.current && after_source.current,
        "each source was current at its own checkpoint"
    );
    let old_source = report
        .imported_evidence
        .iter()
        .find(|source| source.evidence_id == before_source.evidence_id)
        .unwrap();
    let new_source = report
        .imported_evidence
        .iter()
        .find(|source| source.evidence_id == after_source.evidence_id)
        .unwrap();
    assert!(
        !old_source.current && new_source.current,
        "original payload reader reports current membership at report time"
    );
    assert_eq!(
        old_source.record.relationships[0]
            .upstream_status
            .as_deref(),
        Some("pending")
    );
    assert_eq!(
        new_source.record.relationships[0]
            .upstream_status
            .as_deref(),
        Some("ignored")
    );
    let markdown = companion::render_changes(&report);
    assert!(
        markdown.contains("Upstream status: pending")
            && markdown.contains("Upstream status: ignored")
    );
    assert!(markdown.contains("Asserted upstream: false"));
    assert!(markdown.contains("production was not verified"));
    assert_change_budget(&report);
}
