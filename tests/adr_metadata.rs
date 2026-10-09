mod common;
use common::*;
use lore::{
    engine::{self, UpdateOptions},
    inference::{GenerativeModel, GenerationRequest, GenerationResponse, ModelDescriptor, ModelFuture},
    sources,
    storage,
};
use serde_json::{Value, json};
use std::fs;

struct UnknownDecisionLifecycle {
    inner: FakeModel,
}
impl UnknownDecisionLifecycle {
    fn new() -> Self {
        Self { inner: FakeModel::new() }
    }
}
impl GenerativeModel for UnknownDecisionLifecycle {
    fn descriptor(&self) -> &ModelDescriptor {
        self.inner.descriptor()
    }
    fn generate<'a>(&'a self, request: &'a GenerationRequest) -> ModelFuture<'a, GenerationResponse> {
        Box::pin(async move {
            let mut response = self.inner.generate(request).await?;
            let input: Value = serde_json::from_str(&request.input).unwrap();
            if input["task"] == "extract"
                && input["source"].as_str().unwrap_or("").ends_with("ADR-027.md")
                && input["heading_path"].as_array()
                    .and_then(|a| a.last())
                    .and_then(Value::as_str) == Some("Decision")
            {
                // Reproduces the uploaded Foundry result: the substantive
                // PostgreSQL replacement is classified as an unknown
                // lifecycle despite Status: Accepted in the same ADR.
                let mut extraction: Value = serde_json::from_str(&response.text).unwrap();
                for assertion in extraction["assertions"].as_array_mut().unwrap() {
                    if assertion["kind"] == "decision" {
                        assertion["lifecycle"] = json!("unknown");
                    }
                }
                response.text = extraction.to_string();
            }
            Ok(response)
        })
    }
}

#[tokio::test]
async fn accepted_adr_header_qualifies_unknown_decision_and_records_supersession() {
    let (_dir, cfg, _) = project();
    let model = UnknownDecisionLifecycle::new();
    put(
        &cfg,
        "decisions/ADR-001.md",
        "# ADR-001: Ledger database\nStatus: Accepted\n\n## Decision\nDECISION ledger: MySQL is the selected database for the production ledger.\n",
    );
    engine::update(&cfg, &model, None, UpdateOptions::default()).await.unwrap();
    put(
        &cfg,
        "decisions/ADR-027.md",
        "# ADR-027: Replace production ledger database\nStatus: Accepted\nDate: 2026-07-01\nScope: production ledger\n\n## Decision\nDECISION ledger: ADR-027 explicitly supersedes ADR-001 for the production ledger: PostgreSQL replaces MySQL as the selected architecture, but is not an independently verified deployment.\n\n## Implementation plan\nPLAN migration: Production traffic must not move until required checks have passed.\n",
    );
    let changed = engine::update(&cfg, &model, None, UpdateOptions::default()).await.unwrap();
    assert!(!changed.no_op);
    assert!(changed.warnings.iter().any(|w| w.contains("Applied explicit Status: Accepted metadata")));
    let conn = storage::read_only(&cfg.state.join("state.db")).unwrap();
    let all = storage::views(&conn).unwrap();
    let replacement = all.iter()
        .find(|v| v.statement.contains("ADR-027 explicitly supersedes ADR-001"))
        .expect("replacement knowledge");
    assert_eq!(replacement.kind, "decision");
    assert_eq!(replacement.base_lifecycle, "accepted");
    assert_eq!(replacement.lifecycle, "accepted");
    let old = all.iter().find(|v| v.statement.contains("MySQL is the selected database")).unwrap();
    assert_eq!(old.lifecycle, "superseded");
    let relation = storage::relations(&conn).unwrap();
    assert!(relation.iter().any(|r| r.active && r.kind == "supersedes" && r.from == replacement.id && r.to == old.id));
    let plan = all.iter().find(|v| v.statement.contains("Production traffic must not move")).unwrap();
    assert_ne!(plan.kind, "decision");
    assert_ne!(plan.lifecycle, "accepted");
    drop(conn);
    let index = fs::read_to_string(cfg.wiki.join("index.md")).unwrap();
    assert!(index.contains("explicitly supersedes"));
    let noop = engine::update(&cfg, &model, None, UpdateOptions::default()).await.unwrap();
    assert!(noop.no_op);
    assert_eq!(noop.model_calls, 0);
}

#[test]
fn changing_formal_adr_status_invalidates_only_its_decision_context() {
    let (_dir, cfg, _) = project();
    let status_accepted = "# ADR-027: Ledger database\nStatus: Accepted\n\n## Decision\nThe selected database is PostgreSQL.\n\n## Implementation plan\nMigration requires review.\n";
    put(&cfg, "decisions/ADR-027.md", status_accepted);
    let first = sources::scan(&cfg).unwrap().documents.remove(0);
    let old = first.chunks.iter().find(|c| c.heading == "Decision").unwrap();
    assert!(sources::explicitly_accepted_adr_decision(&first, old));
    let old_digest = old.input_digest.clone();
    let old_plan = first.chunks.iter().find(|c| c.heading == "Implementation plan").unwrap().input_digest.clone();
    put(&cfg, "decisions/ADR-027.md", &status_accepted.replace("Status: Accepted", "Status: Proposed"));
    let second = sources::scan(&cfg).unwrap().documents.remove(0);
    let newer = second.chunks.iter().find(|c| c.heading == "Decision").unwrap();
    assert!(!sources::explicitly_accepted_adr_decision(&second, newer));
    assert_ne!(old_digest, newer.input_digest, "the status changed, so the decision must be re-extracted");
    let unchanged_plan = second.chunks.iter().find(|c| c.heading == "Implementation plan").unwrap();
    assert_eq!(old_plan, unchanged_plan.input_digest);
}

#[test]
fn accepted_status_from_another_context_never_promotes_a_decision() {
    let (_dir, cfg, _) = project();
    put(&cfg, "misc/meeting.md", "# ADR-027: Meeting\nStatus: Accepted\n\n## Decision\nA candidate has been discussed.\n");
    let doc = sources::scan(&cfg).unwrap().documents.remove(0);
    let section = doc.chunks.iter().find(|c| c.heading == "Decision").unwrap();
    assert!(!sources::explicitly_accepted_adr_decision(&doc, section));

    put(&cfg, "misc/meeting.md", "# Meeting\nStatus: Accepted\n\n## Decision\nA candidate has been discussed.\n");
    let doc = sources::scan(&cfg).unwrap().documents.remove(0);
    let section = doc.chunks.iter().find(|c| c.heading == "Decision").unwrap();
    assert!(!sources::explicitly_accepted_adr_decision(&doc, section));

    put(&cfg, "decisions/ADR-027.md",
        "# ADR-027: Meeting\n> Status: Accepted\n\n## Decision\nA candidate has been discussed.\n");
    let doc = sources::scan(&cfg).unwrap().documents.into_iter()
        .find(|d| d.relative_path.ends_with("ADR-027.md")).unwrap();
    let section = doc.chunks.iter().find(|c| c.heading == "Decision").unwrap();
    assert!(!sources::explicitly_accepted_adr_decision(&doc, section));
}
