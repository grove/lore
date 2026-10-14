//! Real upstream bytes, authored labels, offline evidence-mechanics controls.
//! This deliberately tests neither model extraction nor independent task success.
mod common;

use lore::{
    config::ResolvedConfig,
    context::{self, ContextOptions},
    domain::AssertionProposal,
    engine::{self, UpdateOptions},
    sources::{self, Chunk},
    storage, util,
};
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::PathBuf, process::Command, sync::atomic::Ordering};

#[derive(Deserialize)]
struct Manifest {
    projects: BTreeMap<String, Project>,
    records: Vec<Record>,
    relations: Vec<Relation>,
    cases: Vec<Case>,
    negative_queries: Vec<NegativeQuery>,
}

#[derive(Deserialize)]
struct Project {
    repository: String,
    revision: String,
    source_root: String,
    files_sha256: BTreeMap<String, String>,
}

#[derive(Deserialize)]
struct Record {
    id: String,
    project: String,
    source: String,
    lines: [i64; 2],
    quote: String,
    statement: String,
    subject: String,
    kind: String,
    lifecycle: String,
    scope: String,
}

#[derive(Deserialize)]
struct Relation {
    from: String,
    to: String,
    kind: String,
    witness: String,
}

#[derive(Deserialize)]
struct Case {
    id: String,
    project: String,
    task: String,
    required: Vec<String>,
    max_tokens: usize,
}

#[derive(Deserialize)]
struct NegativeQuery {
    project: String,
    task: String,
}

fn corpus() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("evaluation/corpora/real-decisions-v1")
}

fn cli(config: &ResolvedConfig, args: &[&str]) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_lore"))
        .arg("--config")
        .arg(&config.config_path)
        .arg("--json")
        .args(args)
        .env_remove("OPENAI_API_KEY")
        .env_remove("LORE_INSPECTION_ROOT")
        .env_remove("LORE_ALLOW_HOSTED_EGRESS")
        .env_remove("LORE_ALLOW_CHECKOUT_EGRESS")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{args:?}: {} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[tokio::test]
async fn upstream_decision_controls_resolve_select_and_disclose_omissions_offline() {
    let manifest: Manifest =
        serde_json::from_slice(&fs::read(corpus().join("manifest.json")).unwrap()).unwrap();
    for (name, project) in &manifest.projects {
        let (_dir, config, model) = common::project();
        let mut settings = config.config.clone();
        settings.sources.roots[0].origin =
            Some(format!("{}@{}", project.repository, project.revision));
        settings.output.mode = Some(lore::config::OutputMode::RegistryOnly);
        fs::write(
            &config.config_path,
            serde_yaml::to_string(&settings).unwrap(),
        )
        .unwrap();
        let config = ResolvedConfig::load(&config.config_path).unwrap();
        // Initialize the ordinary registry with no model and no documents.
        engine::update(&config, &model, None, UpdateOptions::default())
            .await
            .unwrap();
        for (path, hash) in &project.files_sha256 {
            let bytes = fs::read(corpus().join(&project.source_root).join(path)).unwrap();
            assert_eq!(format!("{:x}", Sha256::digest(&bytes)), *hash);
            if path != "LICENSE" {
                // Byte-identical Markdown-named mirrors let status/audit inspect
                // the snapshots without introducing an RST or HTML parser.
                common::put(
                    &config,
                    &format!("{path}.md"),
                    std::str::from_utf8(&bytes).unwrap(),
                );
            }
        }
        let conn = rusqlite::Connection::open(config.state.join("state.db")).unwrap();
        let project_id: String = conn
            .query_row("SELECT id FROM projects", [], |r| r.get(0))
            .unwrap();
        let mut records = BTreeMap::new();
        for document in sources::scan(&config).unwrap().documents {
            let (source, revision) = storage::begin_source(&conn, &document, None).unwrap();
            for record in manifest.records.iter().filter(|r| {
                &r.project == name && document.relative_path == format!("{}.md", r.source)
            }) {
                // Annotations supply the extraction, never a simulated model judgment.
                let chunk = Chunk {
                    key: record.id.clone(),
                    heading: String::new(),
                    heading_path: vec![],
                    text: record.quote.clone(),
                    context: String::new(),
                    offset: document.text.find(&record.quote).unwrap(),
                    input_digest: util::digest(&record.quote),
                };
                let (section, section_revision) =
                    storage::begin_section(&conn, &source, &revision, &chunk).unwrap();
                let proposal = AssertionProposal {
                    topic: name.clone(),
                    topic_title: name.clone(),
                    subject: record.subject.clone(),
                    statement: record.statement.clone(),
                    quote: record.quote.clone(),
                    kind: record.kind.clone(),
                    lifecycle: record.lifecycle.clone(),
                    scope: record.scope.clone(),
                    effective_at: String::new(),
                };
                let (assertion, evidence) = storage::capture_assertion(
                    &conn,
                    storage::AssertionCapture {
                        document: &document,
                        chunk: &chunk,
                        proposal: &proposal,
                        source: &source,
                        source_revision: &revision,
                        section: &section,
                        section_revision: &section_revision,
                        model: "authored-source-label-no-inference",
                    },
                )
                .unwrap();
                let unit = storage::create_unit(&conn, &project_id, &assertion, &proposal).unwrap();
                records.insert(record.id.clone(), (unit, assertion, evidence));
            }
        }
        for relation in manifest
            .relations
            .iter()
            .filter(|r| records.contains_key(&r.from))
        {
            let (_, assertion, evidence) = &records[&relation.witness];
            storage::add_relation(
                &conn,
                &records[&relation.from].0,
                &records[&relation.to].0,
                &relation.kind,
                assertion,
                evidence,
            )
            .unwrap();
        }
        storage::refresh_knowledge(&conn, &project_id).unwrap();
        let views = storage::views(&conn).unwrap();
        assert_eq!(views.len(), records.len());
        for record in manifest.records.iter().filter(|r| &r.project == name) {
            let (unit, _, evidence) = &records[&record.id];
            let view = views.iter().find(|v| &v.id == unit).unwrap();
            assert_eq!(view.lifecycle, record.lifecycle);
            assert_eq!(view.base_lifecycle, record.lifecycle);
            assert_eq!(view.scope, record.scope);
            assert_eq!(view.kind, record.kind);
            assert_eq!(view.support_state, "current_documentary_support");
            assert!(view.evidence.iter().all(|e| e.active));
            let snapshot = storage::evidence_snapshot(&conn, evidence).unwrap();
            assert_eq!(snapshot.excerpt, record.quote);
            assert_eq!(snapshot.digest, util::digest(&record.quote));
            assert_eq!(
                [snapshot.line_start.unwrap(), snapshot.line_end.unwrap()],
                record.lines
            );
            assert_eq!(snapshot.path, format!("{}.md", record.source));
            assert_eq!(snapshot.origin, settings.sources.roots[0].origin);
            assert_eq!(snapshot.material, lore::domain::SourceMaterial::Primary);
            assert_eq!(
                snapshot.source_revision_id,
                view.evidence[0].source_revision_id
            );
            assert_eq!(
                cli(&config, &["evidence", evidence])["excerpt"],
                record.quote
            );
        }
        let before = fs::read(config.state.join("state.db")).unwrap();
        for case in manifest.cases.iter().filter(|c| &c.project == name) {
            let options = ContextOptions {
                task: case.task.clone(),
                paths: vec![],
                max_tokens: case.max_tokens,
            };
            let selected = context::build_context(&conn, &options).unwrap();
            for required in &case.required {
                let (unit, _, evidence) = &records[required];
                let expected = manifest.records.iter().find(|r| &r.id == required).unwrap();
                let item = selected
                    .sections
                    .items()
                    .find(|i| &i.id == unit)
                    .unwrap_or_else(|| panic!("{}: missing {required}", case.id));
                assert_eq!(item.lifecycle, expected.lifecycle);
                assert_eq!(item.scope, expected.scope);
                assert_eq!(item.support_state, "current_documentary_support");
                assert!(
                    selected.evidence.iter().any(|e| &e.id == evidence),
                    "{}: missing evidence for {required}",
                    case.id
                );
                assert_eq!(
                    selected
                        .evidence
                        .iter()
                        .find(|e| &e.id == evidence)
                        .unwrap()
                        .excerpt,
                    storage::evidence_snapshot(&conn, evidence).unwrap().excerpt
                );
            }
            assert_eq!(selected.model_calls, 0);
            let fast = cli(
                &config,
                &[
                    "context",
                    "--fast",
                    &case.task,
                    "--max-tokens",
                    &case.max_tokens.to_string(),
                ],
            );
            assert_eq!(fast, serde_json::to_value(&selected).unwrap());
            for command in ["decisions", "cases"] {
                let result = cli(&config, &[command, &case.task, "--max-tokens", "48000"]);
                assert_eq!(result["model_calls"], 0);
                // Both views retain the ordinary context, including all required
                // source-owned lifecycles, scopes and complete quoted conditions.
                let context: context::ContextResult =
                    serde_json::from_value(result["context"].clone()).unwrap();
                for required in &case.required {
                    assert!(
                        context
                            .sections
                            .items()
                            .any(|i| i.id == records[required].0),
                        "{}: {command} missing {required}",
                        case.id
                    );
                }
                if command == "cases" {
                    assert_eq!(result["runtime_execution"], false);
                    assert_eq!(result["tests_run"], 0);
                    if case.required.iter().any(|id| id == "pep640") {
                        assert!(
                            result["cases"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .any(|c| c["record"]["id"] == records["pep640"].0
                                    && c["kind"] == "rejected_approach")
                        );
                    }
                } else if case.required.iter().any(|id| id == "pep640") {
                    assert!(
                        result["negative_knowledge"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .any(|n| n["knowledge_id"] == records["pep640"].0)
                    );
                }
            }
            let bounded = context::build_context(
                &conn,
                &ContextOptions {
                    max_tokens: 512,
                    ..options
                },
            )
            .unwrap();
            assert!(
                bounded.omissions.knowledge_units > 0,
                "{}: exercise omissions",
                case.id
            );
            for item in bounded.sections.items() {
                for evidence in &item.evidence_ids {
                    assert_eq!(
                        bounded
                            .evidence
                            .iter()
                            .find(|e| &e.id == evidence)
                            .unwrap()
                            .excerpt,
                        storage::evidence_snapshot(&conn, evidence).unwrap().excerpt
                    );
                }
            }
            if case.id == "explicit-supersession" {
                let count = bounded
                    .sections
                    .items()
                    .filter(|i| case.required.iter().any(|r| records[r].0 == i.id))
                    .count();
                assert!(count == 0 || count == 2, "partial replacement group");
                if count == 0 {
                    assert!(bounded.omissions.critical_groups > 0);
                }
            }
        }
        for query in manifest
            .negative_queries
            .iter()
            .filter(|q| &q.project == name)
        {
            let result = context::build_context(
                &conn,
                &ContextOptions {
                    task: query.task.clone(),
                    paths: vec![],
                    max_tokens: 24000,
                },
            )
            .unwrap();
            assert_eq!(result.sections.items().count(), 0);
        }
        assert_eq!(cli(&config, &["audit"])["ok"], true);
        assert_eq!(before, fs::read(config.state.join("state.db")).unwrap());
        assert_eq!(model.calls.load(Ordering::SeqCst), 0);
        assert_eq!(
            conn.query_row::<i64, _, _>("SELECT count(*) FROM model_calls", [], |r| r.get(0))
                .unwrap(),
            0
        );
    }
}
