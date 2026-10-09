//! Schema 4 trust-boundary tests use actual retained records and explicit model
//! drafts. They establish contract behavior, not real-world recommendation quality.
use lore::{
    context::{
        self, ContextOptions, ContextResult,
        decision::{self, DraftDecision, FactProvenance},
        inspection::CodeObservation,
    },
    domain::{AssertionProposal, SourceMaterial},
    sources::{self, Document},
    storage, util,
};
use rusqlite::Connection;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const RULE: &str =
    "Dispatch uses bounded queues. Preserve per-key ordering in src/dispatch/queue.rs.";

struct Fixture {
    conn: Connection,
    selected: ContextResult,
    evidence_id: String,
    knowledge_id: String,
}

impl Fixture {
    fn new() -> Self {
        let conn = Connection::open_in_memory().unwrap();
        storage::migrate(&conn).unwrap();
        conn.execute(
            "INSERT INTO projects VALUES('p','Dispatch fixture','2026-10-09')",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO source_roots(id,project_id,configured_path) VALUES('docs','p','/project/docs')", []).unwrap();
        let document = Document {
            root_id: "docs".into(),
            root_path: "/project/docs".into(),
            material: SourceMaterial::Primary,
            origin: None,
            relative_path: "dispatch-contract.md".into(),
            physical_path: "/project/docs/dispatch-contract.md".into(),
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
        let selected = context::build_context(
            &conn,
            &ContextOptions {
                task: "Refactor dispatch queue allocation".into(),
                paths: vec![],
                max_tokens: 8_000,
            },
        )
        .unwrap();
        let knowledge_id = selected.sections.constraints[0].id.clone();
        let evidence_id = selected.sections.constraints[0].evidence_ids[0].clone();
        Self {
            conn,
            selected,
            evidence_id,
            knowledge_id,
        }
    }

    fn draft(&self) -> Value {
        let advice = |text: &str| json!({"text":text,"evidence_ids":[self.evidence_id],"observation_ids":[]});
        json!({
            "readiness":"proceed", "change_kind":"behavior_preserving",
            "preferred_approach":advice("Refactor dispatch queue allocation while retaining the queue bound and per-key ordering."),
            "rationale":advice("Keeping the documented dispatch boundary limits the refactor's behavioral scope."),
            "main_tradeoff":advice("Preserving ordering limits opportunities to parallelize delivery of the same key."),
            "next_action":advice("Extract queue allocation from dispatch while keeping enqueue order for each key unchanged."),
            "completion_criteria":[advice("Focused dispatch cases preserve ordering for repeated keys and enforce the queue bound.")],
            "known_record_ids":[],
            "hypotheses":[],
            "heuristics":[{"provenance":"general_engineering","principle":"Keep externally visible outcomes stable during a structural refactor.","application":"Separate dispatch allocation changes from changes to delivery ordering."}],
            "constraints":[{"knowledge_id":self.knowledge_id,"disposition":"preserved","explanation":"Keep bounded queues and per-key ordering unchanged.","evidence_ids":[self.evidence_id],"observation_ids":[]}],
            "checks":[{"priority":"recommended_during_implementation","action":"Exercise consecutive dispatches for the same key at the queue boundary.","decision_impact":"A reordered dispatch or an exceeded bound reveals a regression in the preserving refactor.","inexpensive":true,"evidence_ids":[self.evidence_id],"observation_ids":[]}],
            "implementation_seams":[], "risks":[], "material_blockers":[], "counterevidence":[], "remaining_uncertainty":[]
        })
    }

    fn validate(
        &self,
        draft: Value,
        observations: &[CodeObservation],
    ) -> anyhow::Result<decision::DecisionBrief> {
        let draft: DraftDecision = serde_json::from_value(draft)?;
        decision::validate_decision(
            &self.conn,
            &self.selected,
            observations,
            &draft,
            "test-revision",
        )
    }

    fn risk(&self) -> Value {
        json!({"category":"accepted_constraint","severity":"high","text":"Removing the bound would deviate from the adopted dispatch constraint.","evidence_ids":[self.evidence_id],"observation_ids":[]})
    }

    fn blocker(&self) -> Value {
        json!({"kind":"policy_decision","knowledge_id":self.knowledge_id,"explanation":"An unbounded dispatch queue would contradict the adopted queue bound.","decision_needed":"Decide whether the dispatch contract should permit unbounded queues.","evidence_ids":[self.evidence_id],"observation_ids":[]})
    }
}

fn observation(path: &str) -> CodeObservation {
    let excerpt = "const QUEUE_CAPACITY: usize = 128;\n".to_string();
    let content_hash = format!("sha256:{:x}", Sha256::digest(excerpt.as_bytes()));
    let id = format!(
        "co_{:x}",
        Sha256::digest(serde_json::to_vec(&(path, 1, 1, &excerpt, &content_hash)).unwrap())
    );
    CodeObservation {
        id,
        path: path.into(),
        start_line: 1,
        end_line: 1,
        excerpt,
        content_hash,
        kind: "static_source".into(),
        qualification: "Static source inspection; not proof of runtime behavior or test execution."
            .into(),
    }
}

#[test]
fn preserving_change_can_proceed_with_implementation_checks_and_exact_copied_facts() {
    let fixture = Fixture::new();
    let brief = fixture.validate(fixture.draft(), &[]).unwrap();
    assert_eq!(brief.readiness, decision::ActionReadiness::Proceed);
    assert_eq!(
        brief.facts.len(),
        1,
        "adopted constraints cannot be hidden by omitted known_record_ids"
    );
    assert_eq!(brief.facts[0].record.statement, RULE);
    assert_eq!(brief.facts[0].record.record_id, fixture.knowledge_id);
    assert_eq!(brief.facts[0].provenance, FactProvenance::Documentary);
    assert!(
        brief.facts[0]
            .record
            .qualifications
            .iter()
            .any(|q| q.contains("not checkout verification"))
    );
    let output = serde_json::to_value(&brief).unwrap();
    assert_eq!(output["heuristics"][0]["provenance"], "general_engineering");
    assert!(output["heuristics"][0].get("evidence_ids").is_none());
}

#[test]
fn verification_qualifier_does_not_itself_block_a_preserving_change() {
    let fixture = Fixture::new();
    let mut draft = fixture.draft();
    draft["constraints"][0]["disposition"] = json!("needs_verification");
    draft["risks"] = json!([fixture.risk()]);
    assert!(fixture.validate(draft.clone(), &[]).is_ok());
    draft["readiness"] = json!("blocked");
    draft["material_blockers"] = json!([fixture.blocker()]);
    assert!(
        fixture.validate(draft, &[]).is_err(),
        "a disagreement/verification need is not a policy decision"
    );
}

#[test]
fn unresolved_policy_deviation_is_a_real_material_blocker() {
    let fixture = Fixture::new();
    let mut draft = fixture.draft();
    draft["readiness"] = json!("blocked");
    draft["change_kind"] = json!("policy_changing");
    draft["constraints"][0]["disposition"] = json!("proposed_deviation");
    draft["risks"] = json!([fixture.risk()]);
    draft["material_blockers"] = json!([fixture.blocker()]);
    assert!(fixture.validate(draft.clone(), &[]).is_ok());
    draft["material_blockers"] = json!([]);
    assert!(fixture.validate(draft, &[]).is_err());
}

#[test]
fn proceed_after_check_requires_one_or_two_inexpensive_specific_checks() {
    let fixture = Fixture::new();
    let mut draft = fixture.draft();
    draft["readiness"] = json!("proceed_after_check");
    assert!(fixture.validate(draft.clone(), &[]).is_err());
    draft["checks"][0]["priority"] = json!("required_before_proceeding");
    assert!(fixture.validate(draft.clone(), &[]).is_ok());
    draft["checks"][0]["inexpensive"] = json!(false);
    assert!(fixture.validate(draft.clone(), &[]).is_err());
    draft["checks"][0]["inexpensive"] = json!(true);
    let check = draft["checks"][0].clone();
    draft["checks"] = json!([check, check, check]);
    assert!(fixture.validate(draft, &[]).is_err());
}

#[test]
fn proceed_cannot_conceal_a_required_check_or_material_decision() {
    let fixture = Fixture::new();
    let mut draft = fixture.draft();
    draft["checks"][0]["priority"] = json!("required_before_proceeding");
    assert!(fixture.validate(draft, &[]).is_err());
    let mut draft = fixture.draft();
    draft["material_blockers"] = json!([fixture.blocker()]);
    assert!(fixture.validate(draft, &[]).is_err());
}

#[test]
fn all_constraints_and_exact_unique_evidence_ids_are_required() {
    let fixture = Fixture::new();
    let mut draft = fixture.draft();
    draft["constraints"] = json!([]);
    assert!(fixture.validate(draft, &[]).is_err());
    for ids in [
        json!([fixture.knowledge_id]),
        json!([format!(" {}", fixture.evidence_id)]),
        json!([fixture.evidence_id, fixture.evidence_id]),
        json!([]),
    ] {
        let mut draft = fixture.draft();
        draft["preferred_approach"]["evidence_ids"] = ids;
        assert!(fixture.validate(draft, &[]).is_err());
    }
}

#[test]
fn facts_cannot_be_model_written_and_heuristics_cannot_borrow_project_citations() {
    let fixture = Fixture::new();
    let mut draft = fixture.draft();
    draft["facts"] = json!([{"statement":"The project runs an unbounded queue."}]);
    assert!(serde_json::from_value::<DraftDecision>(draft).is_err());
    let mut draft = fixture.draft();
    draft["heuristics"][0]["evidence_ids"] = json!([fixture.evidence_id]);
    assert!(serde_json::from_value::<DraftDecision>(draft).is_err());
    let mut draft = fixture.draft();
    draft["preferred_approach"]["shell_command"] = json!("run arbitrary code");
    assert!(serde_json::from_value::<DraftDecision>(draft).is_err());
}

#[test]
fn implementation_seams_require_exact_hash_bound_observations_for_the_same_path() {
    let fixture = Fixture::new();
    let observation = observation("src/dispatch/queue.rs");
    let mut draft = fixture.draft();
    draft["implementation_seams"] = json!([{"path":observation.path,"observation_ids":[observation.id],"purpose":"Keep the queue allocation bounded at the existing capacity."}]);
    assert!(
        fixture
            .validate(draft.clone(), std::slice::from_ref(&observation))
            .is_ok()
    );
    draft["implementation_seams"][0]["path"] = json!("src/dispatch/unseen.rs");
    assert!(
        fixture
            .validate(draft.clone(), std::slice::from_ref(&observation))
            .is_err()
    );
    draft["implementation_seams"][0]["path"] = json!(observation.path);
    assert!(
        fixture.validate(draft, &[]).is_err(),
        "a retained path suggestion alone is not an inspection"
    );
}

#[test]
fn invented_paths_and_runtime_execution_assertions_are_rejected() {
    let fixture = Fixture::new();
    for text in [
        "Change src/invented/queue.rs to preserve dispatch ordering.",
        "Inspect /etc/shadow before dispatch allocation.",
        "The tests passed and dispatch ordering is correct.",
        "I ran the tests and verified at runtime that dispatch is bounded.",
    ] {
        let mut draft = fixture.draft();
        draft["next_action"]["text"] = json!(text);
        assert!(
            fixture.validate(draft, &[]).is_err(),
            "unexpectedly accepted {text}"
        );
    }
    let mut draft = fixture.draft();
    draft["completion_criteria"][0]["text"] =
        json!("Focused dispatch tests pass after the allocation refactor.");
    assert!(
        fixture.validate(draft, &[]).is_ok(),
        "a future completion criterion is not a claim of executed tests"
    );
}

#[test]
fn hypotheses_remain_qualified_and_direct_static_evidence_can_support_narrow_confidence() {
    let fixture = Fixture::new();
    let observation = observation("src/dispatch/queue.rs");
    let mut draft = fixture.draft();
    draft["hypotheses"] = json!([{
        "provenance":"hypothesis", "text":"The displayed capacity constant is likely the seam for queue allocation.",
        "evidence_ids":[], "observation_ids":[observation.id], "confidence":"high",
        "applicability":"Applies to this source declaration; effective runtime capacity can still depend on callers.", "historical_only":false,
        "alternatives":[{"text":"A caller may select another bound.","evidence_ids":[fixture.evidence_id],"observation_ids":[]}]
    }]);
    assert!(
        fixture
            .validate(draft.clone(), std::slice::from_ref(&observation))
            .is_ok()
    );
    draft["hypotheses"][0]["alternatives"] = json!([]);
    assert!(
        fixture
            .validate(draft, std::slice::from_ref(&observation))
            .is_err()
    );
}

#[test]
fn generic_review_and_validation_are_not_decision_ready_next_actions() {
    let fixture = Fixture::new();
    for text in [
        "Review the implementation and validate behavior.",
        "Inspect relevant code and run tests.",
    ] {
        let mut draft = fixture.draft();
        draft["next_action"]["text"] = json!(text);
        assert!(fixture.validate(draft, &[]).is_err());
    }
}

#[test]
fn public_validation_rechecks_retained_facts_instead_of_trusting_selected_text() {
    let mut fixture = Fixture::new();
    let draft = fixture.draft();
    fixture.selected.sections.constraints[0].statement =
        "Dispatch no longer preserves ordering.".into();
    assert!(fixture.validate(draft, &[]).is_err());
}
