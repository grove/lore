mod common;
use common::*;
use lore::{
    engine::{self, UpdateOptions},
    inference::{GenerativeModel, GenerationRequest, GenerationResponse, ModelDescriptor, ModelFuture},
    storage,
};
use serde_json::{Value, json};
use std::fs;

/// A deterministic stand-in for an imperfect real extractor/reconciler.
/// It deliberately paraphrases scopes differently and either mistakes a
/// reaffirmation for elaboration or omits an explicit supersession entirely.
struct ScopeDriftModel {
    delegate: FakeModel,
}
impl ScopeDriftModel {
    fn new() -> Self {
        Self { delegate: FakeModel::new() }
    }
}
impl GenerativeModel for ScopeDriftModel {
    fn descriptor(&self) -> &ModelDescriptor {
        self.delegate.descriptor()
    }
    fn generate<'a>(&'a self, request: &'a GenerationRequest) -> ModelFuture<'a, GenerationResponse> {
        Box::pin(async move {
            let input: Value = serde_json::from_str(&request.input).unwrap();
            let mut response = self.delegate.generate(request).await?;
            match input["task"].as_str().unwrap() {
                "extract" => {
                    let mut parsed: Value = serde_json::from_str(&response.text).unwrap();
                    let source = input["source"].as_str().unwrap();
                    for a in parsed["assertions"].as_array_mut().unwrap() {
                        if a["kind"] != "decision" {continue;}
                        // Exactly the kind of scope drift observed on real Atlas.
                        a["scope"] = if source.ends_with("ADR-001.md") {
                            json!("New ledger persistence work until another accepted ADR")
                        } else {
                            json!("production payments ledger")
                        };
                    }
                    response.text = parsed.to_string();
                }
                "reconcile" => {
                    let assertion=&input["assertion"];
                    let statement=assertion["statement"].as_str().unwrap();
                    if statement.contains("review reaffirmed") {
                        // A model may classify an explicit reaffirmation as
                        // "elaborates"; documentary identity is the stronger cue.
                        let prior=input["candidates"].as_array().unwrap().iter()
                            .find(|c| c["kind"]=="decision"
                                && c["statement"].as_str().unwrap_or("").contains("MySQL was selected"));
                        response.text = json!({
                            "equivalent_to": "",
                            "relations": prior.map(|p| vec![json!({
                                "target_id":p["id"],
                                "kind":"elaborates",
                                "reason":"Historical detail about the original choice.",
                                "quote":assertion["quote"]
                            })]).unwrap_or_default(),
                            "uncertain":false
                        }).to_string();
                    } else if statement.contains("ADR-027 explicitly supersedes") {
                        // The model fails to return any relationship at all.
                        response.text=json!({
                            "equivalent_to":"","relations":[],"uncertain":false
                        }).to_string();
                    }
                }
                _=> {}
            }
            Ok(response)
        })
    }
}

#[tokio::test]
async fn explicit_adr_mentions_reconcile_despite_model_scope_drift_and_missed_links() {
    let (_temp,cfg,_fake)=project();
    let model=ScopeDriftModel::new();
    put(&cfg,"decisions/ADR-001.md",
        "# ADR-001\n\nDECISION ledger: MySQL was selected as the production payments ledger database.\n");
    engine::update(&cfg,&model,None,UpdateOptions::default()).await.unwrap();

    let before=fs::read_to_string(cfg.wiki.join("topics/ledger.md")).unwrap();
    put(&cfg,"notes/architecture-review.md",
        "# Architecture review\n\nDECISION review: The review reaffirmed that the production payments ledger remains committed to the accepted MySQL decision in ADR-001.\n");
    engine::update(&cfg,&model,None,UpdateOptions::default()).await.unwrap();
    let after_review=fs::read_to_string(cfg.wiki.join("topics/ledger.md")).unwrap();
    assert_ne!(before,after_review);
    assert!(after_review.contains("reaffirms"));

    put(&cfg,"decisions/ADR-027.md",
        "# ADR-027\n\nDECISION postgres: ADR-027 explicitly supersedes ADR-001 for the production payments ledger: PostgreSQL replaces MySQL.\n");
    engine::update(&cfg,&model,None,UpdateOptions::default()).await.unwrap();
    let conn=storage::read_only(&cfg.state.join("state.db")).unwrap();
    let relations=storage::relations(&conn).unwrap();
    assert_eq!(relations.iter().filter(|r|r.kind=="reaffirms" && r.active).count(),1);
    assert_eq!(relations.iter().filter(|r|r.kind=="supersedes" && r.active).count(),1);
    assert!(!relations.iter().any(|r|r.kind=="elaborates"));

    let ledger=fs::read_to_string(cfg.wiki.join("topics/ledger.md")).unwrap();
    assert!(ledger.contains("explicitly supersedes"));
    assert!(ledger.contains("reaffirms"));
    let index=fs::read_to_string(cfg.wiki.join("index.md")).unwrap();
    assert!(index.contains("explicitly supersedes"));
    assert!(index.contains("reaffirms"));
    let original=storage::views(&conn).unwrap().into_iter()
        .find(|v|v.topic=="ledger").unwrap();
    assert_eq!(original.lifecycle,"superseded");
    drop(conn);
    let result=engine::update(&cfg,&model,None,UpdateOptions::default()).await.unwrap();
    assert!(result.no_op);
    assert_eq!(result.model_calls,0);
}

#[tokio::test]
async fn ambiguous_predecessors_and_negated_mentions_are_not_auto_linked() {
    let (_temp,cfg,fake)=project();
    put(&cfg,"decisions/ADR-001.md",
        "DECISION one: MySQL was selected as the primary database.\n\
         DECISION two: Replication will use the primary database.\n");
    engine::update(&cfg,&fake,None,UpdateOptions::default()).await.unwrap();
    put(&cfg,"decisions/ADR-027.md",
        "DECISION successor: ADR-027 explicitly supersedes ADR-001 for the database: PostgreSQL replaces ADR-001 and selects MySQL's successor.\n");
    engine::update(&cfg,&fake,None,UpdateOptions::default()).await.unwrap();
    let conn=storage::read_only(&cfg.state.join("state.db")).unwrap();
    let relations=storage::relations(&conn).unwrap();
    assert!(!relations.iter().any(|r|r.kind=="supersedes"));
    let pending: i64=conn.query_row("SELECT COUNT(*) FROM review_items WHERE status='pending' AND reason LIKE 'An explicit reference to %'",[],|r|r.get(0)).unwrap();
    assert!(pending>0);
    drop(conn);

    put(&cfg,"decisions/ADR-028.md",
        "DECISION refused: ADR-028 does not supersede ADR-001 for the database.\n");
    put(&cfg,"decisions/ADR-029.md",
        "DECISION conditional: Until another accepted ADR explicitly replaces ADR-001, MySQL remains selected.\n");
    engine::update(&cfg,&fake,None,UpdateOptions::default()).await.unwrap();
    let conn=storage::read_only(&cfg.state.join("state.db")).unwrap();
    assert!(!storage::relations(&conn).unwrap().iter().any(|r|r.kind=="supersedes"));
}

