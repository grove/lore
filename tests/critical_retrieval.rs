mod common;

use lore::{
    context::{self, ContextOptions, adaptive, decision::runtime::RunOptions},
    domain::AssertionProposal,
    sources::{self, Document},
    storage, util,
};
use rusqlite::{Connection, params};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::Path, process::Command};

#[derive(Deserialize)]
struct Manifest {
    records: Vec<Record>,
    relations: Vec<Relation>,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Record {
    id: String,
    source: String,
    source_sha256: String,
    section: String,
    quote: String,
    topic: String,
    subject: String,
    kind: String,
    lifecycle: String,
    scope: String,
    state: String,
    evidence_id: String,
}

#[derive(Deserialize)]
struct Relation {
    from_id: String,
    to_id: String,
    kind: String,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    task: String,
    paths: Vec<String>,
    max_tokens: usize,
    required: Vec<String>,
    required_evidence_ids: Vec<String>,
    excluded: Vec<String>,
}

fn fixture(path: &Path) -> (Connection, Manifest) {
    let manifest: Manifest =
        serde_json::from_str(include_str!("fixtures/critical-retrieval/manifest.json")).unwrap();
    let conn = Connection::open(path).unwrap();
    storage::migrate(&conn).unwrap();
    conn.execute(
        "INSERT INTO projects VALUES('p','Critical retrieval','2026-10-14')",
        [],
    )
    .unwrap();
    conn.execute("INSERT INTO source_roots VALUES('docs','p','./docs')", [])
        .unwrap();
    conn.execute("INSERT INTO lore_meta VALUES('initialized','1')", [])
        .unwrap();
    let mut assertions = BTreeMap::new();
    for record in &manifest.records {
        let text = fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/critical-retrieval")
                .join(&record.source),
        )
        .unwrap();
        assert_eq!(
            format!("{:x}", Sha256::digest(text.as_bytes())),
            record.source_sha256
        );
        let source = format!("src_{}", record.source);
        let revision = format!("sr_{}", record.source);
        let section = format!("s_{}", record.section);
        let section_revision = format!("sec_{}", record.section);
        conn.execute(
            "INSERT OR IGNORE INTO sources(id,root_id,relative_path) VALUES(?1,'docs',?2)",
            params![source, record.source],
        )
        .unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO source_revisions VALUES(?1,?2,?3,?4,'2026-10-14')",
            params![revision, source, record.source, util::digest(&text)],
        )
        .unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO source_current VALUES(?1,?2,?3)",
            params![source, revision, util::digest(&text)],
        )
        .unwrap();
        let exists: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM source_revision_provenance WHERE source_revision_id=?1)", [&revision], |r| r.get(0)).unwrap();
        if !exists {
            conn.execute(
                "INSERT INTO source_revision_provenance VALUES(?1,'docs','./docs','primary',NULL)",
                [&revision],
            )
            .unwrap();
        }
        let document = Document {
            root_id: "docs".into(),
            root_path: "./docs".into(),
            material: Default::default(),
            origin: None,
            relative_path: record.source.clone(),
            physical_path: record.source.clone().into(),
            digest: util::digest(&text),
            chunks: sources::split_markdown(&text, "docs", &record.source, 8_000).unwrap(),
            text,
        };
        let chunk = document
            .chunks
            .iter()
            .find(|c| c.text.contains(&record.quote))
            .unwrap();
        conn.execute(
            "INSERT INTO source_sections VALUES(?1,?2)",
            params![section, source],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO section_revisions VALUES(?1,?2,?3,'[]',?4,?5,?6)",
            params![
                section_revision,
                section,
                revision,
                util::digest(&chunk.text),
                document.text[..chunk.offset].lines().count() + 1,
                document.text[..chunk.offset].lines().count() + chunk.text.lines().count()
            ],
        )
        .unwrap();
        let proposal = AssertionProposal {
            topic: record.topic.clone(),
            topic_title: record.topic.clone(),
            subject: record.subject.clone(),
            statement: record.quote.clone(),
            kind: record.kind.clone(),
            lifecycle: record.lifecycle.clone(),
            scope: record.scope.clone(),
            effective_at: String::new(),
            quote: record.quote.clone(),
        };
        let (assertion, evidence) = storage::capture_assertion(
            &conn,
            storage::AssertionCapture {
                document: &document,
                chunk,
                proposal: &proposal,
                source: &source,
                source_revision: &revision,
                section: &section,
                section_revision: &section_revision,
                model: "pinned-fixture",
            },
        )
        .unwrap();
        assert_eq!(evidence, record.evidence_id, "{}", record.id);
        conn.execute(
            "INSERT OR IGNORE INTO topics VALUES(?1,'p',?1,?1)",
            [&record.topic],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO knowledge_units VALUES(?1,'p','2026-10-14')",
            [&record.id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO knowledge_details VALUES(?1,?2,?3,?4,'',?5)",
            params![
                record.id,
                record.topic,
                record.subject,
                record.scope,
                record.lifecycle
            ],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO topic_units VALUES(?1,?2)",
            params![record.topic, record.id],
        )
        .unwrap();
        let kr = format!("kr_{}", record.id);
        conn.execute(
            "INSERT INTO knowledge_revisions VALUES(?1,?2,1,?3,?4,?5,?6,'','2026-10-14')",
            params![
                kr,
                record.id,
                record.quote,
                record.kind,
                record.lifecycle,
                match record.state.as_str() {
                    "unsupported" => "unsupported",
                    "withdrawn" => "historical_only",
                    _ => "current_documentary_support",
                }
            ],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO knowledge_current VALUES(?1,?2,'fixture')",
            params![record.id, kr],
        )
        .unwrap();
        if record.state != "unsupported" {
            storage::assign(&conn, &assertion, &record.id).unwrap();
            conn.execute(
                "INSERT INTO knowledge_support VALUES(?1,?2,'supports')",
                params![kr, assertion],
            )
            .unwrap();
        }
        conn.execute("INSERT INTO sealed_knowledge VALUES(?1)", [&kr])
            .unwrap();
        if record.state == "withdrawn" {
            storage::retire_section(&conn, &section).unwrap();
            storage::retire_source(&conn, &source).unwrap();
        }
        assertions.insert(record.id.clone(), (assertion, evidence));
    }
    for relation in &manifest.relations {
        let (assertion, evidence) = &assertions[&relation.from_id];
        storage::add_relation(
            &conn,
            &relation.from_id,
            &relation.to_id,
            &relation.kind,
            assertion,
            evidence,
        )
        .unwrap();
    }
    // Populate the normal search projection without changing the pinned support states.
    for record in &manifest.records {
        conn.execute(
            "INSERT INTO knowledge_fts VALUES(?1,?2,?3,?4)",
            params![record.id, record.quote, record.subject, record.topic],
        )
        .unwrap();
    }
    (conn, manifest)
}

fn options(case: &Case) -> ContextOptions {
    ContextOptions {
        task: case.task.clone(),
        paths: case.paths.clone(),
        max_tokens: case.max_tokens,
    }
}

#[test]
fn pinned_critical_records_survive_fast_selection_and_negative_controls() {
    let (_dir, config, model) = common::project();
    fs::create_dir_all(&config.state).unwrap();
    let (conn, manifest) = fixture(&config.state.join("state.db"));
    for case in &manifest.cases {
        let result = context::build_context(&conn, &options(case)).unwrap();
        let ids: Vec<_> = result.sections.items().map(|item| &item.id).collect();
        for id in &case.required {
            assert!(ids.contains(&id), "{}: missing {id}", case.name);
            let record = manifest.records.iter().find(|r| &r.id == id).unwrap();
            assert!(
                result
                    .evidence
                    .iter()
                    .any(|e| e.id == record.evidence_id && e.excerpt == record.quote)
            );
        }
        for id in &case.required_evidence_ids {
            assert!(
                result.evidence.iter().any(|e| &e.id == id),
                "{}: missing {id}",
                case.name
            );
        }
        for id in &case.excluded {
            assert!(!ids.contains(&id), "{}: elevated {id}", case.name);
        }
        if case.required.iter().any(|id| id == "ku_rejected") {
            assert!(
                result
                    .sections
                    .historical
                    .iter()
                    .any(|item| item.id == "ku_rejected" && item.lifecycle == "rejected")
            );
            assert!(
                !result
                    .sections
                    .constraints
                    .iter()
                    .any(|item| item.id == "ku_rejected")
            );
        }
        assert!(result.budget.used_tokens <= case.max_tokens);
        assert_eq!(result.model_calls, 0);
        let args = options(case);
        let mut command = Command::new(env!("CARGO_BIN_EXE_lore"));
        command.arg("--config").arg(&config.config_path).args([
            "--json",
            "context",
            "--fast",
            &case.task,
            "--max-tokens",
            &case.max_tokens.to_string(),
        ]);
        for path in &args.paths {
            command.args(["--path", path]);
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
        let cli: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(cli, serde_json::to_value(&result).unwrap());
    }
    assert_eq!(model.calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert_eq!(
        conn.query_row::<i64, _, _>("SELECT count(*) FROM model_calls", [], |r| r.get(0))
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn tight_budgets_keep_groups_whole_and_prevent_readiness() {
    let (_dir, config, model) = common::project();
    let (conn, manifest) = fixture(Path::new(":memory:"));
    for case in manifest
        .cases
        .iter()
        .take(5)
        .filter(|c| c.required.len() > 1)
    {
        let mut options = options(case);
        options.max_tokens = 700;
        let selected = context::build_context(&conn, &options).unwrap();
        let retained = selected
            .sections
            .items()
            .filter(|i| case.required.contains(&i.id))
            .count();
        assert!(
            retained == 0 || retained == case.required.len(),
            "{}: cut-off condition",
            case.name
        );
        assert!(selected.omissions.critical_groups > 0, "{}", case.name);
        options.max_tokens = 2400;
        let result = adaptive::build(
            &conn,
            &config,
            &options,
            selected,
            Some(&model),
            &RunOptions::default(),
        )
        .await
        .unwrap();
        let result = serde_json::to_value(result).unwrap();
        assert_eq!(
            result["intelligence"]["investigation_status"],
            "incomplete_retrieval"
        );
        assert_ne!(result["intelligence"]["mode"], "reference");
        assert_ne!(result["intelligence"]["brief"]["readiness"], "proceed");
    }
    assert_eq!(model.calls.load(std::sync::atomic::Ordering::SeqCst), 0);
}

#[tokio::test]
async fn truncated_query_and_semantic_index_cannot_claim_decision_sufficiency() {
    use context::semantic::{SemanticHit, SemanticReport};
    let (_dir, config, model) = common::project();
    let (conn, manifest) = fixture(Path::new(":memory:"));
    let mut options = options(&manifest.cases[0]);
    options.max_tokens = 12_000;
    options.task = format!(
        "redelivery {}",
        (0..40)
            .map(|i| format!("term{i}"))
            .collect::<Vec<_>>()
            .join(" ")
    );
    let selected = context::build_context(&conn, &options).unwrap();
    assert!(selected.retrieval_truncated);
    assert!(
        context::render_context(&selected).contains("Unselected records could not be assessed")
    );
    let result = adaptive::build(
        &conn,
        &config,
        &options,
        selected,
        Some(&model),
        &RunOptions::default(),
    )
    .await
    .unwrap();
    let value = serde_json::to_value(result).unwrap();
    assert_eq!(
        value["intelligence"]["investigation_status"],
        "incomplete_retrieval"
    );
    assert_eq!(value["intelligence"]["retrieval_truncated"], true);

    options.task = "Revise unrelated terminology".into();
    for (seed, excluded) in [
        ("ku_staging", "ku_production"),
        ("ku_aurora", "ku_withdrawn"),
    ] {
        let semantic = SemanticReport {
            knowledge: vec![
                SemanticHit {
                    id: seed.into(),
                    score: 1.0,
                },
                SemanticHit {
                    id: "ku_unsupported".into(),
                    score: 1.0,
                },
            ],
            truncated: true,
            ..SemanticReport::default()
        };
        let selected = context::build_context_with_semantic(&conn, &options, &semantic).unwrap();
        assert!(selected.retrieval_truncated);
        assert!(selected.sections.items().any(|item| item.id == seed));
        assert!(
            !selected
                .sections
                .items()
                .any(|item| item.id == excluded || item.id == "ku_unsupported")
        );
        let result = context::decision::runtime::build_decision_context(
            &conn,
            &config,
            &options,
            selected,
            Some(&model),
            &RunOptions::default(),
        )
        .await
        .unwrap();
        let value = serde_json::to_value(result).unwrap();
        assert_eq!(value["investigation_status"], "incomplete_retrieval");
        assert_eq!(value["retrieval_truncated"], true);
    }
    assert_eq!(model.calls.load(std::sync::atomic::Ordering::SeqCst), 0);
}

#[tokio::test]
async fn prose_packing_cannot_hide_the_rejected_endpoint() {
    use context::decision::{
        self, DraftDecision,
        runtime::{self, DecisionContextResult},
    };
    use serde_json::json;
    let (_dir, config, _model) = common::project();
    let (conn, manifest) = fixture(Path::new(":memory:"));
    let mut options = options(&manifest.cases[2]);
    options.max_tokens = 12_000;
    let selected = context::build_context(&conn, &options).unwrap();
    assert_eq!(selected.omissions.knowledge_units, 0);
    let chosen = selected
        .sections
        .decisions
        .iter()
        .find(|i| i.id == "ku_chosen")
        .unwrap();
    let advice = json!({"text":"Keep the bounded spool while extracting its allocation step.","evidence_ids":chosen.evidence_ids,"observation_ids":[]});
    let draft: DraftDecision = serde_json::from_value(json!({
        "readiness":"proceed", "change_kind":"behavior_preserving",
        "preferred_approach":advice, "rationale":advice, "main_tradeoff":advice, "next_action":advice,
        "completion_criteria":[advice], "known_record_ids":[], "hypotheses":[], "heuristics":[],
        "constraints":[{"knowledge_id":chosen.id,"disposition":"preserved","explanation":"Keep the adopted spool boundary.","evidence_ids":chosen.evidence_ids,"observation_ids":[]}],
        "checks":[], "implementation_seams":[], "risks":[], "material_blockers":[], "counterevidence":[], "remaining_uncertainty":[]
    })).unwrap();
    let brief =
        decision::validate_decision(&conn, &selected, &[], &draft, "fixture-revision").unwrap();
    assert!(
        brief
            .facts
            .iter()
            .any(|f| f.record.record_id == "ku_rejected" && f.record.lifecycle == "rejected")
    );
    let mut packed = false;
    for budget in [1600, 2400, 3200, 6000] {
        options.max_tokens = budget;
        let result = runtime::build_decision_context(
            &conn,
            &config,
            &options,
            selected.clone(),
            None,
            &RunOptions::default(),
        )
        .await
        .unwrap();
        match result {
            DecisionContextResult::Brief(result) => {
                packed |= result.brief_items_omitted > 0;
                for id in &manifest.cases[2].required {
                    assert!(
                        result
                            .brief
                            .facts
                            .iter()
                            .any(|fact| &fact.record.record_id == id)
                    );
                }
            }
            DecisionContextResult::FastFallback(result) => {
                let count = result
                    .context
                    .sections
                    .items()
                    .filter(|i| manifest.cases[2].required.contains(&i.id))
                    .count();
                assert!(count == 0 || count == 2);
            }
        }
    }
    assert!(packed, "fixture must exercise removal of optional prose");
}
