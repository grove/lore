//! Context assembly uses real immutable evidence and stored relationships.
//! These tests never call a model or infer relationships from fixture prose.
use lore::{
    context::{self, ContextOptions, ContextResult},
    domain::{AssertionProposal, SourceMaterial},
    sources::{self, Document},
    storage, util,
};
use rusqlite::Connection;
use std::collections::BTreeSet;

struct Record {
    id: String,
    source: String,
    assertion: String,
    evidence: String,
}

fn database() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    storage::migrate(&conn).unwrap();
    conn.execute(
        "INSERT INTO projects VALUES('p','Assembly fixture','2026-10-09')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO source_roots(id,project_id,configured_path) VALUES('docs','p','/project/docs')",
        [],
    )
    .unwrap();
    conn
}

fn record(
    conn: &Connection,
    path: &str,
    kind: &str,
    lifecycle: &str,
    text: &str,
    material: SourceMaterial,
) -> Record {
    let document = Document {
        root_id: "docs".into(),
        root_path: "/project/docs".into(),
        material,
        origin: (material == SourceMaterial::Derived)
            .then(|| "https://example.invalid/generated-documentation".into()),
        relative_path: path.into(),
        physical_path: format!("/project/docs/{path}").into(),
        text: text.into(),
        digest: util::digest(text),
        chunks: sources::split_markdown(text, "docs", path, 8_000).unwrap(),
    };
    let proposal = AssertionProposal {
        topic: "delivery".into(),
        topic_title: "Delivery".into(),
        subject: "dispatch policy".into(),
        statement: text.into(),
        kind: kind.into(),
        lifecycle: lifecycle.into(),
        scope: "production".into(),
        effective_at: String::new(),
        quote: text.into(),
    };
    let (source, source_revision) = storage::begin_source(conn, &document, None).unwrap();
    let chunk = &document.chunks[0];
    let (section, section_revision) =
        storage::begin_section(conn, &source, &source_revision, chunk).unwrap();
    let (assertion, evidence) = storage::capture_assertion(
        conn,
        storage::AssertionCapture {
            document: &document,
            chunk,
            proposal: &proposal,
            source: &source,
            source_revision: &source_revision,
            section: &section,
            section_revision: &section_revision,
            model: "fixture-without-inference",
        },
    )
    .unwrap();
    let id = storage::create_unit(conn, "p", &assertion, &proposal).unwrap();
    Record {
        id,
        source,
        assertion,
        evidence,
    }
}

fn relation(conn: &Connection, from: &Record, to: &Record, kind: &str) {
    storage::add_relation(
        conn,
        &from.id,
        &to.id,
        kind,
        &from.assertion,
        &from.evidence,
    )
    .unwrap();
}

fn build(conn: &Connection, task: &str, paths: &[&str], budget: usize) -> ContextResult {
    conn.pragma_update(None, "query_only", true).unwrap();
    context::build_context(
        conn,
        &ContextOptions {
            task: task.into(),
            paths: paths.iter().map(|p| p.to_string()).collect(),
            max_tokens: budget,
        },
    )
    .unwrap()
}

fn ids(result: &ContextResult) -> BTreeSet<String> {
    result
        .sections
        .items()
        .map(|item| item.id.clone())
        .collect()
}

fn assert_citations_resolve(conn: &Connection, result: &ContextResult) {
    let included = ids(result);
    let evidence_ids: BTreeSet<_> = result.evidence.iter().map(|e| e.id.clone()).collect();
    assert_eq!(evidence_ids.len(), result.evidence.len());
    for item in result.sections.items() {
        assert!(!item.evidence_ids.is_empty());
        for citation in &item.evidence_ids {
            assert!(evidence_ids.contains(citation));
        }
    }
    for relation in &result.relations {
        assert!(included.contains(&relation.from));
        assert!(included.contains(&relation.to));
        assert!(evidence_ids.contains(&relation.evidence_id));
    }
    for review in &result.reviews {
        assert!(review.knowledge_ids.iter().all(|id| included.contains(id)));
        assert!(
            review
                .evidence_ids
                .iter()
                .all(|id| evidence_ids.contains(id))
        );
    }
    for evidence in &result.evidence {
        let archived = storage::evidence_snapshot(conn, &evidence.id).unwrap();
        assert_eq!(archived.excerpt, evidence.excerpt);
        assert_eq!(archived.source_revision_id, evidence.source_revision_id);
        assert_eq!(archived.material, evidence.material);
    }
    assert_eq!(result.model_calls, 0);
    assert_eq!(
        conn.query_row::<i64, _, _>("SELECT count(*) FROM model_calls", [], |r| r.get(0))
            .unwrap(),
        0
    );
}

#[test]
fn derived_and_unknown_constraints_require_verification() {
    let conn = database();
    let derived = record(
        &conn,
        "generated-policy.md",
        "constraint",
        "active",
        "Dispatch must keep a stable operation identifier.",
        SourceMaterial::Derived,
    );
    let unknown = record(
        &conn,
        "unresolved-policy.md",
        "constraint",
        "unknown",
        "Dispatch must stop after three attempts; adoption remains undecided.",
        SourceMaterial::Primary,
    );
    let accepted = record(
        &conn,
        "accepted-policy.md",
        "constraint",
        "accepted",
        "Dispatch must preserve request authentication.",
        SourceMaterial::Primary,
    );
    storage::refresh_knowledge(&conn, "p").unwrap();
    let result = build(&conn, "Change dispatch", &[], 20_000);
    assert_eq!(ids(&result).len(), 3);
    assert_eq!(result.sections.constraints.len(), 1);
    assert_eq!(result.sections.constraints[0].id, accepted.id);
    let verification = &result.sections.needs_verification;
    let item = verification.iter().find(|i| i.id == derived.id).unwrap();
    assert!(item.qualifications.iter().any(|q| q.contains("derived")));
    let item = verification.iter().find(|i| i.id == unknown.id).unwrap();
    assert_eq!(item.lifecycle, "unknown");
    assert!(item.qualifications.iter().any(|q| q.contains("adoption")));
    assert_citations_resolve(&conn, &result);
}

#[test]
fn reported_outcomes_and_proposals_retain_their_kind_in_human_output() {
    let conn = database();
    let report = record(
        &conn,
        "deployment-report.md",
        "reported_outcome",
        "completed",
        "Dispatch retries were deployed to all workers.",
        SourceMaterial::Primary,
    );
    let proposal = record(
        &conn,
        "future-option.md",
        "proposal",
        "proposed",
        "Dispatch could use a new queue for future retries.",
        SourceMaterial::Primary,
    );
    let design = record(
        &conn,
        "design.md",
        "design",
        "active",
        "Dispatch uses a dedicated retry worker in the documented design.",
        SourceMaterial::Primary,
    );
    storage::refresh_knowledge(&conn, "p").unwrap();
    let result = build(&conn, "Change dispatch", &[], 20_000);
    let report_item = result
        .sections
        .historical
        .iter()
        .find(|i| i.id == report.id)
        .unwrap();
    assert_eq!(
        report_item.documentary_basis,
        "source_report_not_independent_verification"
    );
    assert!(
        report_item
            .qualifications
            .iter()
            .any(|q| q.contains("source report") && q.contains("not independent verification"))
    );
    assert!(
        result
            .sections
            .proposals
            .iter()
            .any(|i| i.id == proposal.id)
    );
    assert!(
        result
            .sections
            .current_designs
            .iter()
            .any(|i| i.id == design.id)
    );
    let human = context::render_context(&result);
    for item in result.sections.items() {
        let line = human
            .lines()
            .find(|line| line.contains(&item.statement))
            .unwrap();
        assert!(
            line.contains(&item.kind),
            "kind missing from human guidance: {line}"
        );
        assert!(
            line.contains(&item.lifecycle),
            "lifecycle missing from human guidance: {line}"
        );
    }
    assert_citations_resolve(&conn, &result);
}

#[test]
fn conflict_and_supersession_keep_the_complete_recorded_chain() {
    let conn = database();
    let old = record(
        &conn,
        "ADR-010.md",
        "decision",
        "accepted",
        "Dispatch previously selected constant retry intervals.",
        SourceMaterial::Primary,
    );
    let new = record(
        &conn,
        "ADR-020.md",
        "decision",
        "accepted",
        "Dispatch selects exponential backoff and replaces ADR-010.",
        SourceMaterial::Primary,
    );
    let conflicting = record(
        &conn,
        "latency-policy.md",
        "constraint",
        "active",
        "Dispatch forbids increasing retry delays.",
        SourceMaterial::Primary,
    );
    relation(&conn, &new, &old, "supersedes");
    relation(&conn, &conflicting, &new, "contradicts");
    storage::review(
        &conn,
        "p",
        &format!("conflict:{}:{}", conflicting.assertion, new.id),
        "Resolve the dispatch delay policy disagreement before changing retries.",
    )
    .unwrap();
    storage::refresh_knowledge(&conn, "p").unwrap();
    let result = build(&conn, "Implement exponential backoff", &[], 20_000);
    assert_eq!(
        ids(&result),
        [old.id.clone(), new.id.clone(), conflicting.id.clone()].into()
    );
    assert_eq!(result.relations.len(), 2);
    assert_eq!(result.reviews.len(), 1);
    assert!(result.sections.constraints.is_empty());
    assert!(result.sections.decisions.is_empty());
    assert!(
        result
            .sections
            .historical
            .iter()
            .any(|i| i.id == old.id && i.lifecycle == "superseded")
    );
    assert!(
        result
            .sections
            .needs_verification
            .iter()
            .any(|i| i.id == new.id)
    );
    assert!(
        result
            .sections
            .needs_verification
            .iter()
            .any(|i| i.id == conflicting.id)
    );
    assert_eq!(result.omissions.critical_groups, 0);
    assert_citations_resolve(&conn, &result);
}

#[test]
fn withdrawn_replacement_does_not_revive_an_accepted_predecessor() {
    let conn = database();
    let old = record(
        &conn,
        "ADR-010.md",
        "decision",
        "accepted",
        "Dispatch uses fixed retry intervals.",
        SourceMaterial::Primary,
    );
    let new = record(
        &conn,
        "ADR-020.md",
        "decision",
        "accepted",
        "Dispatch uses exponential retry delays and replaces ADR-010.",
        SourceMaterial::Primary,
    );
    relation(&conn, &new, &old, "supersedes");
    storage::refresh_knowledge(&conn, "p").unwrap();
    storage::retire_source(&conn, &new.source).unwrap();
    storage::refresh_knowledge(&conn, "p").unwrap();
    let result = build(&conn, "Change dispatch", &[], 20_000);
    assert!(result.sections.decisions.is_empty());
    let predecessor = result
        .sections
        .needs_verification
        .iter()
        .find(|i| i.id == old.id)
        .unwrap();
    assert_eq!(predecessor.support_state, "needs_review");
    assert!(
        predecessor
            .qualifications
            .iter()
            .any(|q| q.contains("does not revive"))
    );
    assert!(result.sections.historical.iter().any(|i| i.id == new.id));
    assert!(
        result
            .relations
            .iter()
            .any(|r| r.kind == "supersedes" && !r.current)
    );
    assert_eq!(ids(&result).len(), 2);
    assert_citations_resolve(&conn, &result);
}

#[test]
fn tight_budget_omits_whole_conflict_group_and_reports_it() {
    let conn = database();
    let first = record(
        &conn,
        "policy-a.md",
        "constraint",
        "active",
        &format!(
            "Dispatch must apply the conservative policy. {}",
            "Boundary detail must be preserved. ".repeat(50)
        ),
        SourceMaterial::Primary,
    );
    let second = record(
        &conn,
        "policy-b.md",
        "constraint",
        "active",
        &format!(
            "Dispatch must apply the opposing policy. {}",
            "Another boundary detail must be preserved. ".repeat(50)
        ),
        SourceMaterial::Primary,
    );
    relation(&conn, &second, &first, "contradicts");
    storage::refresh_knowledge(&conn, "p").unwrap();
    let full = build(&conn, "Change dispatch", &[], 20_000);
    assert_eq!(ids(&full).len(), 2);
    let small = build(&conn, "Change dispatch", &[], 512);
    assert!(small.empty);
    assert!(ids(&small).is_empty());
    assert!(small.evidence.is_empty());
    assert!(small.relations.is_empty());
    assert_eq!(small.omissions.knowledge_units, 2);
    assert_eq!(small.omissions.critical_groups, 1);
    assert!(
        small
            .warnings
            .iter()
            .any(|w| w.contains("incomplete for a decision"))
    );
    assert!(small.budget.used_tokens <= 512);
    assert_citations_resolve(&conn, &small);
}

#[test]
fn unicode_escaped_sources_and_special_token_spellings_obey_both_output_budgets() {
    let conn = database();
    let source = "Dispatch observations: \"quoted\" and \\backslash\tÆøå 日本語 🔬.\nLiteral <|endoftext|> and <|fim_prefix|> are source text.\nA terminal marker \u{1b}[31m must remain evidence, not terminal formatting. Inspect `src/transport/worker.rs`.";
    record(
        &conn,
        "unicode.md",
        "design",
        "active",
        source,
        SourceMaterial::Primary,
    );
    storage::refresh_knowledge(&conn, "p").unwrap();
    let tokenizer = tiktoken_rs::cl100k_base().unwrap();
    let mut included_complete_record = false;
    for budget in [512, 1_000, 3_000, 6_000] {
        let result = build(&conn, "Change dispatch 日本語 🔬", &[], budget);
        let machine = serde_json::to_string(&result).unwrap() + "\n";
        let human = context::render_context(&result);
        let measured = tokenizer
            .encode_ordinary(&machine)
            .len()
            .max(tokenizer.encode_ordinary(&human).len());
        assert!(measured <= result.budget.used_tokens);
        assert!(result.budget.used_tokens <= budget);
        assert!(!human.contains('\u{1b}'));
        for item in result.sections.items() {
            assert_eq!(item.statement, source);
            included_complete_record = true;
        }
        for evidence in &result.evidence {
            assert_eq!(evidence.excerpt, source);
        }
        assert_citations_resolve(&conn, &result);
    }
    assert!(included_complete_record);
}

#[test]
fn inspection_suggestions_require_literal_source_evidence() {
    let conn = database();
    record(
        &conn,
        "architecture/dispatch.md",
        "design",
        "active",
        "Dispatch runs in `src/transport/worker.rs`; investigate the original implementation before changes.",
        SourceMaterial::Primary,
    );
    storage::refresh_knowledge(&conn, "p").unwrap();
    let result = build(
        &conn,
        "Change dispatch",
        &["src/imagined/guessed.rs"],
        6_000,
    );
    assert!(
        result
            .suggested_inspection
            .iter()
            .any(|p| p.path == "architecture/dispatch.md" && p.basis == "source_record")
    );
    assert!(
        result
            .suggested_inspection
            .iter()
            .any(|p| p.path == "src/transport/worker.rs" && p.basis == "mentioned_in_evidence")
    );
    assert!(
        !result
            .suggested_inspection
            .iter()
            .any(|p| p.path == "src/imagined/guessed.rs")
    );
    for inspection in &result.suggested_inspection {
        let snapshot = storage::evidence_snapshot(&conn, &inspection.evidence_id).unwrap();
        assert_eq!(inspection.root_id, snapshot.root_id);
        if inspection.basis == "source_record" {
            assert_eq!(inspection.path, snapshot.path);
        } else {
            assert!(snapshot.excerpt.contains(&inspection.path));
        }
    }
    assert_citations_resolve(&conn, &result);
}

#[test]
fn complete_queries_are_deterministic_and_never_write_the_registry() {
    let conn = database();
    record(
        &conn,
        "policy.md",
        "constraint",
        "active",
        "Dispatch preserves idempotency.",
        SourceMaterial::Primary,
    );
    storage::refresh_knowledge(&conn, "p").unwrap();
    let before: i64 = conn
        .query_row("SELECT total_changes()", [], |r| r.get(0))
        .unwrap();
    let first = build(&conn, "Change dispatch", &[], 3_000);
    let second = build(&conn, "Change dispatch", &[], 3_000);
    assert_eq!(
        serde_json::to_string(&first).unwrap(),
        serde_json::to_string(&second).unwrap()
    );
    assert_eq!(
        before,
        conn.query_row::<i64, _, _>("SELECT total_changes()", [], |r| r.get(0))
            .unwrap()
    );
    assert_citations_resolve(&conn, &first);
}
