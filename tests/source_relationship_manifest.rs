//! Schema-5 relationship retention is independent of optional generated prose.
mod common;

use lore::{
    config::ResolvedConfig,
    context::{
        self, ContextOptions, adaptive,
        decision::runtime::{self, RunOptions},
    },
    domain::ImportKind,
    engine::{self, UpdateOptions},
    experience::{self, ExperienceMode, ExperienceOptions},
    imports::{
        self, Inventory, SourceBatch,
        adapters::{AdapterRecord, ImportBatch, NativeEvidence, ObservationKind, ObservationScope},
        relationships::{CrossSourceEndpoint, CrossSourceRelation},
    },
    reviews, storage, util,
};
use rusqlite::Connection;
use serde_json::json;
use std::{collections::BTreeSet, sync::atomic::Ordering};

const TASK: &str = "Compare queue ordering with staging work source reports";
const RULE: &str = "Production queue ordering must be preserved except during emergency drains; a drain still refuses new jobs.";
const QUALIFICATION: &str = "Potential relationship only; production ordering and policy replacement have not been independently verified.";
const DISPOSITION: &str = "Review was dismissed; this disposition does not independently verify implementation or establish agreement between the sources.";

struct Fixture {
    _dir: tempfile::TempDir,
    config: ResolvedConfig,
    conn: Connection,
    model: common::FakeModel,
    relation: CrossSourceRelation,
}

fn run_options() -> RunOptions {
    RunOptions {
        no_inspect: true,
        no_cache: true,
        ..Default::default()
    }
}

fn options(max_tokens: usize) -> ContextOptions {
    ContextOptions {
        task: TASK.into(),
        paths: Vec::new(),
        max_tokens,
    }
}

fn human_options() -> ExperienceOptions {
    ExperienceOptions {
        goal: Some(TASK.into()),
        mode: ExperienceMode::Explanation,
        max_tokens: 16_000,
        run: run_options(),
        ..Default::default()
    }
}

impl Fixture {
    async fn new() -> Self {
        let (dir, mut config, model) = common::project();
        common::put(&config, "queue.md", &format!("DECISION queue: {RULE}\n"));
        engine::update(&config, &model, None, UpdateOptions::default())
            .await
            .unwrap();
        config.config.models.generative.enabled = false;
        let conn = Connection::open(config.state.join("state.db")).unwrap();
        let statement = "The queue replay is reported closed for staging only; production ordering was not checked.";
        let mut record = AdapterRecord::new(
            "queue-42",
            statement,
            json!({
                "id":"queue-42", "status":"closed", "statement":statement,
                "relationship_status":"closed", "relationship_asserted":false,
            }),
        );
        record.kind = ObservationKind::WorkState;
        record.subject = "queue ordering".into();
        record.title = "Staging queue replay".into();
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
            warnings: Vec::new(),
        };
        let source = SourceBatch {
            id: "work".into(),
            kind: ImportKind::Beads,
            path: config.base.join("work.jsonl"),
            digest: util::json_digest(&batch).unwrap(),
            batch,
        };
        imports::storage::persist(
            &conn,
            &config.project_id,
            &Inventory {
                sources: vec![source],
                digest: "source-relationship-fixture".into(),
                warnings: Vec::new(),
            },
        )
        .unwrap();
        reviews::record(
            &conn,
            &config.project_id,
            "source-relationship-review",
            "Review queue source scope.",
        )
        .unwrap();
        let review = reviews::list(&conn, true)
            .unwrap()
            .into_iter()
            .find(|review| review.reason == "Review queue source scope.")
            .unwrap();
        reviews::manual(
            &conn,
            &review.id,
            "dismissed",
            "Scope was reviewed, not implementation.",
            "fixture-reviewer",
        )
        .unwrap();
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
        let signature = util::json_digest(&(&from, &to, "potential_discrepancy")).unwrap();
        let relation = CrossSourceRelation {
            id: format!("xrel_{}", &util::digest(&signature)[7..]),
            input_signature: signature.clone(),
            from: from.clone(),
            to: Some(to.clone()),
            kind: "potential_discrepancy".into(),
            reason:
                "The retained production rule and reported staging work require a scope comparison."
                    .into(),
            qualifications: vec![QUALIFICATION.into()],
            evidence_ids: vec![observation.evidence_id, knowledge.evidence[0].id.clone()],
            upstream_kind: Some("relates_to".into()),
            upstream_status: Some("closed".into()),
            upstream_active: Some(false),
            review_id: Some(review.id),
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
                relation.kind
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
            rusqlite::params![
                relation.id,
                signature,
                serde_json::to_string(&relation).unwrap()
            ],
        )
        .unwrap();
        imports::relationships::validate_current_relation(&conn, &relation).unwrap();
        Self {
            _dir: dir,
            config,
            conn,
            model,
            relation,
        }
    }

    async fn shared(&self, max_tokens: usize) -> adaptive::AdaptiveResult {
        let selected = context::build_context(&self.conn, &options(16_000)).unwrap();
        adaptive::build(
            &self.conn,
            &self.config,
            &options(max_tokens),
            selected,
            None,
            &run_options(),
        )
        .await
        .unwrap()
    }
}

fn assert_budget(result: &adaptive::AdaptiveResult) {
    let actual = context::count_tokens(&(serde_json::to_string(result).unwrap() + "\n"))
        .max(context::count_tokens(&adaptive::render(result)));
    assert!(
        actual <= result.budget.used_tokens
            && result.budget.used_tokens <= result.budget.max_tokens
    );
}

#[tokio::test]
async fn source_status_review_qualifications_and_complete_endpoints_survive_shared_packing() {
    let fixture = Fixture::new().await;
    let revision = storage::registry_revision(&fixture.conn).unwrap();
    let writes = fixture.conn.total_changes();
    let calls = fixture.model.calls.load(Ordering::SeqCst);
    let result = fixture.shared(8_000).await;
    assert_eq!(result.schema_version, 5);
    assert_budget(&result);
    let manifest = &result.source_relationships;
    assert_eq!(manifest.relations, vec![fixture.relation.clone()]);
    assert!(manifest.relations[0].active);
    assert_eq!(manifest.relations[0].upstream_active, Some(false));
    assert_eq!(manifest.discrepancies[0].status, "dismissed");
    assert!(
        manifest.discrepancies[0]
            .qualifications
            .iter()
            .any(|q| q == DISPOSITION)
    );
    assert_eq!(manifest.knowledge[0].statement, RULE);
    assert_eq!(manifest.observations[0].lifecycle, "closed");
    assert_eq!(
        manifest.observations[0].scope.environment.as_deref(),
        Some("staging")
    );
    let evidence_ids = manifest
        .evidence
        .iter()
        .map(|e| &e.id)
        .chain(manifest.imported_evidence.iter().map(|e| &e.id))
        .collect::<BTreeSet<_>>();
    assert!(
        fixture
            .relation
            .evidence_ids
            .iter()
            .all(|id| evidence_ids.contains(id))
    );
    assert_eq!(
        manifest.knowledge_revisions[&fixture.relation.to.as_ref().unwrap().id],
        fixture.relation.to.as_ref().unwrap().revision_id
    );
    assert_eq!(
        manifest.imported_evidence[0].snapshot_id,
        fixture.relation.from.revision_id
    );
    let markdown = adaptive::render(&result);
    assert!(markdown.contains(QUALIFICATION) && markdown.contains(DISPOSITION));
    assert!(
        markdown.contains("Asserted upstream: false")
            && markdown.contains("Upstream status: closed")
    );

    let from_runtime = adaptive::run(
        &fixture.config,
        &fixture.conn,
        &options(8_000),
        &run_options(),
    )
    .await
    .unwrap();
    assert_eq!(
        serde_json::to_value(&from_runtime.source_relationships).unwrap(),
        serde_json::to_value(manifest).unwrap()
    );
    assert_budget(&from_runtime);
    let selected = context::build_context(&fixture.conn, &options(8_000)).unwrap();
    let legacy = runtime::build_decision_context(
        &fixture.conn,
        &fixture.config,
        &options(8_000),
        selected,
        None,
        &run_options(),
    )
    .await
    .unwrap();
    let legacy = serde_json::to_value(legacy).unwrap();
    assert_eq!(legacy["schema_version"], 4);
    assert!(legacy.get("source_relationships").is_none());
    assert_eq!(calls, fixture.model.calls.load(Ordering::SeqCst));
    assert_eq!(writes, fixture.conn.total_changes());
    assert_eq!(revision, storage::registry_revision(&fixture.conn).unwrap());
}

#[tokio::test]
async fn human_views_keep_the_same_source_manifest_and_qualify_endpoint_claims() {
    let fixture = Fixture::new().await;
    let shared = fixture.shared(8_000).await;
    let expected = serde_json::to_value(&shared.source_relationships).unwrap();
    let result = experience::build(
        &fixture.conn,
        &fixture.config,
        &human_options(),
        shared,
        None,
    )
    .await
    .unwrap();
    assert_eq!(
        serde_json::to_value(&result.source_relationships).unwrap(),
        expected
    );
    let rule = result
        .orientation
        .constraints
        .iter()
        .find(|claim| claim.text == RULE)
        .unwrap();
    assert!(rule.qualifications.iter().any(|q| q == QUALIFICATION));
    assert!(rule.qualifications.iter().any(|q| q == DISPOSITION));
    let markdown = experience::render_markdown(&result);
    assert!(markdown.contains(QUALIFICATION) && markdown.contains(DISPOSITION));
    let actual = context::count_tokens(&(serde_json::to_string(&result).unwrap() + "\n"))
        .max(context::count_tokens(&markdown));
    assert!(
        actual <= result.budget.used_tokens
            && result.budget.used_tokens <= result.budget.max_tokens
    );
    for evidence in result
        .source_relationships
        .evidence
        .iter()
        .map(|e| &e.id)
        .chain(
            result
                .source_relationships
                .imported_evidence
                .iter()
                .map(|e| &e.id),
        )
    {
        assert!(markdown.contains(&format!("### Source {evidence}")));
    }
}

#[tokio::test]
async fn human_adapter_rejects_forged_qualifications_status_revisions_and_missing_group_members() {
    let fixture = Fixture::new().await;
    let original = fixture.shared(8_000).await;
    let calls = fixture.model.calls.load(Ordering::SeqCst);
    for mutation in 0..7 {
        let mut changed = original.clone();
        let manifest = &mut changed.source_relationships;
        match mutation {
            0 => manifest.relations[0].qualifications.clear(),
            1 => manifest.relations[0].upstream_active = Some(true),
            2 => manifest.discrepancies[0]
                .qualifications
                .retain(|q| q != DISPOSITION),
            3 => manifest.observations.clear(),
            4 => {
                manifest.knowledge_revisions.insert(
                    fixture.relation.to.as_ref().unwrap().id.clone(),
                    manifest.evidence[0].source_revision_id.clone(),
                );
            }
            5 => manifest.imported_evidence.clear(),
            _ => *manifest = adaptive::SourceRelationships::default(),
        }
        assert!(
            experience::build(
                &fixture.conn,
                &fixture.config,
                &human_options(),
                changed,
                Some(&fixture.model)
            )
            .await
            .is_err()
        );
        assert!(fixture.conn.is_autocommit());
    }
    assert_eq!(calls, fixture.model.calls.load(Ordering::SeqCst));
}

#[tokio::test]
async fn complete_group_budget_is_reserved_before_inference_and_never_shortens_conditions() {
    let fixture = Fixture::new().await;
    let full = fixture.shared(8_000).await;
    let bounded = fixture.shared(full.budget.used_tokens + 128).await;
    assert_budget(&bounded);
    assert_eq!(
        serde_json::to_value(&bounded.source_relationships).unwrap(),
        serde_json::to_value(&full.source_relationships).unwrap()
    );
    let source_tokens =
        context::count_tokens(&serde_json::to_string(&full.source_relationships).unwrap());
    assert!(source_tokens > 512);
    let selected = context::build_context(&fixture.conn, &options(16_000)).unwrap();
    let calls = fixture.model.calls.load(Ordering::SeqCst);
    let result = adaptive::build(
        &fixture.conn,
        &fixture.config,
        &options(source_tokens - 1),
        selected,
        Some(&fixture.model),
        &run_options(),
    )
    .await;
    assert!(
        result.is_err(),
        "a complete manifest cannot fit in fewer tokens than its own JSON"
    );
    assert_eq!(calls, fixture.model.calls.load(Ordering::SeqCst));
    assert!(fixture.conn.is_autocommit());
}
