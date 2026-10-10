//! ON-01–ON-24 contract fixtures. Model doubles establish provenance, privacy
//! and UX behavior, not real-model quality or an actual human learning gain.

use lore::{
    config::{Config, ResolvedConfig},
    context::{
        self, ContextOptions, adaptive,
        decision::{
            GenerationBasis,
            runtime::{DecisionContextResult, RunOptions},
        },
    },
    domain::{AssertionProposal, SourceMaterial},
    experience::{
        self, Claim, ClaimBasis, Concept, ExperienceMode, ExperienceOptions, FeedbackBasis,
        FeedbackOutcome, LearningGraph, LearningStage, Prerequisite,
    },
    inference::{
        ExecutionLocation, GenerationRequest, GenerationResponse, GenerativeModel, ModelDescriptor,
        ModelFuture, Provider,
    },
    sources::{self, Document},
    storage, util,
};
use rusqlite::Connection;
use serde_json::{Value, json};
use std::{
    fs,
    path::PathBuf,
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

const PURPOSE: &str = "Harbor dispatches tenant jobs through bounded queues so that one tenant cannot exhaust all workers.";
const FLOW: &str = "Dispatch receives a tenant job, checks the tenant key, enqueues it, and a worker processes it. A full queue returns Busy without accepting the job.";
const RULE: &str = "Production dispatch preserves per-tenant ordering and bounds the queue at 128 jobs; staging uses 64 jobs. A full queue must return Busy without accepting the job.";
const CODE: &str = "// checkout_marker_dispatch_9173\npub const CAPACITY: usize = 128;\npub fn dispatch_job() {}\n";

struct Fixture {
    dir: tempfile::TempDir,
    config: ResolvedConfig,
    conn: Connection,
}

impl Fixture {
    fn new() -> Self {
        let fixture = Self::empty();
        fixture.add("purpose", "design", PURPOSE);
        fixture.add("workflow", "procedure", FLOW);
        fixture.add("capacity", "constraint", RULE);
        fixture
    }

    fn empty() -> Self {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("docs")).unwrap();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::create_dir_all(dir.path().join(".git")).unwrap();
        fs::write(dir.path().join("src/dispatch.rs"), CODE).unwrap();
        let mut config = Config::default();
        config.project.name = "Harbor".into();
        config.context.cache = false;
        config.context.inspection.max_elapsed_ms = 5_000;
        let config_path = dir.path().join("lore.yml");
        fs::write(&config_path, serde_yaml::to_string(&config).unwrap()).unwrap();
        let config = ResolvedConfig::load(&config_path).unwrap();
        let conn = Connection::open_in_memory().unwrap();
        storage::migrate(&conn).unwrap();
        conn.execute("INSERT INTO projects VALUES('p','Harbor','2026-10-09')", [])
            .unwrap();
        conn.execute(
            "INSERT INTO source_roots(id,project_id,configured_path) VALUES('docs','p',?1)",
            [dir.path().join("docs").to_string_lossy().as_ref()],
        )
        .unwrap();
        Self { dir, config, conn }
    }

    fn add(&self, subject: &str, kind: &str, text: &str) {
        self.add_with_lifecycle(subject, kind, text, "accepted");
    }

    fn add_with_lifecycle(&self, subject: &str, kind: &str, text: &str, lifecycle: &str) {
        let relative = format!("{subject}.md");
        let path = self.dir.path().join("docs").join(&relative);
        fs::write(&path, text).unwrap();
        let document = Document {
            root_id: "docs".into(),
            root_path: self.dir.path().join("docs"),
            material: SourceMaterial::Primary,
            origin: None,
            relative_path: relative.clone(),
            physical_path: path,
            text: text.into(),
            digest: util::digest(text),
            chunks: sources::split_markdown(text, "docs", &relative, 8_000).unwrap(),
        };
        let proposal = AssertionProposal {
            topic: "dispatch".into(),
            topic_title: "Dispatch".into(),
            subject: format!("dispatch {subject}"),
            statement: text.into(),
            kind: kind.into(),
            lifecycle: lifecycle.into(),
            scope: "production and staging".into(),
            effective_at: String::new(),
            quote: text.into(),
        };
        let (source, revision) = storage::begin_source(&self.conn, &document, None).unwrap();
        let chunk = &document.chunks[0];
        let (section, section_revision) =
            storage::begin_section(&self.conn, &source, &revision, chunk).unwrap();
        let (assertion, _) = storage::capture_assertion(
            &self.conn,
            storage::AssertionCapture {
                document: &document,
                chunk,
                proposal: &proposal,
                source: &source,
                source_revision: &revision,
                section: &section,
                section_revision: &section_revision,
                model: "fixture",
            },
        )
        .unwrap();
        storage::create_unit(&self.conn, "p", &assertion, &proposal).unwrap();
        storage::refresh_knowledge(&self.conn, "p").unwrap();
    }

    fn options(&self) -> ExperienceOptions {
        ExperienceOptions {
            goal: Some("Understand dispatch tenant jobs queues".into()),
            max_tokens: 18_000,
            ..Default::default()
        }
    }

    async fn shared(&self, run: &RunOptions) -> adaptive::AdaptiveResult {
        self.shared_task(run, "Understand dispatch tenant jobs queues")
            .await
    }

    async fn shared_task(&self, run: &RunOptions, task: &str) -> adaptive::AdaptiveResult {
        let options = ContextOptions {
            task: task.into(),
            paths: vec!["src/dispatch.rs".into()],
            max_tokens: 11_000,
        };
        let selected = context::build_context(&self.conn, &options).unwrap();
        adaptive::build(&self.conn, &self.config, &options, selected, None, run)
            .await
            .unwrap()
    }

    async fn experience(
        &self,
        options: &ExperienceOptions,
        model: Option<&dyn GenerativeModel>,
    ) -> experience::ExperienceResult {
        let task = options.task.as_ref().or(options.goal.as_ref()).unwrap();
        experience::build(
            &self.conn,
            &self.config,
            options,
            self.shared_task(&options.run, task).await,
            model,
        )
        .await
        .unwrap()
    }
}

#[derive(Clone, Copy)]
enum Behavior {
    Valid,
    UnknownEvidence,
    InventedFile,
    RuntimeClaim,
    Reject,
    MutateCode,
}

struct HumanModel {
    descriptor: ModelDescriptor,
    behavior: Behavior,
    calls: AtomicUsize,
    inputs: Mutex<Vec<Value>>,
    mutation: Option<PathBuf>,
}

impl HumanModel {
    fn new(behavior: Behavior) -> Self {
        Self {
            descriptor: ModelDescriptor {
                provider: Provider::Ollama,
                model: "human-fixture".into(),
                location: ExecutionLocation::Local,
            },
            behavior,
            calls: AtomicUsize::new(0),
            inputs: Mutex::new(Vec::new()),
            mutation: None,
        }
    }
}

fn claim(text: &str, evidence: &str) -> Value {
    json!({"text":text,"basis":"documentary","evidence_ids":[evidence],"observation_ids":[],"qualifications":[]})
}

fn draft(input: &Value) -> Value {
    let records = input["evidence"]["records"].as_array().unwrap();
    let record = |kind: &str| {
        records
            .iter()
            .find(|r| r["kind"] == kind && (kind != "constraint" || r["claim"]["text"] == RULE))
            .unwrap()
    };
    let design = record("design");
    let rule = record("constraint");
    let workflow = record("procedure");
    let evidence = |record: &Value| {
        record["claim"]["evidence_ids"][0]
            .as_str()
            .unwrap()
            .to_string()
    };
    let rule_id = evidence(rule);
    let flow_id = evidence(workflow);
    let tutorial = if input["mode"] == "tutorial" {
        let activity = |title: &str, kind: &str, objective: &str, task: &str| {
            let mut starting = claim(
                "Practice a hypothetical tenant request at the queue boundary.",
                &rule_id,
            );
            starting["basis"] = json!("hypothetical");
            json!({"title":title,"kind":kind,"objective":objective,"task":task,
                "starting_point":starting,"concept_ids":["queue"],
                "hints":[claim("Consider the environment before choosing the capacity.", &rule_id),
                    claim("A rejected request does not enter the queue.", &rule_id),claim(RULE, &rule_id)],
                "expected":[claim(RULE, &rule_id)],"solution":claim(RULE, &rule_id)})
        };
        json!({"prerequisites":[{"before":"dispatch","after":"queue",
            "rationale":{"text":"Understanding a tenant request can help explain its queue boundary.",
            "basis":"inference","evidence_ids":[flow_id],"observation_ids":[],"qualifications":[]}}],
            "primary":activity("Predict an overflow","predict","Predict the full-queue result.",
                "A production queue already contains 128 jobs. Predict the result of submitting another job."),
            "transfer":activity("Explain the staging boundary","explain","Apply the condition in another environment.",
                "A staging queue contains 64 jobs. Explain how a diagnostic-only change would preserve rejection and ordering.")})
    } else {
        Value::Null
    };
    json!({"purpose":claim(PURPOSE, &evidence(design)),"rationale":Value::Null,
        "concepts":[{"id":"dispatch","title":"Tenant dispatch","description":claim(PURPOSE, &evidence(design))},
            {"id":"queue","title":"Bounded queue","description":claim(RULE, &rule_id)}],
        "architecture":[claim("A worker processes each accepted tenant job.", &flow_id)],
        "workflow":{"title":"Dispatch a tenant job","sequence_supported":true,
            "stops":[{"title":"Receive the job","role":"entry","description":claim("Dispatch receives a tenant job and checks the tenant key.", &flow_id)},
                {"title":"Process accepted work","role":"outcome","description":claim("The job is enqueued and a worker processes it.", &flow_id)},
                {"title":"Reject overflow","role":"failure","description":claim("A full queue returns Busy without accepting the job.", &flow_id)}],
            "limitations":["The workflow is documented; no runtime execution was observed."]},
        "next_exploration":[{"label":"Explore the queue boundary","goal":"Explain dispatch queue capacity",
            "mode":"explanation","evidence_ids":[rule_id],"observation_ids":[]}],
        "gaps":[],"tutorial":tutorial})
}

impl GenerativeModel for HumanModel {
    fn descriptor(&self) -> &ModelDescriptor {
        &self.descriptor
    }
    fn generate<'a>(
        &'a self,
        request: &'a GenerationRequest,
    ) -> ModelFuture<'a, GenerationResponse> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let input: Value = serde_json::from_str(&request.input).unwrap();
            self.inputs.lock().unwrap().push(input.clone());
            let output = if input["task"] == "human_experience" {
                let mut output = draft(&input);
                match self.behavior {
                    Behavior::UnknownEvidence => {
                        output["purpose"]["evidence_ids"] = json!(["ev_not_retained"])
                    }
                    Behavior::InventedFile => {
                        output["purpose"]["text"] =
                            json!("Dispatch begins at src/invented_handler.rs.")
                    }
                    Behavior::RuntimeClaim => {
                        output["purpose"]["text"] =
                            json!("The tests passed and you have mastered dispatch.")
                    }
                    Behavior::MutateCode => fs::write(
                        self.mutation.as_ref().unwrap(),
                        "pub const CAPACITY: usize = 7;\n",
                    )
                    .unwrap(),
                    _ => {}
                }
                output
            } else {
                assert_eq!(input["task"], "human_experience_verification");
                let feedback = if input["learner_answer"].is_null() {
                    Value::Null
                } else {
                    let rule = input["evidence"]["records"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|r| r["kind"] == "constraint")
                        .unwrap();
                    json!({"basis":"model_assessed","outcome":"needs_revision",
                        "message":"Include the separate staging boundary in your explanation.",
                        "comparison_points":[rule["claim"]],"hints_requested":input["hints_requested"],
                        "limitations":"Model-assessed feedback is fallible and is not independent verification."})
                };
                json!({"supported":!matches!(self.behavior, Behavior::Reject),
                    "evidence_ids":input["expected_evidence_ids"],"observation_ids":input["expected_observation_ids"],
                    "issues":[],"feedback":feedback})
            };
            Ok(GenerationResponse {
                usage: None,
                model: "human-fixture".into(),
                text: output.to_string(),
            })
        })
    }
}

#[tokio::test]
async fn on01_immediate_orientation_is_shared_source_extractive_and_stateless() {
    let fixture = Fixture::new();
    let before = storage::registry_revision(&fixture.conn).unwrap();
    let result = fixture.experience(&fixture.options(), None).await;
    assert_eq!(result.schema_version, 1);
    assert_eq!(result.orientation.purpose.unwrap().text, PURPOSE);
    assert!(
        result
            .orientation
            .constraints
            .iter()
            .any(|c| c.text == RULE)
    );
    assert_eq!(
        result.orientation.workflow.unwrap().stops[0]
            .description
            .text,
        FLOW
    );
    assert!(result.tutorial.is_none());
    assert!(result.feedback.is_none());
    assert_eq!(result.model_calls, 0);
    assert_eq!(before, storage::registry_revision(&fixture.conn).unwrap());
    assert!(!fixture.config.state.exists());
}

#[tokio::test]
async fn on03_source_checked_orientation_preserves_conditions_and_resolves_all_ids() {
    let fixture = Fixture::new();
    let model = HumanModel::new(Behavior::Valid);
    let result = fixture.experience(&fixture.options(), Some(&model)).await;
    assert!(matches!(
        result.generation_basis,
        GenerationBasis::ModelAssessed
    ));
    assert_eq!(result.presentation_model_calls, 2);
    assert_eq!(result.model_calls, 2);
    assert_eq!(result.orientation.constraints[0].text, RULE);
    let markdown = experience::render_markdown(&result);
    assert!(markdown.contains("staging uses 64 jobs"));
    assert!(markdown.contains("Follow a workflow"));
    assert!(!markdown.contains("Requested hints"));
    assert!(
        context::count_tokens(&serde_json::to_string(&result).unwrap()) <= result.budget.max_tokens
    );
    assert!(context::count_tokens(&markdown) <= result.budget.max_tokens);
    let DecisionContextResult::Brief(shared) = &result.intelligence else {
        panic!("brief")
    };
    for evidence in &shared.evidence {
        let archived = storage::evidence_snapshot(&fixture.conn, &evidence.id).unwrap();
        assert_eq!(archived.digest, evidence.content_hash);
        assert_eq!(archived.source_revision_id, evidence.revision_id);
    }
}

#[tokio::test]
async fn on05_invalid_files_evidence_execution_and_failed_support_degrade_to_sources() {
    let fixture = Fixture::new();
    for behavior in [
        Behavior::UnknownEvidence,
        Behavior::InventedFile,
        Behavior::RuntimeClaim,
        Behavior::Reject,
    ] {
        let model = HumanModel::new(behavior);
        let result = fixture.experience(&fixture.options(), Some(&model)).await;
        assert!(matches!(
            result.generation_basis,
            GenerationBasis::DeterministicFallback
        ));
        let markdown = experience::render_markdown(&result);
        assert!(markdown.contains("staging uses 64 jobs"));
        assert!(!markdown.contains("invented_handler"));
        assert!(!markdown.contains("have mastered"));
        assert!(!markdown.contains("ev_not_retained"));
    }
}

#[tokio::test]
async fn on10_tutorial_has_purposeful_activity_optional_hints_and_distinct_transfer() {
    let fixture = Fixture::new();
    let model = HumanModel::new(Behavior::Valid);
    let mut options = fixture.options();
    options.mode = ExperienceMode::Tutorial;
    let result = fixture.experience(&options, Some(&model)).await;
    let path = result.tutorial.unwrap();
    assert_ne!(path.primary.task, path.transfer.task);
    assert_ne!(path.primary.kind, path.transfer.kind);
    assert!(path.primary.hints.is_empty());
    assert!(path.primary.solution.is_none());
    assert!(path.transfer.solution.is_none());
    assert!(path.transfer.expected.is_empty());
    assert_eq!(path.primary.provenance, "generated_practice");
    assert_eq!(path.primary.completion_status, "not_assessed");
    assert!(!path.prerequisites.mandatory);
    options.hint_level = 2;
    options.activity = LearningStage::Transfer;
    let hinted = fixture
        .experience(&options, Some(&model))
        .await
        .tutorial
        .unwrap();
    assert!(hinted.primary.hints.is_empty());
    assert_eq!(hinted.transfer.hints.len(), 2);
    assert!(hinted.transfer.solution.is_none());
    options.show_solution = true;
    let revealed = fixture
        .experience(&options, Some(&model))
        .await
        .tutorial
        .unwrap();
    assert!(revealed.primary.solution.is_none());
    assert!(revealed.transfer.solution.is_some());
}

#[tokio::test]
async fn on18_answer_is_not_in_shared_task_or_persisted_and_feedback_is_fallible() {
    let fixture = Fixture::new();
    let model = HumanModel::new(Behavior::Valid);
    let mut options = fixture.options();
    options.mode = ExperienceMode::Tutorial;
    options.lesson = Some(
        fixture
            .experience(&options, Some(&model))
            .await
            .tutorial
            .unwrap()
            .revision_key,
    );
    options.answer = Some("private_learner_answer_8901 all environments accept 128 jobs".into());
    let result = fixture.experience(&options, Some(&model)).await;
    let json = serde_json::to_string(&result).unwrap();
    assert!(!json.contains("private_learner_answer_8901"));
    let feedback = result.feedback.unwrap();
    assert_eq!(feedback.basis, FeedbackBasis::ModelAssessed);
    assert_eq!(feedback.outcome, FeedbackOutcome::NeedsRevision);
    assert!(feedback.limitations.contains("fallible"));
    let inputs = model.inputs.lock().unwrap();
    assert!(
        !inputs[0]
            .to_string()
            .contains("private_learner_answer_8901")
    );
    assert!(
        inputs
            .last()
            .unwrap()
            .to_string()
            .contains("private_learner_answer_8901")
    );
    assert!(!fixture.config.state.exists());
}

#[tokio::test]
async fn on12_first_task_bypasses_curriculum_and_preserves_supplied_intent() {
    let fixture = Fixture::new();
    let mut options = fixture.options();
    options.task = Some("Refactor dispatch without changing queue behavior".into());
    let result = fixture.experience(&options, None).await;
    assert_eq!(result.mode, ExperienceMode::HowTo);
    assert!(result.tutorial.is_none());
    let task = result.first_task.unwrap();
    assert_eq!(task.task, options.task.unwrap());
    assert_eq!(task.provenance, "user_supplied_task");
    assert!(task.qualification.contains("not checks Lore executed"));
}

#[tokio::test]
async fn on11_reference_is_exact_and_does_not_call_presentation_model() {
    let fixture = Fixture::new();
    let model = HumanModel::new(Behavior::Valid);
    let mut options = fixture.options();
    options.mode = ExperienceMode::Reference;
    let result = fixture.experience(&options, Some(&model)).await;
    assert_eq!(model.calls.load(Ordering::SeqCst), 0);
    assert_eq!(result.model_calls, 0);
    assert!(result.tutorial.is_none());
    assert!(result.references.iter().any(|r| r.excerpt == RULE));
}

#[tokio::test]
async fn on24_offline_tutorial_provides_real_activity_and_honest_source_comparison() {
    let fixture = Fixture::new();
    let mut options = fixture.options();
    options.mode = ExperienceMode::Tutorial;
    options.answer = Some("I would keep the same queue boundary".into());
    let result = fixture.experience(&options, None).await;
    let path = result.tutorial.unwrap();
    assert_ne!(path.primary.task, path.transfer.task);
    assert!(path.primary.task.contains("Predict"));
    assert!(path.primary.solution.is_none());
    let feedback = result.feedback.unwrap();
    assert_eq!(feedback.basis, FeedbackBasis::SourceComparison);
    assert_eq!(feedback.outcome, FeedbackOutcome::ReviewNeeded);
}

#[tokio::test]
async fn human_model_cannot_expand_host_egress_grants() {
    let mut fixture = Fixture::new();
    fixture.config.config.privacy.local_only = false;
    fixture.config.config.privacy.allow_checkout_egress = true;
    let mut model = HumanModel::new(Behavior::Valid);
    model.descriptor.provider = Provider::OpenAi;
    model.descriptor.location = ExecutionLocation::Hosted;
    let result = fixture.experience(&fixture.options(), Some(&model)).await;
    assert_eq!(model.calls.load(Ordering::SeqCst), 0);
    assert_eq!(result.presentation_status, "presentation_egress_denied");
    assert!(!result.capabilities.hosted_egress);
}

#[tokio::test]
async fn on21_code_mutation_during_generation_discards_stale_static_premises() {
    let fixture = Fixture::new();
    let mut model = HumanModel::new(Behavior::MutateCode);
    model.mutation = Some(fixture.dir.path().join("src/dispatch.rs"));
    let mut options = fixture.options();
    options.run.inspect = true;
    let shared = fixture.shared(&options.run).await;
    let DecisionContextResult::Brief(before) = &shared.intelligence else {
        panic!("brief")
    };
    assert!(!before.inspection.observations.is_empty());
    let result = experience::build(
        &fixture.conn,
        &fixture.config,
        &options,
        shared,
        Some(&model),
    )
    .await
    .unwrap();
    assert_eq!(
        result.presentation_status,
        "documentary_fallback_after_revalidation"
    );
    assert!(matches!(
        result.generation_basis,
        GenerationBasis::DeterministicFallback
    ));
    assert_eq!(result.model_calls, 2);
    let DecisionContextResult::Brief(after) = &result.intelligence else {
        panic!("brief")
    };
    assert!(after.inspection.observations.is_empty());
    assert!(after.brief.implementation_seams.is_empty());
    assert!(
        result
            .orientation
            .constraints
            .iter()
            .any(|c| c.text == RULE)
    );
}

#[tokio::test]
async fn on21_new_source_invalidates_an_older_shared_snapshot() {
    let fixture = Fixture::new();
    let shared = fixture.shared(&RunOptions::default()).await;
    fixture.add(
        "exception",
        "constraint",
        "Dispatch maintenance mode rejects every new tenant job.",
    );
    let outcome = experience::build(
        &fixture.conn,
        &fixture.config,
        &fixture.options(),
        shared,
        None,
    )
    .await;
    assert!(
        outcome
            .unwrap_err()
            .to_string()
            .contains("same project and registry snapshot")
    );
}

fn concept(id: &str) -> Concept {
    Concept {
        id: id.into(),
        title: id.into(),
        description: Claim {
            text: "A test concept".into(),
            basis: ClaimBasis::Inference,
            evidence_ids: vec!["ev_fixture".into()],
            observation_ids: Vec::new(),
            qualifications: Vec::new(),
        },
    }
}

#[test]
fn on08_prerequisite_cycles_are_exact_colearned_groups_without_mandatory_gates() {
    let concepts = [concept("a"), concept("b"), concept("c"), concept("d")];
    let edge = |before: &str, after: &str| Prerequisite {
        before: before.into(),
        after: after.into(),
        rationale: concepts[0].description.clone(),
    };
    let graph = LearningGraph::build(
        &concepts,
        vec![
            edge("a", "b"),
            edge("b", "a"),
            edge("b", "c"),
            edge("d", "d"),
        ],
    )
    .unwrap();
    assert_eq!(
        graph.co_learned,
        vec![
            vec!["a".to_string(), "b".to_string()],
            vec!["d".to_string()]
        ]
    );
    assert!(
        graph
            .suggested_order
            .iter()
            .position(|g| g.contains(&"a".to_string()))
            .unwrap()
            < graph
                .suggested_order
                .iter()
                .position(|g| g == &["c".to_string()])
                .unwrap()
    );
    assert!(!graph.mandatory);
    assert!(LearningGraph::build(&concepts, vec![edge("a", "missing")]).is_err());
}

#[test]
fn ordinary_intent_does_not_trigger_quizzes_and_learning_overrides_are_explicit() {
    let mode = |goal: &str| {
        experience::resolve_mode(&ExperienceOptions {
            goal: Some(goal.into()),
            ..Default::default()
        })
    };
    assert_eq!(
        mode("Explain dispatch retries"),
        ExperienceMode::Explanation
    );
    assert_eq!(
        mode("What value is the default capacity?"),
        ExperienceMode::Reference
    );
    assert_eq!(mode("Teach me dispatch queues"), ExperienceMode::Tutorial);
    assert_eq!(mode("Refactor dispatch queues"), ExperienceMode::HowTo);
}

#[tokio::test]
async fn verified_lesson_cache_reuses_hints_without_retaining_answers_and_invalidates_on_new_evidence()
 {
    let mut fixture = Fixture::new();
    fixture.config.config.context.cache = true;
    let model = HumanModel::new(Behavior::Valid);
    let mut options = fixture.options();
    options.mode = ExperienceMode::Tutorial;
    let initial = fixture.experience(&options, Some(&model)).await;
    let lesson = initial.tutorial.as_ref().unwrap().revision_key.clone();
    assert_eq!(model.calls.load(Ordering::SeqCst), 2);
    options.hint_level = 1;
    options.lesson = Some(lesson.clone());
    let hinted = fixture.experience(&options, Some(&model)).await;
    assert_eq!(hinted.presentation_status, "verified_cache_reuse");
    assert_eq!(hinted.presentation_model_calls, 0);
    assert_eq!(hinted.tutorial.as_ref().unwrap().revision_key, lesson);
    assert_eq!(model.calls.load(Ordering::SeqCst), 2);
    options.answer = Some("learner_private_marker_cache_4982 preserves ordering".into());
    let feedback = fixture.experience(&options, Some(&model)).await;
    assert_eq!(feedback.presentation_model_calls, 1);
    assert_eq!(
        feedback.feedback.as_ref().unwrap().basis,
        FeedbackBasis::ModelAssessed
    );
    for entry in fs::read_dir(fixture.config.state.join("human-view-cache")).unwrap() {
        let raw = fs::read_to_string(entry.unwrap().path()).unwrap();
        assert!(!raw.contains("learner_private_marker_cache_4982"));
        assert!(!raw.contains("\"feedback\""));
    }
    fixture.add(
        "maintenance",
        "constraint",
        "Dispatch maintenance mode rejects every new tenant job.",
    );
    let changed = fixture.experience(&options, Some(&model)).await;
    assert_ne!(initial.snapshot, changed.snapshot);
    assert_eq!(changed.presentation_model_calls, 2);
    assert_eq!(
        changed.feedback.unwrap().basis,
        FeedbackBasis::SourceComparison
    );
    assert!(
        changed
            .warnings
            .iter()
            .any(|w| w.contains("lesson revision does not match"))
    );
    assert_eq!(model.calls.load(Ordering::SeqCst), 5);
}

#[tokio::test]
async fn no_cache_bypasses_verified_lesson_reuse_and_an_unpinned_answer_is_not_graded() {
    let mut fixture = Fixture::new();
    fixture.config.config.context.cache = true;
    let model = HumanModel::new(Behavior::Valid);
    let mut options = fixture.options();
    options.mode = ExperienceMode::Tutorial;
    fixture.experience(&options, Some(&model)).await;
    options.run.no_cache = true;
    options.answer = Some("unpinned_private_answer_7653".into());
    let result = fixture.experience(&options, Some(&model)).await;
    assert_eq!(result.presentation_model_calls, 2);
    assert_eq!(
        result.feedback.unwrap().basis,
        FeedbackBasis::SourceComparison
    );
    assert!(
        model
            .inputs
            .lock()
            .unwrap()
            .iter()
            .all(|input| !input.to_string().contains("unpinned_private_answer_7653"))
    );
}

#[tokio::test]
async fn default_output_budget_keeps_a_complete_learning_activity_and_exact_constraint() {
    let fixture = Fixture::new();
    let model = HumanModel::new(Behavior::Valid);
    let mut options = fixture.options();
    options.max_tokens = experience::DEFAULT_MAX_TOKENS;
    options.mode = ExperienceMode::Tutorial;
    let result = fixture.experience(&options, Some(&model)).await;
    assert!(
        result.tutorial.is_some(),
        "{}",
        experience::render_markdown(&result)
    );
    assert!(
        result
            .orientation
            .constraints
            .iter()
            .any(|c| c.text == RULE)
    );
    assert!(result.budget.used_tokens <= experience::DEFAULT_MAX_TOKENS);
}

struct PendingModel {
    descriptor: ModelDescriptor,
    calls: AtomicUsize,
}

impl GenerativeModel for PendingModel {
    fn descriptor(&self) -> &ModelDescriptor {
        &self.descriptor
    }
    fn generate<'a>(&'a self, _: &'a GenerationRequest) -> ModelFuture<'a, GenerationResponse> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Box::pin(std::future::pending())
    }
}

#[tokio::test]
async fn cancelled_presentation_releases_read_snapshot_and_schedules_no_followup() {
    let fixture = Fixture::new();
    let model = PendingModel {
        descriptor: ModelDescriptor {
            provider: Provider::Ollama,
            model: "pending-fixture".into(),
            location: ExecutionLocation::Local,
        },
        calls: AtomicUsize::new(0),
    };
    let shared = fixture.shared(&RunOptions::default()).await;
    let options = fixture.options();
    let outcome = tokio::time::timeout(
        std::time::Duration::from_millis(50),
        experience::build(
            &fixture.conn,
            &fixture.config,
            &options,
            shared,
            Some(&model),
        ),
    )
    .await;
    assert!(outcome.is_err());
    assert_eq!(model.calls.load(Ordering::SeqCst), 1);
    assert!(fixture.conn.is_autocommit());
    assert!(!fixture.config.state.exists());
}

#[tokio::test]
async fn unchanged_checkout_is_rehashed_after_presentation_and_preserves_observations() {
    let fixture = Fixture::new();
    let model = HumanModel::new(Behavior::Valid);
    let mut options = fixture.options();
    options.run.inspect = true;
    let result = fixture.experience(&options, Some(&model)).await;
    assert_eq!(result.presentation_revalidation.status, "validated");
    assert!(result.presentation_revalidation.files_rehashed > 0);
    let DecisionContextResult::Brief(shared) = result.intelligence else {
        panic!("brief")
    };
    assert!(!shared.inspection.observations.is_empty());
    assert!(
        result.presentation_revalidation.bytes_read + shared.inspection.budget.bytes_read
            <= fixture.config.config.context.inspection.max_total_bytes
    );
}

#[tokio::test]
async fn a_narrower_request_cannot_reuse_shared_checkout_content_from_an_older_grant() {
    let fixture = Fixture::new();
    let model = HumanModel::new(Behavior::Valid);
    let shared = fixture
        .shared(&RunOptions {
            inspect: true,
            ..Default::default()
        })
        .await;
    let mut options = fixture.options();
    options.run.no_inspect = true;
    let outcome = experience::build(
        &fixture.conn,
        &fixture.config,
        &options,
        shared,
        Some(&model),
    )
    .await;
    assert!(
        outcome
            .unwrap_err()
            .to_string()
            .contains("current capability envelope")
    );
    assert_eq!(model.calls.load(Ordering::SeqCst), 0);
    assert!(fixture.conn.is_autocommit());
}

#[tokio::test]
async fn moving_the_allowed_root_cannot_leak_previous_observations_to_a_model() {
    let mut fixture = Fixture::new();
    let model = HumanModel::new(Behavior::Valid);
    let mut options = fixture.options();
    options.run.inspect = true;
    let shared = fixture.shared(&options.run).await;
    fixture.config.config.context.inspection.root = Some("src".into());
    let outcome = experience::build(
        &fixture.conn,
        &fixture.config,
        &options,
        shared,
        Some(&model),
    )
    .await;
    assert!(
        outcome
            .unwrap_err()
            .to_string()
            .contains("outside the current human-view inspection grant")
    );
    assert_eq!(model.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn oversized_or_junk_filled_human_cache_does_not_grow_and_generation_remains_usable() {
    let mut fixture = Fixture::new();
    fixture.config.config.context.cache = true;
    let model = HumanModel::new(Behavior::Valid);
    let mut options = fixture.options();
    fixture.experience(&options, Some(&model)).await;
    let directory = fixture.config.state.join("human-view-cache");
    for index in 0..257 {
        fs::write(directory.join(format!("unmanaged-{index}")), b"preserve me").unwrap();
    }
    let names = || {
        fs::read_dir(&directory)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<std::collections::BTreeSet<_>>()
    };
    let before = names();
    for index in 0..3 {
        options.goal = Some(format!("Explain dispatch capacity review {index}"));
        let result = fixture.experience(&options, Some(&model)).await;
        assert_eq!(
            result.presentation_status,
            "source_checked_cache_unavailable"
        );
        assert_eq!(result.generation_basis, GenerationBasis::ModelAssessed);
        assert!(result.orientation.purpose.is_some());
        assert_eq!(names(), before);
    }
    assert_eq!(model.calls.load(Ordering::SeqCst), 8);
}

#[tokio::test]
async fn human_cache_evicts_only_managed_regular_files_before_publishing_new_drafts() {
    let mut fixture = Fixture::new();
    fixture.config.config.context.cache = true;
    let model = HumanModel::new(Behavior::Valid);
    let mut options = fixture.options();
    fixture.experience(&options, Some(&model)).await;
    let directory = fixture.config.state.join("human-view-cache");
    for index in 0..127 {
        fs::write(
            directory.join(format!("hv_{index:064x}.json")),
            b"old derived draft",
        )
        .unwrap();
    }
    let unmanaged = directory.join("notes.txt");
    fs::write(&unmanaged, b"preserve unmanaged content").unwrap();
    let managed_shape_directory = directory.join(format!("hv_{:064x}.json", 9_001));
    fs::create_dir(&managed_shape_directory).unwrap();
    #[cfg(unix)]
    let managed_shape_symlink = {
        let link = directory.join(format!("hv_{:064x}.json", 9_002));
        std::os::unix::fs::symlink(&unmanaged, &link).unwrap();
        link
    };
    for index in 0..3 {
        options.goal = Some(format!(
            "Explain dispatch capacity and ordering review {index}"
        ));
        let result = fixture.experience(&options, Some(&model)).await;
        assert_eq!(result.presentation_status, "source_checked");
        let managed = fs::read_dir(&directory)
            .unwrap()
            .map(|entry| entry.unwrap())
            .filter(|entry| {
                entry.file_type().unwrap().is_file()
                    && entry
                        .file_name()
                        .to_str()
                        .is_some_and(|name| name.starts_with("hv_") && name.ends_with(".json"))
            })
            .count();
        assert_eq!(managed, 128);
        assert_eq!(fs::read(&unmanaged).unwrap(), b"preserve unmanaged content");
        assert!(managed_shape_directory.is_dir());
        #[cfg(unix)]
        assert!(
            fs::symlink_metadata(&managed_shape_symlink)
                .unwrap()
                .file_type()
                .is_symlink()
        );
    }
}

#[tokio::test]
async fn fresh_proposed_design_is_not_presented_as_an_adopted_project_or_learning_rule() {
    let fixture = Fixture::empty();
    fixture.add_with_lifecycle(
        "purpose",
        "design",
        "The proposed dispatch design would send jobs to a remote unbounded queue.",
        "proposed",
    );
    fixture.add_with_lifecycle(
        "workflow",
        "procedure",
        "The proposed dispatch workflow would send each job to the remote queue before processing.",
        "proposed",
    );
    fixture.add_with_lifecycle(
        "capacity",
        "constraint",
        "The proposed dispatch capacity rule would remove the tenant queue bound.",
        "proposed",
    );
    let mut options = fixture.options();
    options.mode = ExperienceMode::Tutorial;
    let result = fixture.experience(&options, None).await;
    assert!(result.orientation.purpose.is_none());
    assert!(result.orientation.workflow.is_none());
    assert!(result.orientation.architecture.is_empty());
    assert!(result.tutorial.is_none());
    assert!(result.orientation.constraints.iter().any(|claim| {
        claim
            .qualifications
            .iter()
            .any(|qualification| qualification.contains("does not establish adoption"))
    }));
    assert!(experience::render_markdown(&result).contains("proposed"));
}
