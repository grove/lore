mod common;
use common::*;
use lore::{
    domain,
    engine::{self, UpdateOptions},
    inference::*,
    provider_wire, util,
};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    fs,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

fn citation_enum(schema: &Value) -> &Value {
    &schema["properties"]["sections"]["items"]["properties"]["paragraphs"]["items"]["properties"]["knowledge_ids"]
        ["items"]["enum"]
}

#[test]
fn scoped_citations_are_closed_in_both_provider_request_formats() {
    let ids: BTreeSet<_> = ["ku_first", "ku_second"]
        .into_iter()
        .map(String::from)
        .collect();
    let schema = domain::page_schema_for(&ids).unwrap();
    assert_eq!(citation_enum(&schema), &json!(["ku_first", "ku_second"]));
    assert!(domain::page_schema_for(&BTreeSet::new()).is_err());
    let request = GenerationRequest {
        instructions: "Synthesize only supplied knowledge".into(),
        input: "{}".into(),
        schema: Some(schema.clone()),
    };
    let hosted = provider_wire::openai_responses_request(&request, "fixture");
    assert_eq!(hosted["text"]["format"]["strict"], true);
    assert_eq!(
        citation_enum(&hosted["text"]["format"]["schema"]),
        citation_enum(&schema)
    );
    let local = provider_wire::ollama_chat_request(&request, "fixture");
    assert_eq!(citation_enum(&local["format"]), citation_enum(&schema));
    assert_eq!(request.schema, Some(schema));
}

/// Checks the actual wire contract rather than returning idealised hard-coded
/// outputs. A schema-ignoring provider can still return bad IDs and is rejected.
struct ContractModel {
    inner: FakeModel,
    fail_overview: AtomicBool,
    fail_first: bool,
    overview_calls: AtomicUsize,
    extract_calls: AtomicUsize,
    reject_verification: AtomicBool,
}
impl ContractModel {
    fn new(fail_first: bool) -> Self {
        Self {
            inner: FakeModel::new(),
            fail_overview: AtomicBool::new(false),
            fail_first,
            overview_calls: AtomicUsize::new(0),
            extract_calls: AtomicUsize::new(0),
            reject_verification: AtomicBool::new(false),
        }
    }
}
impl GenerativeModel for ContractModel {
    fn descriptor(&self) -> &ModelDescriptor {
        self.inner.descriptor()
    }
    fn generate<'a>(&'a self, r: &'a GenerationRequest) -> ModelFuture<'a, GenerationResponse> {
        Box::pin(async move {
            let input: Value = serde_json::from_str(&r.input).unwrap();
            let task = input["task"].as_str().unwrap();
            if task == "extract" {
                self.extract_calls.fetch_add(1, Ordering::SeqCst);
            }
            if matches!(task, "overview" | "synthesize") {
                let rows = input["knowledge"].as_array().unwrap();
                let ids: BTreeSet<_> = rows.iter().map(|u| u["id"].as_str().unwrap()).collect();
                let choices: BTreeSet<_> = citation_enum(r.schema.as_ref().unwrap())
                    .as_array()
                    .expect("citations must be constrained at generation time")
                    .iter()
                    .map(|x| x.as_str().unwrap())
                    .collect();
                assert_eq!(choices, ids);
                if task == "overview" {
                    for link in input["documented_decision_relationships"]
                        .as_array()
                        .unwrap()
                    {
                        for key in ["from_id", "to_id"] {
                            let id = link[key].as_str().unwrap();
                            assert!(
                                ids.contains(id),
                                "decision context leaked an uncitable endpoint"
                            );
                            let row = rows.iter().find(|u| u["id"] == id).unwrap();
                            assert!(!row["evidence"].as_array().unwrap().is_empty());
                        }
                    }
                    let attempt = self.overview_calls.fetch_add(1, Ordering::SeqCst);
                    if self.fail_overview.load(Ordering::SeqCst)
                        || (self.fail_first && attempt == 0)
                    {
                        return Ok(GenerationResponse { model: "fixture".into(), text: json!({"sections":[
                            {"heading":"Overview","paragraphs":[{"text":"Do not persist rejected private prose.",
                                "knowledge_ids":["ev_private_not_a_knowledge_id"]}]}]}).to_string() });
                    }
                    if self.fail_first && attempt == 1 {
                        assert!(r.instructions.contains("knowledge[].id"));
                        assert!(r.instructions.contains("schema enum"));
                    }
                }
            }
            if task == "verify_overview" && self.reject_verification.load(Ordering::SeqCst) {
                return Ok(GenerationResponse { model:"fixture".into(), text:json!({
                    "supported":false,"issues":["The claim is not supported despite a valid citation."]
                }).to_string() });
            }
            self.inner.generate(r).await
        })
    }
}

#[tokio::test]
async fn overview_repairs_schema_ignoring_citations_then_mutates_and_noops() {
    let (_tmp, cfg, _) = project();
    put(
        &cfg,
        "ADR-001.md",
        "DECISION ledger: MySQL is the selected database.\n",
    );
    let model = ContractModel::new(true);
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    assert_eq!(model.overview_calls.load(Ordering::SeqCst), 2);
    let files = fs::read_dir(cfg.state.join("citation-diagnostics"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect::<Vec<_>>();
    assert_eq!(files.len(), 1);
    let diagnostic = fs::read_to_string(&files[0]).unwrap();
    assert!(diagnostic.contains("outside_request_scope"));
    assert!(diagnostic.contains(&util::digest("ev_private_not_a_knowledge_id")));
    assert!(!diagnostic.contains("ev_private_not_a_knowledge_id"));
    assert!(!diagnostic.contains("Do not persist rejected private prose"));
    assert!(!diagnostic.contains("selected database"));

    put(
        &cfg,
        "review.md",
        "DECISION review: The review reaffirms ADR-001 and its MySQL choice.\n",
    );
    put(
        &cfg,
        "ADR-027.md",
        "DECISION ledger: PostgreSQL replaces ADR-001 for the production database.\n",
    );
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let index = fs::read(cfg.wiki.join("index.md")).unwrap();
    assert!(String::from_utf8_lossy(&index).contains("explicitly supersedes"));
    let calls = model.inner.calls.load(Ordering::SeqCst);
    let no_op = engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    assert!(no_op.no_op);
    assert_eq!(no_op.model_calls, 0);
    assert_eq!(model.inner.calls.load(Ordering::SeqCst), calls);
    assert_eq!(index, fs::read(cfg.wiki.join("index.md")).unwrap());
}

#[tokio::test]
async fn unknown_citations_still_fail_closed_and_preserve_initial_publication() {
    let (_tmp, cfg, _) = project();
    let model = ContractModel::new(false);
    put(&cfg, "first.md", "DECISION ledger: Use durable storage.\n");
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let old_index = fs::read(cfg.wiki.join("index.md")).unwrap();
    let old_db = fs::read(cfg.state.join("state.db")).unwrap();
    put(&cfg, "second.md", "DECISION cache: Use a local cache.\n");
    model.fail_overview.store(true, Ordering::SeqCst);
    let error = engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("cited unknown or duplicate knowledge"));
    assert_eq!(fs::read(cfg.wiki.join("index.md")).unwrap(), old_index);
    assert_eq!(fs::read(cfg.state.join("state.db")).unwrap(), old_db);
    assert_eq!(model.overview_calls.load(Ordering::SeqCst), 3);
    // Correctly formed IDs do not excuse unsupported model-written prose.
    // An explicitly degraded evidence index is permitted, but it cannot be
    // treated as a verified project overview or a passing beta evaluation.
    model.fail_overview.store(false, Ordering::SeqCst);
    model.reject_verification.store(true, Ordering::SeqCst);
    let result = engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    assert!(result.degraded_overview);
    assert!(result.warnings.iter().any(|w| w.contains("OVERVIEW_DEGRADED")));
    let index = fs::read_to_string(cfg.wiki.join("index.md")).unwrap();
    assert!(index.contains("lore:degraded-overview-synthesis"));
    assert!(index.contains("## Overview evidence"));
    assert!(!index.contains("Do not persist rejected private prose"));
    let noop = engine::update(&cfg, &model, None, UpdateOptions::default()).await.unwrap();
    assert!(noop.no_op);
    assert_eq!(noop.model_calls, 0);
    assert!(noop.degraded_overview);
}

#[tokio::test]
async fn presentation_upgrade_reuses_extracted_assertions_then_returns_to_noop() {
    let (_tmp, cfg, _) = project();
    let model = ContractModel::new(false);
    put(&cfg, "first.md", "DECISION ledger: Use durable storage.\n");
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let extracted = model.extract_calls.load(Ordering::SeqCst);
    let db = rusqlite::Connection::open(cfg.state.join("state.db")).unwrap();
    // Emulate a pre-fix baseline with unchanged source/configuration metadata.
    db.execute(
        "DELETE FROM lore_meta WHERE key='presentation_contract'",
        [],
    )
    .unwrap();
    drop(db);
    let status = engine::status(&cfg).unwrap();
    assert!(status.presentation_changed);
    assert!(!status.configuration_changed);
    let changed = engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    assert!(!changed.no_op);
    assert_eq!(changed.processed_sections, 0);
    assert_eq!(changed.extracted_assertions, 0);
    assert_eq!(model.extract_calls.load(Ordering::SeqCst), extracted);
    assert!(!engine::status(&cfg).unwrap().presentation_changed);
    assert!(
        engine::update(&cfg, &model, None, UpdateOptions::default())
            .await
            .unwrap()
            .no_op
    );
}
