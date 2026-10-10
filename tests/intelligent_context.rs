//! Intelligence uses real retained documentary/native evidence and an explicit
//! inference double. These tests exercise its trust boundary, not model quality.
use lore::{
    config::{Config, ResolvedConfig},
    context::{
        self, ContextOptions, ContextResult,
        intelligence::{self, IntelligentContextResult},
    },
    domain::{AssertionProposal, ImportKind, SourceMaterial},
    imports::{
        self, Inventory, SourceBatch,
        adapters::{
            AdapterRecord, ImportBatch, NativeEvidence, ObservationKind, ObservationVerification,
        },
    },
    inference::{
        ExecutionLocation, GenerationRequest, GenerationResponse, GenerativeModel, ModelDescriptor,
        ModelError, ModelFuture, Provider, ReasoningEffort,
    },
    sources::{self, Document},
    storage, util,
};
use rusqlite::Connection;
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    fs,
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

struct Fixture {
    _dir: tempfile::TempDir,
    config: ResolvedConfig,
    conn: Connection,
    options: ContextOptions,
}

impl Fixture {
    fn new(payment: bool) -> Self {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("docs")).unwrap();
        let mut config = Config::default();
        config.project.name = "Intelligence fixture".into();
        let path = dir.path().join("lore.yml");
        fs::write(&path, serde_yaml::to_string(&config).unwrap()).unwrap();
        let config = ResolvedConfig::load(&path).unwrap();
        let conn = Connection::open_in_memory().unwrap();
        storage::migrate(&conn).unwrap();
        conn.execute(
            "INSERT INTO projects VALUES('p','Intelligence fixture','2026-10-09')",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO source_roots(id,project_id,configured_path) VALUES('docs','p','/project/docs')", []).unwrap();
        document(
            &conn,
            "ADR-017.md",
            if payment { "decision" } else { "design" },
            if payment {
                "Payment retries permit at most three attempts to avoid duplicate charges. Preserve idempotency in src/payments/retry.rs."
            } else {
                "Dispatch uses a dedicated worker in src/transport/worker.rs."
            },
        );
        storage::refresh_knowledge(&conn, "p").unwrap();
        Self {
            _dir: dir,
            config,
            conn,
            options: ContextOptions {
                task: if payment {
                    "Implement payment retries"
                } else {
                    "Improve dispatch worker"
                }
                .into(),
                paths: vec![],
                max_tokens: 8_000,
            },
        }
    }

    fn selected(&self) -> ContextResult {
        context::build_context(
            &self.conn,
            &ContextOptions {
                max_tokens: 16_000,
                ..self.options.clone()
            },
        )
        .unwrap()
    }

    fn imports(&self, related: bool, old_implementation: bool) {
        let mut implementation = native(
            "retry-limit",
            "OpenWiki reports five payment retries in src/payments/retry.rs at commit-five.",
            ObservationKind::Implementation,
        );
        implementation.verification = ObservationVerification::UpstreamVerifiedAtRevision;
        implementation.lifecycle = if old_implementation {
            "superseded"
        } else {
            "active"
        }
        .into();
        implementation.evidence[0].locator = "src/payments/retry.rs".into();
        implementation.evidence[0].revision = Some("commit-five".into());
        let mut memory = native(
            "incident-memory",
            "After incident INC-8, payment retries were increased to five under PAY-8.",
            ObservationKind::Recollection,
        );
        memory.observed_at = Some("2026-10-08T10:00:00Z".into());
        let mut work = native(
            "PAY-8",
            if related {
                "Payment retries: completed the five-attempt change following incident INC-8."
            } else {
                "Payment dashboard colors: completed theme work unrelated to retry behavior."
            },
            ObservationKind::WorkState,
        );
        work.lifecycle = "closed".into();
        let sources = vec![
            source("wiki", ImportKind::Openwiki, vec![implementation]),
            source("memory", ImportKind::Engram, vec![memory]),
            source("work", ImportKind::Beads, vec![work]),
        ];
        imports::storage::persist(
            &self.conn,
            "p",
            &Inventory {
                sources,
                digest: "fixture".into(),
                warnings: vec![],
            },
        )
        .unwrap();
    }
}

fn document(conn: &Connection, path: &str, kind: &str, text: &str) {
    let document = Document {
        root_id: "docs".into(),
        root_path: "/project/docs".into(),
        material: SourceMaterial::Primary,
        origin: None,
        relative_path: path.into(),
        physical_path: format!("/project/docs/{path}").into(),
        text: text.into(),
        digest: util::digest(text),
        chunks: sources::split_markdown(text, "docs", path, 8_000).unwrap(),
    };
    let proposal = AssertionProposal {
        topic: "delivery".into(),
        topic_title: "Delivery".into(),
        subject: if text.contains("Payment") {
            "payment retries"
        } else {
            "dispatch worker"
        }
        .into(),
        statement: text.into(),
        kind: kind.into(),
        lifecycle: "accepted".into(),
        scope: "production".into(),
        effective_at: String::new(),
        quote: text.into(),
    };
    let (source, revision) = storage::begin_source(conn, &document, None).unwrap();
    let chunk = &document.chunks[0];
    let (section, section_revision) =
        storage::begin_section(conn, &source, &revision, chunk).unwrap();
    let (assertion, _) = storage::capture_assertion(
        conn,
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
    storage::create_unit(conn, "p", &assertion, &proposal).unwrap();
}

fn native(id: &str, statement: &str, kind: ObservationKind) -> AdapterRecord {
    let mut record = AdapterRecord::new(id, statement, json!({"id":id,"statement":statement}));
    record.title = statement.into();
    record.subject = "payment retries".into();
    record.kind = kind;
    record.evidence = vec![NativeEvidence {
        locator: format!("record://{id}"),
        revision: None,
        field: Some("/statement".into()),
    }];
    record
}

fn source(id: &str, kind: ImportKind, records: Vec<AdapterRecord>) -> SourceBatch {
    let batch = ImportBatch {
        format: "intelligence-fixture-v1".into(),
        records,
        warnings: vec![],
    };
    SourceBatch {
        id: id.into(),
        kind,
        path: format!("/project/imports/{id}").into(),
        digest: util::json_digest(&batch).unwrap(),
        batch,
    }
}

#[derive(Clone, Copy)]
enum Behavior {
    Good,
    Unavailable,
    InvalidJson,
    WrongCitation,
    CitationWhitespace,
    CitationPrefix,
    RecordCitation,
    InventedPath,
    MissingConstraint,
    ConcealedDeviation,
    OmittedRiskCheck,
    UnsupportedInference,
    OrdinaryHypothesis,
    MultipleConditions,
}

struct ContextModel {
    descriptor: ModelDescriptor,
    identity: String,
    behavior: Behavior,
    calls: AtomicUsize,
    requests: Mutex<Vec<Value>>,
    efforts: Mutex<Vec<Option<ReasoningEffort>>>,
    override_citation: Option<String>,
}

impl ContextModel {
    fn new(behavior: Behavior) -> Self {
        Self {
            descriptor: ModelDescriptor {
                provider: Provider::Ollama,
                model: "context-fixture-v1".into(),
                location: ExecutionLocation::Local,
            },
            identity: "fixture-endpoint-one".into(),
            behavior,
            calls: AtomicUsize::new(0),
            requests: Mutex::new(vec![]),
            efforts: Mutex::new(vec![]),
            override_citation: None,
        }
    }
}

fn all_ids(input: &Value) -> Vec<Value> {
    input["source_context"]["documentary_evidence"]
        .as_array()
        .unwrap()
        .iter()
        .chain(
            input["source_context"]["native_evidence"]
                .as_array()
                .unwrap(),
        )
        .map(|e| e["id"].clone())
        .collect()
}

fn draft(input: &Value) -> Value {
    let sources = &input["source_context"];
    let evidence = all_ids(input);
    let constraints = sources["adopted_constraints"].as_object().unwrap();
    let payment = input["user_task"].as_str().unwrap().contains("payment");
    let known = sources["knowledge"]
        .as_array()
        .unwrap()
        .iter()
        .chain(sources["observations"].as_array().unwrap())
        .map(|r| r["id"].clone())
        .collect::<Vec<_>>();
    let checks = constraints.iter().map(|(id, ids)| json!({"knowledge_id":id,"disposition": if payment && !sources["observations"].as_array().unwrap().is_empty() { "needs_verification" } else { "preserved" },"explanation":"Preserve the accepted safety constraint; verify the reported deviation and incident rationale before adopting a policy change.","evidence_ids":ids})).collect::<Vec<_>>();
    let observations = sources["observations"].as_array().unwrap();
    let inferred = if observations.is_empty() {
        vec![]
    } else {
        vec![
            json!({"text":"The five-attempt implementation was probably introduced intentionally after the incident; the ADR may be outdated.","evidence_ids":evidence,"confidence":"medium","applicability":"Applies provisionally if the incident, completed work and reported implementation concern the same component and revision; inspect the current checkout before changing behavior.","historical_only":false,"alternatives":[{"text":"The issue may concern different scope or a change that was not deployed; the ADR may still govern the intended policy.","evidence_ids":evidence}]}),
        ]
    };
    let mut risks = vec![];
    if payment {
        risks.push(json!({"category":"financial_correctness","severity":"high","text":"Changing retry behavior can create duplicate charges; preserve idempotency and inspect the existing tests.","evidence_ids":evidence}));
        if !observations.is_empty() {
            risks.push(json!({"category":"accepted_constraint","severity":"high","text":"Five reported retries differ from the accepted limit of three; this is a provisional implementation recommendation, not a superseded ADR.","evidence_ids":evidence}));
        }
    }
    json!({"preferred_approach":{"text":if payment && !observations.is_empty() { "Preserve the reported five-retry behavior provisionally and extend the existing payment abstraction; confirm the incident rationale before revising the policy." } else { "Extend the existing worker while preserving the documented behavior." },"evidence_ids":evidence},
        "known_record_ids":known,"inferred":inferred,"recommended":[],"risks":risks,
        "next_steps":[{"text":if payment { "Inspect src/payments/retry.rs and the idempotency tests, then compare the incident notes with the accepted ADR." } else { "Inspect src/transport/worker.rs before changing its behavior." },"evidence_ids":evidence}],"constraint_checks":checks})
}

impl GenerativeModel for ContextModel {
    fn descriptor(&self) -> &ModelDescriptor {
        &self.descriptor
    }
    fn cache_identity(&self) -> String {
        format!("{}:{}", self.identity, self.descriptor.model)
    }
    fn generate<'a>(
        &'a self,
        request: &'a GenerationRequest,
    ) -> ModelFuture<'a, GenerationResponse> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let input: Value = serde_json::from_str(&request.input).unwrap();
            self.requests.lock().unwrap().push(input.clone());
            self.efforts.lock().unwrap().push(request.reasoning_effort);
            assert!(request.instructions.contains("untrusted data"));
            if matches!(self.behavior, Behavior::Unavailable) {
                return Err(ModelError::Unavailable(
                    "offline; sensitive provider diagnostic".into(),
                ));
            }
            if matches!(self.behavior, Behavior::InvalidJson) {
                return Ok(GenerationResponse {
                    usage: None,
                    model: self.descriptor.model.clone(),
                    text: "not valid JSON".into(),
                });
            }
            let output = if input["task"] == "context_synthesis" {
                let mut result = draft(&input);
                let citation = result["preferred_approach"]["evidence_ids"][0]
                    .as_str()
                    .unwrap()
                    .to_string();
                match self.behavior {
                    Behavior::WrongCitation => {
                        result["preferred_approach"]["evidence_ids"] = json!(["ev_not_selected"])
                    }
                    Behavior::CitationWhitespace => {
                        result["preferred_approach"]["evidence_ids"] =
                            json!([format!(" {citation}")])
                    }
                    Behavior::CitationPrefix => {
                        result["preferred_approach"]["evidence_ids"] = json!([&citation[..20]])
                    }
                    Behavior::RecordCitation => {
                        result["preferred_approach"]["evidence_ids"] =
                            json!([input["source_context"]["knowledge"][0]["id"]])
                    }
                    Behavior::InventedPath => {
                        result["next_steps"][0]["text"] =
                            json!("Inspect src/imagined/nonexistent.rs before proceeding.")
                    }
                    Behavior::MissingConstraint => result["constraint_checks"] = json!([]),
                    Behavior::ConcealedDeviation => result["risks"] = json!([]),
                    Behavior::UnsupportedInference => {
                        result["inferred"][0]["text"] = json!(
                            "The closed issue confirms the current production implementation and independently proves the ADR is superseded."
                        )
                    }
                    Behavior::OrdinaryHypothesis => {
                        let ids = result["preferred_approach"]["evidence_ids"].clone();
                        result["inferred"] = json!([{"text":"The dedicated worker may exist to isolate dispatch scheduling from request handling.","evidence_ids":ids,"confidence":"low","applicability":"Only if the documented worker design applies to the current component; inspect it before assuming this rationale.","historical_only":false,"alternatives":[{"text":"The source may describe a planned design rather than a deployed separation of responsibilities.","evidence_ids":ids}]}]);
                    }
                    Behavior::MultipleConditions => {
                        let ids = result["preferred_approach"]["evidence_ids"].clone();
                        let mut second = result["inferred"][0].clone();
                        second["text"] = json!(
                            "The captured rationale may also depend on how the worker schedules transient failures."
                        );
                        second["applicability"] = json!(format!("Apply only after validating the worker scope. {}", "Inspect the incident rationale and the recorded component boundaries before relying on this interpretation. ".repeat(16)));
                        result["inferred"].as_array_mut().unwrap().push(second);
                        result["next_steps"].as_array_mut().unwrap().push(json!({"text":"Compare the work item and the incident to identify which component and deployment they concern.","evidence_ids":ids}));
                        result["next_steps"].as_array_mut().unwrap().push(json!({"text":"Before relying on the proposed deviation, verify the duplicate-charge and idempotency tests and confirm the incident rationale with the accepted policy owner.","evidence_ids":ids}));
                    }
                    _ => {}
                }
                if let Some(id) = &self.override_citation {
                    result["preferred_approach"]["evidence_ids"] = json!([id]);
                }
                result
            } else {
                assert_eq!(input["task"], "context_verification");
                let constraints = input["source_context"]["adopted_constraints"]
                    .as_object()
                    .unwrap();
                let observations = input["source_context"]["observations"].as_array().unwrap();
                // A model-quality double rejects this fixture's explicitly
                // unrelated/old support; the production code must propagate
                // that rejection and cannot silently publish the first draft.
                let supported = !matches!(self.behavior, Behavior::UnsupportedInference)
                    && !observations.iter().any(|o| {
                        o["statement"].as_str().unwrap().contains("unrelated")
                            || o["lifecycle"] == "superseded"
                    });
                json!({"supported":supported,"evidence_ids":all_ids(&input),"checked_risks":if matches!(self.behavior, Behavior::OmittedRiskCheck) {json!([])} else {input["required_risks"].clone()},
                    "constraint_checks":constraints.iter().map(|(id,ids)| json!({"knowledge_id":id,"acceptable":true,"explanation":"The original accepted policy remains intact; the provisional approach is explicitly conditional and includes targeted verification.","evidence_ids":ids})).collect::<Vec<_>>(),
                    "issues":if supported {json!([])} else {json!(["The claimed current causal explanation is not supported by these exact records."])} })
            };
            Ok(GenerationResponse {
                usage: None,
                model: self.descriptor.model.clone(),
                text: output.to_string(),
            })
        })
    }
}

fn assert_fallback(result: &IntelligentContextResult, phrase: &str) {
    let IntelligentContextResult::FastFallback(result) = result else {
        panic!("expected explicit fallback: {result:?}")
    };
    assert_eq!(result.context.schema_version, 3);
    assert_eq!(result.mode, "fast_fallback");
    assert!(
        result.fallback_reason.contains(phrase),
        "{}",
        result.fallback_reason
    );
    assert!(
        !serde_json::to_string(result)
            .unwrap()
            .contains("sensitive provider diagnostic")
    );
}

fn assert_budget(result: &IntelligentContextResult) {
    let json = serde_json::to_string(result).unwrap() + "\n";
    let markdown = intelligence::render_intelligent_context(result);
    assert!(context::count_tokens(&json) <= result.budget().used_tokens);
    assert!(context::count_tokens(&markdown) <= result.budget().used_tokens);
    assert!(result.budget().used_tokens <= result.budget().max_tokens);
}

#[tokio::test]
async fn four_sources_produce_useful_cited_advice_without_changing_accepted_knowledge() {
    let fixture = Fixture::new(true);
    fixture.imports(true, false);
    let model = ContextModel::new(Behavior::Good);
    let before = fixture.conn.total_changes();
    let original = serde_json::to_value(storage::views(&fixture.conn).unwrap()).unwrap();
    fixture
        .conn
        .pragma_update(None, "query_only", true)
        .unwrap();
    let selected = fixture.selected();
    assert_eq!(selected.imported_observations.len(), 3);
    let result = intelligence::build_intelligent_context(
        &fixture.conn,
        &fixture.config,
        &fixture.options,
        selected,
        Some(&model),
    )
    .await
    .unwrap();
    assert_budget(&result);
    let IntelligentContextResult::Intelligent(result) = &result else {
        panic!("{result:?}")
    };
    assert!(
        result
            .brief
            .preferred_approach
            .text
            .contains("Preserve the reported five-retry behavior")
    );
    assert_eq!(result.brief.scrutiny.verification, "support_check_passed");
    assert_eq!(result.model_calls, 2);
    assert_eq!(
        result
            .brief
            .known
            .iter()
            .filter(|k| k.basis == "documented")
            .count(),
        1
    );
    assert_eq!(
        result
            .brief
            .known
            .iter()
            .filter(|k| k.basis == "observed")
            .count(),
        3
    );
    assert_eq!(result.brief.inferred.len(), 1);
    assert!(
        !result.brief.inferred[0]
            .interpretation
            .alternatives
            .is_empty()
    );
    assert!(
        !result.brief.inferred[0]
            .interpretation
            .applicability
            .is_empty()
    );
    let evidence: BTreeSet<_> = result.evidence.iter().map(|e| &e.id).collect();
    for citation in &result.brief.preferred_approach.evidence_ids {
        assert!(evidence.contains(citation));
    }
    for evidence in &result.evidence {
        if evidence.id.starts_with("ne_") {
            assert_eq!(
                imports::evidence(&fixture.conn, &evidence.id)
                    .unwrap()
                    .snapshot_id,
                evidence.revision_id
            );
        } else {
            assert_eq!(
                storage::evidence_snapshot(&fixture.conn, &evidence.id)
                    .unwrap()
                    .source_revision_id,
                evidence.revision_id
            );
        }
    }
    assert_eq!(before, fixture.conn.total_changes());
    assert_eq!(
        original,
        serde_json::to_value(storage::views(&fixture.conn).unwrap()).unwrap()
    );
    assert!(fixture.config.state.join("context-cache").is_dir());
    let efforts = model.efforts.lock().unwrap();
    assert_eq!(
        *efforts,
        vec![Some(ReasoningEffort::Medium), Some(ReasoningEffort::High)]
    );
}

#[tokio::test]
async fn native_reports_cannot_become_authoritative_known_statements() {
    let fixture = Fixture::new(true);
    fixture.imports(true, false);
    let model = ContextModel::new(Behavior::Good);
    let selected = fixture.selected();
    let statements: BTreeSet<_> = selected
        .imported_observations
        .iter()
        .map(|o| o.statement.clone())
        .collect();
    let result = intelligence::build_intelligent_context(
        &fixture.conn,
        &fixture.config,
        &fixture.options,
        selected,
        Some(&model),
    )
    .await
    .unwrap();
    let IntelligentContextResult::Intelligent(result) = &result else {
        panic!("{result:?}")
    };
    for known in result
        .brief
        .known
        .iter()
        .filter(|known| known.basis == "observed")
    {
        assert!(statements.contains(&known.statement));
        assert!(!known.qualifications.is_empty());
    }
    let rendered = intelligence::render_intelligent_context(
        &IntelligentContextResult::Intelligent(result.clone()),
    );
    assert!(rendered.contains("Observed report:"));
    assert!(rendered.contains("conditional interpretation, not confirmed policy"));
    assert!(rendered.contains("checkout unverified"));
}

#[tokio::test]
async fn four_source_recommendation_retains_its_hypothesis_and_constraint_at_default_budget() {
    let mut fixture = Fixture::new(true);
    fixture.options.max_tokens = context::DEFAULT_MAX_TOKENS;
    fixture.imports(true, false);
    let model = ContextModel::new(Behavior::Good);
    let result = intelligence::build_intelligent_context(
        &fixture.conn,
        &fixture.config,
        &fixture.options,
        fixture.selected(),
        Some(&model),
    )
    .await
    .unwrap();
    assert_budget(&result);
    let IntelligentContextResult::Intelligent(result) = result else {
        panic!("default budget lost the actionable brief: {result:?}")
    };
    assert_eq!(result.brief.inferred.len(), 1);
    assert_eq!(result.brief.constraint_checks.len(), 1);
    assert!(
        result
            .brief
            .known
            .iter()
            .any(|known| known.basis == "documented" && known.statement.contains("three attempts"))
    );
    assert!(result.brief.preferred_approach.text.contains("five-retry"));
    assert!(!result.brief.next_steps.is_empty());
}

#[tokio::test]
async fn unrelated_work_older_observations_and_unsupported_policy_claims_fail_scrutiny() {
    for (related, old, behavior) in [
        (false, false, Behavior::Good),
        (true, true, Behavior::Good),
        (true, false, Behavior::UnsupportedInference),
    ] {
        let fixture = Fixture::new(true);
        fixture.imports(related, old);
        let model = ContextModel::new(behavior);
        let result = intelligence::build_intelligent_context(
            &fixture.conn,
            &fixture.config,
            &fixture.options,
            fixture.selected(),
            Some(&model),
        )
        .await
        .unwrap();
        assert_fallback(&result, "additional support check");
        assert_eq!(result.model_calls(), 2);
        assert!(!fixture.config.state.join("context-cache").exists());
        assert_budget(&result);
    }
}

#[tokio::test]
async fn exact_citation_validation_rejects_prefixes_whitespace_record_ids_and_fabrication() {
    for behavior in [
        Behavior::WrongCitation,
        Behavior::CitationWhitespace,
        Behavior::CitationPrefix,
        Behavior::RecordCitation,
        Behavior::InventedPath,
    ] {
        let fixture = Fixture::new(false);
        let model = ContextModel::new(behavior);
        let result = intelligence::build_intelligent_context(
            &fixture.conn,
            &fixture.config,
            &fixture.options,
            fixture.selected(),
            Some(&model),
        )
        .await
        .unwrap();
        assert_fallback(&result, "exact evidence");
        assert_eq!(result.model_calls(), 1);
    }
}

#[tokio::test]
async fn cannot_skip_accepted_constraints_or_conceal_a_deviation_or_omit_risk_scrutiny() {
    for behavior in [
        Behavior::MissingConstraint,
        Behavior::ConcealedDeviation,
        Behavior::OmittedRiskCheck,
    ] {
        let fixture = Fixture::new(true);
        fixture.imports(true, false);
        let model = ContextModel::new(behavior);
        let result = intelligence::build_intelligent_context(
            &fixture.conn,
            &fixture.config,
            &fixture.options,
            fixture.selected(),
            Some(&model),
        )
        .await
        .unwrap();
        assert_fallback(
            &result,
            if matches!(behavior, Behavior::OmittedRiskCheck) {
                "additional support check"
            } else {
                "constraint validation"
            },
        );
    }
}

#[tokio::test]
async fn cache_reuses_guidance_and_binds_task_paths_evidence_model_endpoint_reasoning_and_privacy()
{
    let mut fixture = Fixture::new(false);
    let model = ContextModel::new(Behavior::Good);
    let first = intelligence::build_intelligent_context(
        &fixture.conn,
        &fixture.config,
        &fixture.options,
        fixture.selected(),
        Some(&model),
    )
    .await
    .unwrap();
    assert_eq!(first.model_calls(), 1);
    let cached = intelligence::build_intelligent_context(
        &fixture.conn,
        &fixture.config,
        &fixture.options,
        fixture.selected(),
        Some(&model),
    )
    .await
    .unwrap();
    let IntelligentContextResult::Intelligent(cached) = cached else {
        panic!("expected cached brief")
    };
    assert_eq!(cached.cache_status, "hit");
    assert_eq!(cached.model_calls, 0);
    assert_eq!(model.calls.load(Ordering::SeqCst), 1);
    fixture.options.task.push_str(" efficiency");
    let changed_task = intelligence::build_intelligent_context(
        &fixture.conn,
        &fixture.config,
        &fixture.options,
        fixture.selected(),
        Some(&model),
    )
    .await
    .unwrap();
    assert_eq!(changed_task.model_calls(), 1);
    fixture.options.paths.push("src/transport/worker.rs".into());
    let changed_paths = intelligence::build_intelligent_context(
        &fixture.conn,
        &fixture.config,
        &fixture.options,
        fixture.selected(),
        Some(&model),
    )
    .await
    .unwrap();
    assert_eq!(changed_paths.model_calls(), 1);
    fixture.config.config.models.reasoning.context_synthesis = ReasoningEffort::High;
    let changed_effort = intelligence::build_intelligent_context(
        &fixture.conn,
        &fixture.config,
        &fixture.options,
        fixture.selected(),
        Some(&model),
    )
    .await
    .unwrap();
    assert_eq!(changed_effort.model_calls(), 1);
    fixture.config.config.privacy.local_only = false;
    let changed_privacy = intelligence::build_intelligent_context(
        &fixture.conn,
        &fixture.config,
        &fixture.options,
        fixture.selected(),
        Some(&model),
    )
    .await
    .unwrap();
    assert_eq!(changed_privacy.model_calls(), 1);
    let mut endpoint = ContextModel::new(Behavior::Good);
    endpoint.identity = "fixture-endpoint-two".into();
    let changed_endpoint = intelligence::build_intelligent_context(
        &fixture.conn,
        &fixture.config,
        &fixture.options,
        fixture.selected(),
        Some(&endpoint),
    )
    .await
    .unwrap();
    assert_eq!(changed_endpoint.model_calls(), 1);
    document(
        &fixture.conn,
        "worker-history.md",
        "design",
        "Dispatch uses a dedicated worker and a bounded queue in src/transport/worker.rs.",
    );
    storage::refresh_knowledge(&fixture.conn, "p").unwrap();
    let changed_evidence = intelligence::build_intelligent_context(
        &fixture.conn,
        &fixture.config,
        &fixture.options,
        fixture.selected(),
        Some(&model),
    )
    .await
    .unwrap();
    assert_eq!(changed_evidence.model_calls(), 1);
    fixture.options.max_tokens = 7_000;
    let changed_budget = intelligence::build_intelligent_context(
        &fixture.conn,
        &fixture.config,
        &fixture.options,
        fixture.selected(),
        Some(&model),
    )
    .await
    .unwrap();
    assert_eq!(changed_budget.model_calls(), 1);
}

#[tokio::test]
async fn replacing_native_evidence_invalidates_revision_bound_interpretations() {
    let fixture = Fixture::new(true);
    fixture.imports(true, false);
    let model = ContextModel::new(Behavior::Good);
    let first = intelligence::build_intelligent_context(
        &fixture.conn,
        &fixture.config,
        &fixture.options,
        fixture.selected(),
        Some(&model),
    )
    .await
    .unwrap();
    let IntelligentContextResult::Intelligent(first) = first else {
        panic!("expected first interpretation")
    };
    let key = first.brief.inferred[0].revision_key.clone();
    fixture.imports(false, false);
    let changed = intelligence::build_intelligent_context(
        &fixture.conn,
        &fixture.config,
        &fixture.options,
        fixture.selected(),
        Some(&model),
    )
    .await
    .unwrap();
    assert_fallback(&changed, "additional support check");
    assert_eq!(changed.model_calls(), 2);
    assert!(!intelligence::render_intelligent_context(&changed).contains(&key));
    assert_eq!(model.calls.load(Ordering::SeqCst), 4);
}

#[tokio::test]
async fn unavailable_disabled_private_invalid_and_absent_models_return_explicit_fast_context() {
    for behavior in [Behavior::Unavailable, Behavior::InvalidJson] {
        let fixture = Fixture::new(false);
        let model = ContextModel::new(behavior);
        let result = intelligence::build_intelligent_context(
            &fixture.conn,
            &fixture.config,
            &fixture.options,
            fixture.selected(),
            Some(&model),
        )
        .await
        .unwrap();
        assert_fallback(&result, "deterministic context");
        assert_eq!(result.model_calls(), 1);
        assert_budget(&result);
    }
    let mut fixture = Fixture::new(false);
    let mut model = ContextModel::new(Behavior::Good);
    model.descriptor.provider = Provider::OpenAi;
    model.descriptor.location = ExecutionLocation::Hosted;
    let private = intelligence::build_intelligent_context(
        &fixture.conn,
        &fixture.config,
        &fixture.options,
        fixture.selected(),
        Some(&model),
    )
    .await
    .unwrap();
    assert_fallback(&private, "local-only privacy");
    assert_eq!(model.calls.load(Ordering::SeqCst), 0);
    let absent = intelligence::build_intelligent_context(
        &fixture.conn,
        &fixture.config,
        &fixture.options,
        fixture.selected(),
        None,
    )
    .await
    .unwrap();
    assert_fallback(&absent, "No enabled generative model");
    fixture.config.config.models.generative.enabled = false;
    let disabled = intelligence::build_intelligent_context(
        &fixture.conn,
        &fixture.config,
        &fixture.options,
        fixture.selected(),
        Some(&model),
    )
    .await
    .unwrap();
    assert_fallback(&disabled, "No enabled generative model");
    assert_eq!(disabled.model_calls(), 0);
}

#[tokio::test]
async fn tight_output_budgets_and_input_limits_remain_enforced_in_both_formats() {
    let mut fixture = Fixture::new(false);
    let model = ContextModel::new(Behavior::Good);
    for budget in [512, 1_000, 3_000, 8_000] {
        fixture.options.max_tokens = budget;
        let result = intelligence::build_intelligent_context(
            &fixture.conn,
            &fixture.config,
            &fixture.options,
            fixture.selected(),
            Some(&model),
        )
        .await
        .unwrap();
        assert_budget(&result);
    }
    fixture.options.max_tokens = 3_000;
    fixture.config.config.processing.max_context_bytes = 512;
    let before = model.calls.load(Ordering::SeqCst);
    let result = intelligence::build_intelligent_context(
        &fixture.conn,
        &fixture.config,
        &fixture.options,
        fixture.selected(),
        Some(&model),
    )
    .await
    .unwrap();
    assert_fallback(&result, "inference input budget");
    assert_eq!(model.calls.load(Ordering::SeqCst), before);
    assert_budget(&result);
}

#[tokio::test]
async fn cache_disabled_and_unwritable_cache_do_not_prevent_intelligence() {
    let mut fixture = Fixture::new(false);
    fixture.config.config.context.cache = false;
    let model = ContextModel::new(Behavior::Good);
    for _ in 0..2 {
        let result = intelligence::build_intelligent_context(
            &fixture.conn,
            &fixture.config,
            &fixture.options,
            fixture.selected(),
            Some(&model),
        )
        .await
        .unwrap();
        let IntelligentContextResult::Intelligent(result) = result else {
            panic!("expected uncached brief")
        };
        assert_eq!(result.cache_status, "disabled");
        assert_eq!(result.model_calls, 1);
    }
    assert!(!fixture.config.state.exists());
    fixture.config.config.context.cache = true;
    fs::create_dir_all(&fixture.config.state).unwrap();
    fs::write(
        fixture.config.state.join("context-cache"),
        "not a directory",
    )
    .unwrap();
    let result = intelligence::build_intelligent_context(
        &fixture.conn,
        &fixture.config,
        &fixture.options,
        fixture.selected(),
        Some(&model),
    )
    .await
    .unwrap();
    let IntelligentContextResult::Intelligent(result) = result else {
        panic!("expected usable brief")
    };
    assert_eq!(result.cache_status, "unavailable");
    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.contains("could not be cached"))
    );
}

#[tokio::test]
async fn stale_selected_revisions_are_not_sent_to_the_model() {
    let fixture = Fixture::new(true);
    fixture.imports(true, false);
    let stale = fixture.selected();
    fixture.imports(false, false);
    let model = ContextModel::new(Behavior::Good);
    let result = intelligence::build_intelligent_context(
        &fixture.conn,
        &fixture.config,
        &fixture.options,
        stale,
        Some(&model),
    )
    .await
    .unwrap();
    assert_fallback(&result, "Selected evidence changed");
    assert_eq!(model.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn fast_retrieval_is_deterministic_read_only_and_never_touches_guidance_cache() {
    let fixture = Fixture::new(false);
    fixture
        .conn
        .pragma_update(None, "query_only", true)
        .unwrap();
    let before = fixture.conn.total_changes();
    let first = context::build_context(&fixture.conn, &fixture.options).unwrap();
    let second = context::build_context(&fixture.conn, &fixture.options).unwrap();
    assert_eq!(
        serde_json::to_value(&first).unwrap(),
        serde_json::to_value(&second).unwrap()
    );
    assert_eq!(first.schema_version, 2);
    assert_eq!(first.model_calls, 0);
    assert_eq!(fixture.conn.total_changes(), before);
    assert!(!fixture.config.state.exists());
}

#[tokio::test]
async fn ordinary_hypotheses_get_a_support_check_and_historical_only_support_is_not_current() {
    let fixture = Fixture::new(false);
    let model = ContextModel::new(Behavior::OrdinaryHypothesis);
    let result = intelligence::build_intelligent_context(
        &fixture.conn,
        &fixture.config,
        &fixture.options,
        fixture.selected(),
        Some(&model),
    )
    .await
    .unwrap();
    assert_eq!(result.model_calls(), 2);
    let IntelligentContextResult::Intelligent(result) = result else {
        panic!("expected qualified ordinary hypothesis")
    };
    assert_eq!(result.brief.scrutiny.verification, "support_check_passed");
    assert!(
        model.requests.lock().unwrap()[1]["required_risks"]
            .as_array()
            .unwrap()
            .contains(&json!("other"))
    );
    let source: String = fixture
        .conn
        .query_row("SELECT id FROM sources LIMIT 1", [], |row| row.get(0))
        .unwrap();
    storage::retire_source(&fixture.conn, &source).unwrap();
    storage::refresh_knowledge(&fixture.conn, "p").unwrap();
    let historical = intelligence::build_intelligent_context(
        &fixture.conn,
        &fixture.config,
        &fixture.options,
        fixture.selected(),
        Some(&model),
    )
    .await
    .unwrap();
    assert_fallback(&historical, "applicability");
    assert_eq!(historical.model_calls(), 1);
}

#[tokio::test]
async fn destructive_and_security_tasks_cannot_avoid_scrutiny_by_omitting_risk_labels() {
    for (task, risk) in [
        ("Delete dispatch worker data", "destructive"),
        ("Change dispatch worker authentication", "security"),
    ] {
        let mut fixture = Fixture::new(false);
        fixture.options.task = task.into();
        let model = ContextModel::new(Behavior::Good);
        let result = intelligence::build_intelligent_context(
            &fixture.conn,
            &fixture.config,
            &fixture.options,
            fixture.selected(),
            Some(&model),
        )
        .await
        .unwrap();
        assert_eq!(result.model_calls(), 2);
        let requests = model.requests.lock().unwrap();
        assert!(
            requests[1]["required_risks"]
                .as_array()
                .unwrap()
                .contains(&json!(risk))
        );
    }
}

#[tokio::test]
async fn a_valid_registry_citation_outside_selected_evidence_is_rejected() {
    let fixture = Fixture::new(false);
    document(
        &fixture.conn,
        "elsewhere.md",
        "design",
        "An unrelated dispatch worker prototype has separate storage.",
    );
    storage::refresh_knowledge(&fixture.conn, "p").unwrap();
    let mut selected = fixture.selected();
    let omitted = selected
        .evidence
        .iter()
        .find(|evidence| evidence.source.ends_with("elsewhere.md"))
        .unwrap()
        .id
        .clone();
    selected.evidence.retain(|evidence| evidence.id != omitted);
    selected
        .suggested_inspection
        .retain(|path| path.evidence_id != omitted);
    for section in [
        &mut selected.sections.constraints,
        &mut selected.sections.decisions,
        &mut selected.sections.current_designs,
        &mut selected.sections.historical,
        &mut selected.sections.needs_verification,
        &mut selected.sections.proposals,
        &mut selected.sections.relevant_knowledge,
    ] {
        section.retain(|item| !item.evidence_ids.contains(&omitted));
    }
    assert!(storage::evidence_snapshot(&fixture.conn, &omitted).is_ok());
    let mut model = ContextModel::new(Behavior::Good);
    model.override_citation = Some(omitted);
    let result = intelligence::build_intelligent_context(
        &fixture.conn,
        &fixture.config,
        &fixture.options,
        selected,
        Some(&model),
    )
    .await
    .unwrap();
    assert_fallback(&result, "exact evidence");
    assert_eq!(result.model_calls(), 1);
}

#[tokio::test]
async fn corrupted_cache_is_ignored_and_replaced_by_validated_guidance() {
    let fixture = Fixture::new(false);
    let model = ContextModel::new(Behavior::Good);
    intelligence::build_intelligent_context(
        &fixture.conn,
        &fixture.config,
        &fixture.options,
        fixture.selected(),
        Some(&model),
    )
    .await
    .unwrap();
    let cache = fs::read_dir(fixture.config.state.join("context-cache"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let mut value: Value = serde_json::from_slice(&fs::read(&cache).unwrap()).unwrap();
    value["draft"]["preferred_approach"]["text"] = json!("Unvalidated altered cached advice.");
    fs::write(&cache, value.to_string()).unwrap();
    let result = intelligence::build_intelligent_context(
        &fixture.conn,
        &fixture.config,
        &fixture.options,
        fixture.selected(),
        Some(&model),
    )
    .await
    .unwrap();
    assert_eq!(result.model_calls(), 1);
    assert!(!intelligence::render_intelligent_context(&result).contains("Unvalidated altered"));
    let cached_again = intelligence::build_intelligent_context(
        &fixture.conn,
        &fixture.config,
        &fixture.options,
        fixture.selected(),
        Some(&model),
    )
    .await
    .unwrap();
    assert_eq!(cached_again.model_calls(), 0);
}

#[tokio::test]
async fn unchanged_section_evidence_remains_usable_after_source_revision_and_rename() {
    let fixture = Fixture::new(false);
    storage::set_meta(&fixture.conn, "initialized", "true").unwrap();
    let original = fixture.selected();
    let evidence_id = original.evidence[0].id.clone();
    let captured_revision = original.evidence[0].source_revision_id.clone();
    let model = ContextModel::new(Behavior::Good);
    for path in ["ADR-017.md", "architecture/dispatch.md"] {
        let head = storage::source_heads(&fixture.conn).unwrap().remove(0);
        let text = "Dispatch uses a dedicated worker in src/transport/worker.rs.\n\n# Another section\nUnrelated build metadata changed.";
        let document = Document {
            root_id: "docs".into(),
            root_path: "/project/docs".into(),
            material: SourceMaterial::Primary,
            origin: None,
            relative_path: path.into(),
            physical_path: format!("/project/docs/{path}").into(),
            text: text.into(),
            digest: util::digest(text),
            chunks: sources::split_markdown(text, "docs", path, 8_000).unwrap(),
        };
        let (_, updated_revision) =
            storage::begin_source(&fixture.conn, &document, Some(&head)).unwrap();
        assert_ne!(updated_revision, captured_revision);
        // Incremental compilation preserves the unchanged section's existing
        // active assertion and exact captured evidence at its earlier revision.
        let selected = fixture.selected();
        let evidence = selected
            .evidence
            .iter()
            .find(|e| e.id == evidence_id)
            .unwrap();
        assert_eq!(evidence.source_revision_id, captured_revision);
        assert!(evidence.current);
        assert!(
            selected
                .suggested_inspection
                .iter()
                .any(|p| p.basis == "source_record" && p.path == path)
        );
        let result = intelligence::build_intelligent_context(
            &fixture.conn,
            &fixture.config,
            &fixture.options,
            selected,
            Some(&model),
        )
        .await
        .unwrap();
        assert_budget(&result);
        let IntelligentContextResult::Intelligent(result) = result else {
            panic!("active older evidence must remain usable after {path}: {result:?}")
        };
        assert!(
            result
                .evidence
                .iter()
                .any(|e| e.id == evidence_id && e.current && e.revision_id == captured_revision)
        );
        assert!(
            result
                .suggested_inspection
                .iter()
                .any(|p| p.basis == "source_record" && p.path == path)
        );
    }
}

#[tokio::test]
async fn budgets_keep_all_conditions_and_late_required_verification_steps_or_fall_back() {
    let mut fixture = Fixture::new(true);
    fixture.imports(true, false);
    let model = ContextModel::new(Behavior::MultipleConditions);
    let mut saw_guidance = false;
    let mut saw_fallback = false;
    for budget in [1_500, 3_000, 8_000] {
        fixture.options.max_tokens = budget;
        let result = intelligence::build_intelligent_context(
            &fixture.conn,
            &fixture.config,
            &fixture.options,
            fixture.selected(),
            Some(&model),
        )
        .await
        .unwrap();
        assert_budget(&result);
        match result {
            IntelligentContextResult::Intelligent(result) => {
                saw_guidance = true;
                assert_eq!(
                    result.brief.inferred.len(),
                    2,
                    "budget detached an applicability condition"
                );
                assert_eq!(
                    result.brief.next_steps.len(),
                    3,
                    "budget removed a necessary verification step"
                );
                assert!(result.brief.next_steps[2].text.contains("policy owner"));
            }
            IntelligentContextResult::FastFallback(result) => {
                saw_fallback = true;
                assert!(result.fallback_reason.contains("output budget"));
            }
        }
    }
    assert!(saw_guidance && saw_fallback);
}
