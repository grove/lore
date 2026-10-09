//! End-to-end schema 4 tests with retained SQLite evidence, real checkout files,
//! and an explicit provider double. No test or source command is ever executed.
use lore::{
    config::{Config, ResolvedConfig},
    context::{
        self, ContextOptions, ContextResult,
        decision::runtime::{self, DecisionContextResult, RunOptions},
    },
    domain::{AssertionProposal, SourceMaterial},
    inference::{
        ExecutionLocation, GenerationRequest, GenerationResponse, GenerativeModel, ModelDescriptor,
        ModelError, ModelFuture, Provider,
    },
    sources::{self, Document},
    storage, util,
};
use rusqlite::Connection;
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    fs::{self, FileTimes, OpenOptions},
    path::PathBuf,
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

const RULE: &str =
    "Dispatch uses bounded queues. Preserve per-key ordering in src/dispatch/queue.rs.";
const QUEUE: &str = "// checkout_only_marker_9173\npub const QUEUE_CAPACITY: usize = 128;\n";
const DEEP: &str = "pub const USE_BOUNDED_GATEWAY: bool = false;\n";

struct Fixture {
    dir: tempfile::TempDir,
    conn: Connection,
    config: ResolvedConfig,
    options: ContextOptions,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        for path in [".git", "docs", "src/dispatch", "tests"] {
            fs::create_dir_all(dir.path().join(path)).unwrap();
        }
        fs::write(dir.path().join("docs/dispatch-contract.md"), RULE).unwrap();
        fs::write(dir.path().join("src/dispatch/queue.rs"), QUEUE).unwrap();
        fs::write(
            dir.path().join("src/dispatch/allocation.rs"),
            "pub fn allocate_dispatch_queue() {}\n",
        )
        .unwrap();
        fs::write(dir.path().join("src/dispatch/zz_policy.rs"), DEEP).unwrap();
        fs::write(
            dir.path().join("tests/dispatch_queue.rs"),
            "#[test]\nfn dispatch_keeps_key_order_and_queue_bound() {}\n",
        )
        .unwrap();
        let mut config = Config::default();
        config.project.name = "Decision fixture".into();
        config.context.investigation.max_rounds = 1;
        config.context.inspection.max_elapsed_ms = 5_000;
        let config_path = dir.path().join("lore.yml");
        fs::write(&config_path, serde_yaml::to_string(&config).unwrap()).unwrap();
        let config = ResolvedConfig::load(&config_path).unwrap();
        let conn = Connection::open_in_memory().unwrap();
        storage::migrate(&conn).unwrap();
        conn.execute(
            "INSERT INTO projects VALUES('p','Decision fixture','2026-10-09')",
            [],
        )
        .unwrap();
        let docs = dir.path().join("docs").to_string_lossy().to_string();
        conn.execute(
            "INSERT INTO source_roots(id,project_id,configured_path) VALUES('docs','p',?1)",
            [&docs],
        )
        .unwrap();
        let document = Document {
            root_id: "docs".into(),
            root_path: docs.into(),
            material: SourceMaterial::Primary,
            origin: None,
            relative_path: "dispatch-contract.md".into(),
            physical_path: dir.path().join("docs/dispatch-contract.md"),
            text: RULE.into(),
            digest: util::digest(RULE),
            chunks: sources::split_markdown(RULE, "docs", "dispatch-contract.md", 8_000).unwrap(),
        };
        let proposal = AssertionProposal {
            topic: "dispatch".into(),
            topic_title: "Dispatch".into(),
            subject: "dispatch queues".into(),
            statement: RULE.into(),
            kind: "constraint".into(),
            lifecycle: "accepted".into(),
            scope: "production".into(),
            effective_at: String::new(),
            quote: RULE.into(),
        };
        let (source, revision) = storage::begin_source(&conn, &document, None).unwrap();
        let chunk = &document.chunks[0];
        let (section, section_revision) =
            storage::begin_section(&conn, &source, &revision, chunk).unwrap();
        let (assertion, _) = storage::capture_assertion(
            &conn,
            storage::AssertionCapture {
                document: &document,
                chunk,
                proposal: &proposal,
                source: &source,
                source_revision: &revision,
                section: &section,
                section_revision: &section_revision,
                model: "fixture-no-inference",
            },
        )
        .unwrap();
        storage::create_unit(&conn, "p", &assertion, &proposal).unwrap();
        storage::refresh_knowledge(&conn, "p").unwrap();
        Self {
            dir,
            conn,
            config,
            options: ContextOptions {
                task: "Refactor dispatch queue allocation".into(),
                paths: vec![],
                max_tokens: 12_000,
            },
        }
    }

    fn selected(&self) -> ContextResult {
        context::build_context(
            &self.conn,
            &ContextOptions {
                max_tokens: 12_000,
                ..self.options.clone()
            },
        )
        .unwrap()
    }

    async fn run(&self, model: &FakeModel, run: RunOptions) -> Value {
        let result = runtime::build_decision_context(
            &self.conn,
            &self.config,
            &self.options,
            self.selected(),
            Some(model),
            &run,
        )
        .await
        .unwrap();
        complete_and_bounded(&result);
        serde_json::to_value(result).unwrap()
    }

    fn cache_files(&self) -> Vec<PathBuf> {
        fs::read_dir(self.config.state.join("context-cache"))
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("decision-")
            })
            .collect()
    }

    fn source_snapshot(&self) -> Vec<Vec<u8>> {
        [
            "docs/dispatch-contract.md",
            "src/dispatch/queue.rs",
            "src/dispatch/zz_policy.rs",
            "tests/dispatch_queue.rs",
            "src/dispatch/allocation.rs",
        ]
        .iter()
        .map(|path| fs::read(self.dir.path().join(path)).unwrap())
        .collect()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Behavior {
    Good,
    InvalidJson,
    WrongCitation,
    RejectedVerification,
    MissingObservationReview,
    InvalidPlan,
    UnchangedCounterevidence,
    MutateFile,
    Slow,
    UnsupportedHeuristic,
    InlineObservation,
}

struct FakeModel {
    descriptor: ModelDescriptor,
    behavior: Mutex<Behavior>,
    requests: Mutex<Vec<GenerationRequest>>,
    calls: AtomicUsize,
    mutation: Option<PathBuf>,
}

impl FakeModel {
    fn new(behavior: Behavior) -> Self {
        Self {
            descriptor: ModelDescriptor {
                provider: Provider::Ollama,
                model: "decision-fixture".into(),
                location: ExecutionLocation::Local,
            },
            behavior: Mutex::new(behavior),
            requests: Mutex::new(Vec::new()),
            calls: AtomicUsize::new(0),
            mutation: None,
        }
    }

    fn hosted(behavior: Behavior) -> Self {
        let mut model = Self::new(behavior);
        model.descriptor.provider = Provider::OpenAi;
        model.descriptor.location = ExecutionLocation::Hosted;
        model
    }

    fn inputs(&self) -> Vec<Value> {
        self.requests
            .lock()
            .unwrap()
            .iter()
            .map(|request| serde_json::from_str(&request.input).unwrap())
            .collect()
    }
}

fn advice(text: &str, evidence: &str, observations: Vec<String>) -> Value {
    json!({"text":text,"evidence_ids":[evidence],"observation_ids":observations})
}

fn draft(input: &Value, behavior: Behavior) -> Value {
    let source = &input["source_context"];
    let constraints = source["adopted_constraints"].as_object().unwrap();
    let (knowledge_id, ids) = constraints.iter().next().unwrap();
    let evidence = ids[0].as_str().unwrap();
    let observations = input["local_observations"].as_array().unwrap();
    let queue = observations
        .iter()
        .find(|o| o["path"] == "src/dispatch/queue.rs");
    let deep = observations
        .iter()
        .find(|o| o["path"] == "src/dispatch/zz_policy.rs");
    let refs = queue
        .map(|o| vec![o["id"].as_str().unwrap().to_string()])
        .unwrap_or_default();
    let initial = "Refactor dispatch queue allocation while preserving the existing bounded gateway and per-key ordering.";
    let bypass = deep.is_some_and(|o| o["excerpt"].as_str().unwrap().contains("false"));
    let preferred = if bypass && behavior != Behavior::UnchangedCounterevidence {
        "Route dispatch allocation through the bounded gateway before refactoring per-key scheduling."
    } else if deep.is_some() && !bypass {
        "Simplify queue allocation behind the enabled bounded gateway while preserving per-key ordering."
    } else {
        initial
    };
    let mut result = json!({
        "readiness":"proceed", "change_kind":if bypass {"behavior_changing"} else {"behavior_preserving"},
        "preferred_approach":advice(preferred,evidence,refs.clone()),
        "rationale":advice("Keeping the adopted queue bound and per-key ordering makes the allocation change compatible with the recorded contract.",evidence,vec![]),
        "main_tradeoff":advice("Preserving per-key ordering limits parallelism for a single key.",evidence,vec![]),
        "next_action":advice("Isolate dispatch allocation behind the bounded gateway while retaining per-key enqueue order.",evidence,refs.clone()),
        "completion_criteria":[advice("Repeated-key dispatches retain their ordering and cannot exceed the queue bound.",evidence,vec![])],
        "known_record_ids":[], "hypotheses":[],
        "heuristics":[{"provenance":"general_engineering","principle":"Preserve externally observable outcomes during a structural refactor.","application":"Separate allocation changes from delivery-order changes."}],
        "constraints":[{"knowledge_id":knowledge_id,"disposition":"preserved","explanation":"Keep bounded queues and per-key ordering unchanged.","evidence_ids":[evidence],"observation_ids":[]}],
        "checks":[{"priority":"recommended_during_implementation","action":"Exercise repeated-key dispatches at the queue boundary.","decision_impact":"Reordering or exceeding the queue bound reveals a preserving-change regression.","inexpensive":true,"evidence_ids":[evidence],"observation_ids":[]}],
        "implementation_seams":[], "risks":[], "material_blockers":[], "counterevidence":[], "remaining_uncertainty":[]
    });
    if let Some(queue) = queue {
        result["implementation_seams"] = json!([{"path":queue["path"],"observation_ids":[queue["id"]],"purpose":"Retain the queue capacity boundary while extracting allocation."}]);
    }
    if deep.is_none() {
        result["hypotheses"] = json!([{"provenance":"hypothesis","text":"Allocation likely routes through the bounded gateway before dispatch.","evidence_ids":[evidence],"observation_ids":refs,"confidence":"medium","applicability":"Applies if callers use the bounded gateway for each key.","historical_only":false,"alternatives":[advice("A dispatch branch may bypass the bounded gateway.",evidence,vec![])]}]);
    }
    if !input["previous_brief"].is_null() && bypass {
        let deep = deep.unwrap();
        result["counterevidence"] = json!([{"hypothesis":"The existing allocation path already uses the bounded gateway.","explanation":"The displayed gateway flag is false, contradicting the assumption that allocation already enters that gateway.","evidence_ids":[evidence],"observation_ids":[deep["id"]],"material":true}]);
        result["preferred_approach"]["observation_ids"] = json!([deep["id"]]);
    }
    if behavior == Behavior::WrongCitation {
        result["next_action"]["evidence_ids"] = json!(["ev_invented"]);
    }
    if behavior == Behavior::UnsupportedHeuristic {
        result["heuristics"][0]["principle"] =
            json!("This project always deploys exactly one dispatch worker.");
    }
    if behavior == Behavior::InlineObservation {
        let inline = observations
            .iter()
            .find(|observation| observation["path"] == "tests/dispatch_queue.rs")
            .unwrap();
        result["preferred_approach"]["text"] = json!(format!(
            "{preferred} Use the ordering expectation in {} to shape the regression case.",
            inline["id"].as_str().unwrap()
        ));
        result["heuristics"] = json!((0..6).map(|_| json!({
            "provenance":"general_engineering",
            "principle":"Prefer bounded work queues when protecting latency under sustained input. ".repeat(14),
            "application":"Preserve the dispatch bound throughout the structural allocation change. ".repeat(8),
        })).collect::<Vec<_>>());
    }
    result
}

fn all_references(value: &Value) -> (BTreeSet<String>, BTreeSet<String>) {
    fn visit(value: &Value, evidence: &mut BTreeSet<String>, observations: &mut BTreeSet<String>) {
        match value {
            Value::Object(object) => {
                for (name, child) in object {
                    if name == "evidence_ids" || name == "observation_ids" {
                        for id in child.as_array().unwrap() {
                            if name == "evidence_ids" {
                                evidence.insert(id.as_str().unwrap().to_string());
                            } else {
                                observations.insert(id.as_str().unwrap().to_string());
                            }
                        }
                    } else if matches!(
                        name.as_str(),
                        "text"
                            | "applicability"
                            | "principle"
                            | "application"
                            | "explanation"
                            | "action"
                            | "decision_impact"
                            | "purpose"
                            | "decision_needed"
                            | "hypothesis"
                            | "uncertainty"
                            | "previous_recommendation"
                            | "revised_recommendation"
                    ) && child.is_string()
                    {
                        for word in child
                            .as_str()
                            .unwrap()
                            .split(|c: char| !c.is_alphanumeric() && c != '_')
                        {
                            if word.starts_with("ev_") || word.starts_with("ne_") {
                                evidence.insert(word.to_string());
                            } else if word.starts_with("co_") {
                                observations.insert(word.to_string());
                            }
                        }
                    } else {
                        visit(child, evidence, observations);
                    }
                }
            }
            Value::Array(values) => {
                for child in values {
                    visit(child, evidence, observations);
                }
            }
            _ => (),
        }
    }
    let mut evidence = BTreeSet::new();
    let mut observations = BTreeSet::new();
    visit(value, &mut evidence, &mut observations);
    (evidence, observations)
}

fn verification(input: &Value, behavior: Behavior) -> Value {
    let (mut evidence, mut observations) = all_references(&input["brief"]);
    let (trace_evidence, trace_observations) = all_references(&input["investigation_steps"]);
    evidence.extend(trace_evidence);
    observations.extend(trace_observations);
    if behavior == Behavior::MissingObservationReview {
        observations.clear();
    }
    let rejected = matches!(
        behavior,
        Behavior::RejectedVerification | Behavior::UnsupportedHeuristic
    );
    let constraints = input["source_context"]["adopted_constraints"].as_object().unwrap().iter().map(|(id, evidence)| {
        json!({"knowledge_id":id,"acceptable":true,"explanation":"The preferred action retains bounded queues and per-key ordering.","evidence_ids":evidence})
    }).collect::<Vec<_>>();
    json!({
        "supported":!rejected,"readiness_supported":true,"counterevidence_addressed":true,
        "checked_evidence_ids":evidence,"checked_observation_ids":observations,
        "constraint_checks":constraints,
        "issues":if rejected {vec!["A generated project assertion is unsupported by the retained evidence."]} else {vec![]},
    })
}

fn plan(input: &Value, behavior: Behavior) -> Value {
    let candidate = input["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .find(|candidate| candidate["path"] == "src/dispatch/zz_policy.rs");
    match candidate {
        Some(candidate) => json!({
            "uncertainty":"Does the allocation path actually use the bounded dispatch gateway?",
            "hypothesis":"The existing allocation path already uses the bounded gateway.",
            "candidate_id":if behavior == Behavior::InvalidPlan {"../../etc/shadow"} else {candidate["id"].as_str().unwrap()},
            "expected_discriminator":"Read the gateway flag declaration to distinguish enabled bounded routing from a bypass.",
            "stop":false,
        }),
        None => {
            json!({"uncertainty":"","hypothesis":"","candidate_id":"","expected_discriminator":"","stop":true})
        }
    }
}

impl GenerativeModel for FakeModel {
    fn descriptor(&self) -> &ModelDescriptor {
        &self.descriptor
    }
    fn generate<'a>(
        &'a self,
        request: &'a GenerationRequest,
    ) -> ModelFuture<'a, GenerationResponse> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.requests.lock().unwrap().push(request.clone());
            let behavior = *self.behavior.lock().unwrap();
            if behavior == Behavior::Slow {
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            let input: Value = serde_json::from_str(&request.input).unwrap();
            let output = match input["task"].as_str().unwrap() {
                "context_decision" => {
                    if behavior == Behavior::InvalidJson {
                        return Ok(GenerationResponse {
                            model: self.descriptor.model.clone(),
                            text: "not-json".into(),
                        });
                    }
                    draft(&input, behavior)
                }
                "context_investigation" => plan(&input, behavior),
                "context_decision_verification" => {
                    if behavior == Behavior::MutateFile {
                        fs::write(self.mutation.as_ref().unwrap(), QUEUE.replace("128", "256"))
                            .unwrap();
                    }
                    verification(&input, behavior)
                }
                other => {
                    return Err(ModelError::InvalidRequest(format!(
                        "unexpected fixture task {other}"
                    )));
                }
            };
            Ok(GenerationResponse {
                model: self.descriptor.model.clone(),
                text: serde_json::to_string(&output).unwrap(),
            })
        })
    }
}

fn complete_and_bounded(result: &DecisionContextResult) {
    let value = serde_json::to_value(result).unwrap();
    let (evidence, observations) = all_references(&value);
    let available_evidence = value["evidence"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["id"].as_str().unwrap().to_string())
        .collect::<BTreeSet<_>>();
    let available_observations = value["inspection"]["observations"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|o| o["id"].as_str().unwrap().to_string())
        .collect::<BTreeSet<_>>();
    assert!(
        evidence.is_subset(&available_evidence),
        "dangling retained evidence citation"
    );
    assert!(
        observations.is_subset(&available_observations),
        "dangling static observation citation"
    );
    let reported = value["budget"]["used_tokens"].as_u64().unwrap() as usize;
    let max = value["budget"]["max_tokens"].as_u64().unwrap() as usize;
    assert!(context::count_tokens(&(serde_json::to_string(result).unwrap() + "\n")) <= reported);
    assert!(context::count_tokens(&runtime::render(result)) <= reported);
    assert!(reported <= max);
}

#[tokio::test]
async fn normal_schema_four_has_independent_support_check_and_revision_bound_cache_hit() {
    let fixture = Fixture::new();
    let model = FakeModel::new(Behavior::Good);
    let before = fixture.source_snapshot();
    let first = fixture.run(&model, RunOptions::default()).await;
    assert_eq!(first["schema_version"], 4);
    assert_eq!(first["mode"], "intelligent");
    assert_eq!(first["model_calls"], 2);
    assert_eq!(first["cache_status"], "miss");
    assert_eq!(first["inspection"]["status"], "disabled");
    assert_eq!(
        model
            .inputs()
            .iter()
            .map(|i| i["task"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["context_decision", "context_decision_verification"]
    );
    let second = fixture.run(&model, RunOptions::default()).await;
    assert_eq!(second["cache_status"], "hit");
    assert_eq!(second["model_calls"], 0);
    assert_eq!(first["brief"], second["brief"]);
    assert_eq!(model.calls.load(Ordering::SeqCst), 2);
    assert_eq!(fixture.cache_files().len(), 1);
    assert_eq!(
        before,
        fixture.source_snapshot(),
        "reasoning must not modify source files"
    );
}

#[tokio::test]
async fn invalid_generation_and_failed_review_fall_back_without_poisoning_the_cache() {
    for behavior in [
        Behavior::InvalidJson,
        Behavior::WrongCitation,
        Behavior::RejectedVerification,
        Behavior::UnsupportedHeuristic,
    ] {
        let fixture = Fixture::new();
        let model = FakeModel::new(behavior);
        let failed = fixture.run(&model, RunOptions::default()).await;
        assert_eq!(failed["mode"], "fast_fallback");
        assert_eq!(
            failed["brief"]["generation_basis"],
            "deterministic_fallback"
        );
        assert!(fixture.cache_files().is_empty());
        *model.behavior.lock().unwrap() = Behavior::Good;
        let next = fixture.run(&model, RunOptions::default()).await;
        assert_eq!(next["mode"], "intelligent");
        assert_eq!(next["cache_status"], "miss");
    }
}

#[tokio::test]
async fn hosted_retained_knowledge_permission_never_implies_checkout_egress() {
    let mut fixture = Fixture::new();
    fixture.config.config.privacy.local_only = false;
    let model = FakeModel::hosted(Behavior::Good);
    let result = fixture
        .run(
            &model,
            RunOptions {
                investigate: true,
                ..RunOptions::default()
            },
        )
        .await;
    assert_eq!(result["mode"], "intelligent");
    assert_eq!(result["model_calls"], 2);
    assert_eq!(result["checkout_egress"]["allowed"], false);
    assert_eq!(result["checkout_egress"]["model_received_checkout"], false);
    assert!(
        !result["inspection"]["observations"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        result["investigation"]["stop_reason"],
        "checkout_egress_withheld"
    );
    for request in model.requests.lock().unwrap().iter() {
        let wire = format!(
            "{}{}{}",
            request.instructions,
            request.input,
            request.schema.as_ref().unwrap()
        );
        for withheld in [
            "checkout_only_marker_9173",
            "tests/dispatch_queue.rs",
            "src/dispatch/zz_policy.rs",
            fixture.dir.path().to_str().unwrap(),
        ] {
            assert!(
                !wire.contains(withheld),
                "unauthorized checkout content or metadata leaked: {withheld}"
            );
        }
    }
}

#[tokio::test]
async fn explicit_checkout_permission_allows_hosted_static_evidence_and_cache_hit_sends_nothing() {
    let mut fixture = Fixture::new();
    fixture.config.config.privacy.local_only = false;
    let model = FakeModel::hosted(Behavior::Good);
    let options = RunOptions {
        inspect: true,
        allow_checkout_egress: true,
        ..RunOptions::default()
    };
    let first = fixture.run(&model, options.clone()).await;
    assert_eq!(first["mode"], "intelligent");
    assert_eq!(first["checkout_egress"]["allowed"], true);
    assert_eq!(first["checkout_egress"]["model_received_checkout"], true);
    assert!(
        model.requests.lock().unwrap()[0]
            .input
            .contains("checkout_only_marker_9173")
    );
    let second = fixture.run(&model, options).await;
    assert_eq!(second["cache_status"], "hit");
    assert_eq!(second["checkout_egress"]["model_received_checkout"], false);
    assert_eq!(model.calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn no_inspect_overrides_investigation_configuration_and_performs_zero_checkout_discovery() {
    let mut fixture = Fixture::new();
    fixture.config.config.context.inspection.enabled = true;
    let model = FakeModel::new(Behavior::Good);
    let result = fixture
        .run(
            &model,
            RunOptions {
                inspect: true,
                investigate: true,
                no_inspect: true,
                no_cache: true,
                ..RunOptions::default()
            },
        )
        .await;
    assert_eq!(result["mode"], "intelligent");
    assert_eq!(result["inspection"]["status"], "disabled");
    assert!(result["inspection"]["root"].is_null());
    assert_eq!(result["inspection"]["budget"]["files_read"], 0);
    assert_eq!(result["inspection"]["budget"]["index_entries"], 0);
    assert_eq!(result["inspection"]["budget"]["bytes_read"], 0);
    assert_eq!(result["investigation"]["enabled"], false);
    assert!(fixture.cache_files().is_empty());
    assert!(
        model
            .inputs()
            .iter()
            .all(|input| input["local_observations"].as_array().unwrap().is_empty())
    );
}

#[tokio::test]
async fn discriminating_deeper_observation_changes_the_action_and_preserves_a_cited_trace() {
    let fixture = Fixture::new();
    let model = FakeModel::new(Behavior::Good);
    let result = fixture
        .run(
            &model,
            RunOptions {
                investigate: true,
                ..RunOptions::default()
            },
        )
        .await;
    assert_eq!(result["mode"], "intelligent", "{result:#}");
    assert_eq!(result["model_calls"], 4);
    let inputs = model.inputs();
    assert!(
        inputs[0]["local_observations"]
            .as_array()
            .unwrap()
            .iter()
            .all(|o| o["path"] != "src/dispatch/zz_policy.rs")
    );
    assert_eq!(inputs[1]["task"], "context_investigation");
    assert_eq!(inputs[2]["task"], "context_decision");
    assert!(!inputs[2]["previous_brief"].is_null());
    assert_eq!(
        result["investigation"]["steps"].as_array().unwrap().len(),
        1
    );
    let step = &result["investigation"]["steps"][0];
    assert_eq!(step["path"], "src/dispatch/zz_policy.rs");
    assert_eq!(step["recommendation_changed"], true);
    assert_ne!(
        step["previous_recommendation"],
        step["revised_recommendation"]
    );
    assert_eq!(step["counterevidence"][0]["material"], true);
    assert!(
        result["brief"]["preferred_approach"]["text"]
            .as_str()
            .unwrap()
            .starts_with("Route dispatch")
    );
    assert_eq!(fixture.source_snapshot()[1], QUEUE.as_bytes());
}

#[tokio::test]
async fn material_counterevidence_cannot_leave_the_original_action_in_place() {
    let fixture = Fixture::new();
    let model = FakeModel::new(Behavior::UnchangedCounterevidence);
    let result = fixture
        .run(
            &model,
            RunOptions {
                investigate: true,
                ..RunOptions::default()
            },
        )
        .await;
    assert_eq!(result["mode"], "fast_fallback");
    assert_eq!(
        result["investigation"]["stop_reason"],
        "counterevidence_not_addressed"
    );
    assert_eq!(result["model_calls"], 3);
    assert!(fixture.cache_files().is_empty());
}

#[tokio::test]
async fn an_invalid_plan_cannot_read_an_arbitrary_path_and_stops_with_the_validated_initial_brief()
{
    let fixture = Fixture::new();
    let model = FakeModel::new(Behavior::InvalidPlan);
    let result = fixture
        .run(
            &model,
            RunOptions {
                investigate: true,
                ..RunOptions::default()
            },
        )
        .await;
    assert_eq!(result["mode"], "intelligent");
    assert_eq!(
        result["investigation"]["stop_reason"],
        "invalid_or_unavailable_plan"
    );
    assert!(
        result["investigation"]["steps"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(result["model_calls"], 3);
    assert!(
        result["inspection"]["observations"]
            .as_array()
            .unwrap()
            .iter()
            .all(|o| o["path"] != "src/dispatch/zz_policy.rs")
    );
}

#[tokio::test]
async fn deeper_only_file_change_invalidates_cached_reasoning_even_with_restored_mtime_and_size() {
    let fixture = Fixture::new();
    let model = FakeModel::new(Behavior::Good);
    let options = RunOptions {
        investigate: true,
        ..RunOptions::default()
    };
    let first = fixture.run(&model, options.clone()).await;
    assert_eq!(first["mode"], "intelligent");
    let cached = fixture.run(&model, options.clone()).await;
    assert_eq!(cached["cache_status"], "hit");
    let path = fixture.dir.path().join("src/dispatch/zz_policy.rs");
    let before = fs::metadata(&path).unwrap();
    let replacement = DEEP.replace("false;", "true; ");
    assert_eq!(replacement.len(), DEEP.len());
    fs::write(&path, &replacement).unwrap();
    // Windows requires write access to restore timestamps. Open the existing
    // file without creation or truncation so this remains a same-size change.
    OpenOptions::new()
        .write(true)
        .open(&path)
        .unwrap()
        .set_times(FileTimes::new().set_modified(before.modified().unwrap()))
        .unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), replacement);
    assert_eq!(
        fs::metadata(&path).unwrap().modified().unwrap(),
        before.modified().unwrap()
    );
    let refreshed = fixture.run(&model, options).await;
    assert_eq!(refreshed["cache_status"], "miss");
    assert_ne!(refreshed["revision_key"], first["revision_key"]);
    assert_ne!(
        refreshed["brief"]["preferred_approach"],
        first["brief"]["preferred_approach"]
    );
}

#[tokio::test]
async fn adding_and_removing_a_relevant_file_invalidates_the_checkout_index_cache() {
    let fixture = Fixture::new();
    let model = FakeModel::new(Behavior::Good);
    let options = RunOptions {
        inspect: true,
        ..RunOptions::default()
    };
    let first = fixture.run(&model, options.clone()).await;
    assert_eq!(
        fixture.run(&model, options.clone()).await["cache_status"],
        "hit"
    );
    let path = fixture.dir.path().join("src/dispatch/new_allocator.rs");
    fs::write(&path, "pub fn dispatch_allocation_strategy() {}\n").unwrap();
    let added = fixture.run(&model, options.clone()).await;
    assert_eq!(added["cache_status"], "miss");
    assert_ne!(
        first["inspection"]["index_digest"],
        added["inspection"]["index_digest"]
    );
    fs::remove_file(path).unwrap();
    let removed = fixture.run(&model, options).await;
    assert_ne!(
        removed["inspection"]["index_digest"],
        added["inspection"]["index_digest"]
    );
    assert_ne!(removed["revision_key"], added["revision_key"]);
}

#[tokio::test]
async fn inspected_source_change_during_reasoning_discards_generated_advice() {
    let fixture = Fixture::new();
    let mut model = FakeModel::new(Behavior::MutateFile);
    model.mutation = Some(fixture.dir.path().join("src/dispatch/queue.rs"));
    let result = fixture
        .run(
            &model,
            RunOptions {
                inspect: true,
                ..RunOptions::default()
            },
        )
        .await;
    assert_eq!(result["mode"], "fast_fallback");
    assert_eq!(result["investigation"]["stop_reason"], "evidence_changed");
    assert!(
        result["inspection"]["observations"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(fixture.cache_files().is_empty());
}

#[tokio::test]
async fn source_mutation_after_investigation_discards_generated_trace_and_its_local_references() {
    let fixture = Fixture::new();
    let mut model = FakeModel::new(Behavior::MutateFile);
    model.mutation = Some(fixture.dir.path().join("src/dispatch/queue.rs"));
    let result = fixture
        .run(
            &model,
            RunOptions {
                investigate: true,
                ..RunOptions::default()
            },
        )
        .await;
    assert_eq!(result["mode"], "fast_fallback");
    assert_eq!(result["model_calls"], 4);
    assert_eq!(result["investigation"]["stop_reason"], "evidence_changed");
    assert!(
        result["inspection"]["observations"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        result["investigation"]["steps"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(all_references(&result).1.is_empty());
    assert!(fixture.cache_files().is_empty());
}

#[tokio::test]
async fn a_dangling_cached_investigation_reference_is_rejected_even_with_a_valid_cache_checksum() {
    let fixture = Fixture::new();
    let model = FakeModel::new(Behavior::Good);
    let options = RunOptions {
        investigate: true,
        ..RunOptions::default()
    };
    assert_eq!(
        fixture.run(&model, options.clone()).await["mode"],
        "intelligent"
    );
    let path = fixture.cache_files().pop().unwrap();
    let mut cached: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    let draft: lore::context::decision::DraftDecision =
        serde_json::from_value(cached["draft"].clone()).unwrap();
    let verification: lore::context::decision::DecisionVerification =
        serde_json::from_value(cached["verification"].clone()).unwrap();
    let inspection: lore::context::inspection::InspectionReport =
        serde_json::from_value(cached["inspection"].clone()).unwrap();
    let mut investigation: lore::context::investigation::InvestigationReport =
        serde_json::from_value(cached["investigation"].clone()).unwrap();
    investigation.steps[0].observation_ids = vec!["co_missing_observation".into()];
    cached["content_hash"] = json!(
        util::json_digest(&(
            cached["key"].as_str().unwrap(),
            cached["revision_key"].as_str().unwrap(),
            &draft,
            &verification,
            &inspection,
            &investigation,
        ))
        .unwrap()
    );
    cached["investigation"] = serde_json::to_value(investigation).unwrap();
    fs::write(path, serde_json::to_vec(&cached).unwrap()).unwrap();
    *model.behavior.lock().unwrap() = Behavior::InvalidJson;
    let result = fixture.run(&model, options).await;
    assert_eq!(result["mode"], "fast_fallback");
    assert_eq!(
        result["model_calls"], 1,
        "the invalid cache must trigger a fresh inference attempt"
    );
    assert!(
        result["investigation"]["steps"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        !serde_json::to_string(&result)
            .unwrap()
            .contains("co_missing_observation")
    );
}

#[tokio::test]
async fn openai_loopback_proxy_location_does_not_authorize_checkout_egress() {
    let mut fixture = Fixture::new();
    fixture.config.config.privacy.local_only = false;
    let mut model = FakeModel::hosted(Behavior::Good);
    model.descriptor.location = ExecutionLocation::Local;
    let result = fixture
        .run(
            &model,
            RunOptions {
                investigate: true,
                ..RunOptions::default()
            },
        )
        .await;
    assert_eq!(result["mode"], "intelligent");
    assert_eq!(result["model_calls"], 2);
    assert_eq!(result["checkout_egress"]["allowed"], false);
    assert_eq!(result["checkout_egress"]["model_received_checkout"], false);
    assert_eq!(
        result["investigation"]["stop_reason"],
        "checkout_egress_withheld"
    );
    for request in model.requests.lock().unwrap().iter() {
        let wire = format!(
            "{}{}{}",
            request.instructions,
            request.input,
            request.schema.as_ref().unwrap()
        );
        assert!(!wire.contains("checkout_only_marker_9173"));
        assert!(!wire.contains("src/dispatch/zz_policy.rs"));
        assert!(!wire.contains("tests/dispatch_queue.rs"));
    }
}

#[tokio::test]
async fn local_only_policy_rejects_cloud_tagged_ollama_even_with_checkout_permission() {
    for tag in ["fixture:cloud", "fixture-cloud"] {
        let fixture = Fixture::new();
        let mut model = FakeModel::new(Behavior::Good);
        model.descriptor.model = tag.into();
        let result = fixture
            .run(
                &model,
                RunOptions {
                    investigate: true,
                    allow_checkout_egress: true,
                    ..RunOptions::default()
                },
            )
            .await;
        assert_eq!(result["mode"], "fast_fallback");
        assert_eq!(result["model_calls"], 0);
        assert_eq!(model.calls.load(Ordering::SeqCst), 0);
        assert_eq!(result["checkout_egress"]["allowed"], false);
        assert_eq!(result["checkout_egress"]["model_received_checkout"], false);
        assert!(fixture.cache_files().is_empty());
    }
}

#[tokio::test]
async fn final_support_check_must_cover_inspected_observation_references() {
    let fixture = Fixture::new();
    let model = FakeModel::new(Behavior::MissingObservationReview);
    let result = fixture
        .run(
            &model,
            RunOptions {
                inspect: true,
                ..RunOptions::default()
            },
        )
        .await;
    assert_eq!(result["mode"], "fast_fallback");
    assert_eq!(
        result["investigation"]["stop_reason"],
        "support_check_failed"
    );
    assert!(fixture.cache_files().is_empty());
}

#[tokio::test]
async fn one_call_budget_never_skips_the_mandatory_support_check_and_deadline_is_shared() {
    let mut fixture = Fixture::new();
    fixture.config.config.context.investigation.max_model_calls = 1;
    let model = FakeModel::new(Behavior::Good);
    let bounded = fixture
        .run(
            &model,
            RunOptions {
                investigate: true,
                ..RunOptions::default()
            },
        )
        .await;
    assert_eq!(bounded["mode"], "fast_fallback");
    assert_eq!(bounded["model_calls"], 1);
    assert_eq!(model.calls.load(Ordering::SeqCst), 1);
    assert!(fixture.cache_files().is_empty());
    fixture.config.config.context.investigation.max_model_calls = 6;
    fixture.config.config.context.investigation.timeout_seconds = 1;
    let model = FakeModel::new(Behavior::Slow);
    let started = Instant::now();
    let expired = fixture.run(&model, RunOptions::default()).await;
    assert_eq!(expired["mode"], "fast_fallback");
    assert_eq!(expired["model_calls"], 1);
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[tokio::test]
async fn output_budget_preserves_complete_citations_and_honest_fallback() {
    let mut fixture = Fixture::new();
    fixture.options.max_tokens = 1_500;
    let model = FakeModel::new(Behavior::Good);
    let result = fixture
        .run(
            &model,
            RunOptions {
                inspect: true,
                no_cache: true,
                ..RunOptions::default()
            },
        )
        .await;
    assert_eq!(result["schema_version"], 4);
    assert!(result["budget"]["used_tokens"].as_u64().unwrap() <= 1_500);
    assert!(result["mode"] == "intelligent" || result["fallback_reason"].is_string());
}

#[tokio::test]
async fn inline_only_local_citation_remains_manifested_when_output_budget_prunes_other_items() {
    let mut fixture = Fixture::new();
    fixture.options.max_tokens = 3_500;
    let model = FakeModel::new(Behavior::InlineObservation);
    let result = fixture
        .run(
            &model,
            RunOptions {
                inspect: true,
                no_cache: true,
                ..RunOptions::default()
            },
        )
        .await;
    assert_eq!(result["mode"], "intelligent", "{result:#}");
    assert!(
        result["brief_items_omitted"].as_u64().unwrap() > 0,
        "fixture must actually trigger pruning"
    );
    let observation = result["inspection"]["observations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|observation| observation["path"] == "tests/dispatch_queue.rs")
        .unwrap();
    let id = observation["id"].as_str().unwrap();
    assert!(
        result["brief"]["preferred_approach"]["text"]
            .as_str()
            .unwrap()
            .contains(id)
    );
    assert!(
        !result["brief"]["preferred_approach"]["observation_ids"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value == id),
        "this must remain an inline-only reference"
    );
    assert!(
        model.inputs().last().unwrap()["brief"]["preferred_approach"]["text"]
            .as_str()
            .unwrap()
            .contains(id)
    );
}
