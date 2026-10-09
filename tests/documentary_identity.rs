mod common;
use common::*;
use lore::{
    engine::{self, UpdateOptions},
    inference::{
        GenerationRequest, GenerationResponse, GenerativeModel, ModelDescriptor, ModelFuture,
    },
    storage,
};
use serde_json::{Value, json};

/// Simulates a real extractor treating accepted document metadata as a second
/// decision, and a reconciler quoting a heading together with its exact
/// substantive source passage. The subject is organizational approval, not an
/// architecture or Atlas-specific decision.
struct MetadataAndHeadingModel {
    inner: FakeModel,
}
impl GenerativeModel for MetadataAndHeadingModel {
    fn descriptor(&self) -> &ModelDescriptor {
        self.inner.descriptor()
    }
    fn generate<'a>(&'a self, req: &'a GenerationRequest) -> ModelFuture<'a, GenerationResponse> {
        Box::pin(async move {
            let input: Value = serde_json::from_str(&req.input).unwrap();
            let mut response = self.inner.generate(req).await?;
            match input["task"].as_str().unwrap() {
                "extract"
                    if input["source"]
                        .as_str()
                        .unwrap_or("")
                        .ends_with("POL-017.md")
                        && input["text"]
                            .as_str()
                            .unwrap_or("")
                            .contains("Status: Accepted") =>
                {
                    let mut extracted: Value = serde_json::from_str(&response.text).unwrap();
                    extracted["assertions"].as_array_mut().unwrap().push(json!({
                        "topic":"policy", "topic_title":"Approval policy",
                        "subject":"Policy POL-017 metadata", "statement":"Policy POL-017 has status Accepted.",
                        "kind":"decision", "lifecycle":"accepted", "scope":"organization",
                        "effective_at":"", "quote":"Status: Accepted"
                    }));
                    response.text = extracted.to_string();
                }
                "reconcile"
                    if input["assertion"]["statement"]
                        .as_str()
                        .unwrap_or("")
                        .contains("POL-021 supersedes POL-017") =>
                {
                    let predecessor = input["candidates"].as_array().unwrap().iter().find(|c| {
                        c["statement"]
                            .as_str()
                            .unwrap_or("")
                            .contains("board adopted two-person approval")
                    });
                    // The proposed relationship quote starts with a Markdown
                    // heading, so it is longer than the captured assertion.
                    let quote = format!(
                        "## Decision\n{}\n",
                        input["assertion"]["quote"].as_str().unwrap()
                    );
                    response.text = json!({
                        "equivalent_to":"", "uncertain":false,
                        "relations": predecessor.map(|unit| vec![json!({
                            "target_id":unit["id"], "kind":"supersedes", "quote":quote,
                            "reason":"The policy explicitly replaces the predecessor."
                        })]).unwrap_or_default()
                    })
                    .to_string();
                }
                _ => {}
            }
            Ok(response)
        })
    }
}

#[tokio::test]
async fn accepted_document_header_is_not_an_ambiguous_predecessor_and_heading_quotes_work() {
    let (_temp, cfg, _) = project();
    let model = MetadataAndHeadingModel {
        inner: FakeModel::new(),
    };
    put(
        &cfg,
        "policies/POL-017.md",
        "# POL-017: Approvals\n\nStatus: Accepted\n\n## Decision\nDECISION policy: The board adopted two-person approval for privileged access.\n",
    );
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();

    put(
        &cfg,
        "notes/committee-review.md",
        "DECISION policy: The committee reaffirmed POL-017 and retained two-person approvals.\n",
    );
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();

    put(
        &cfg,
        "policies/POL-021.md",
        "# POL-021: Access changes\n\nStatus: Accepted\n\n## Decision\nDECISION policy: POL-021 supersedes POL-017 and selects three-person approvals for privileged access.\n",
    );
    let report = engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    assert!(report.degraded_topics.is_empty());
    assert!(!report.degraded_overview);

    let conn = storage::read_only(&cfg.state.join("state.db")).unwrap();
    let relations = storage::relations(&conn).unwrap();
    assert_eq!(
        relations
            .iter()
            .filter(|r| r.active && r.kind == "reaffirms")
            .count(),
        1
    );
    assert_eq!(
        relations
            .iter()
            .filter(|r| r.active && r.kind == "supersedes")
            .count(),
        1
    );
    let units = storage::views(&conn).unwrap();
    let metadata = units
        .iter()
        .find(|u| u.statement.contains("has status Accepted"))
        .unwrap();
    assert!(!relations.iter().any(|r| r.to == metadata.id));
    let prior = units
        .iter()
        .find(|u| u.statement.contains("board adopted two-person approval"))
        .unwrap();
    assert_eq!(prior.lifecycle, "superseded");
    assert!(
        relations
            .iter()
            .any(|r| r.to == prior.id && r.kind == "supersedes")
    );
    let ambiguous: i64 = conn.query_row(
        "SELECT COUNT(*) FROM review_items WHERE status='pending' AND reason LIKE 'An explicit reference to %matches multiple accepted%'",
        [], |r| r.get(0),
    ).unwrap();
    assert_eq!(ambiguous, 0);
    drop(conn);

    let no_op = engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    assert!(no_op.no_op);
    assert_eq!(no_op.model_calls, 0);
}

#[tokio::test]
async fn a_document_with_two_substantive_decisions_stays_ambiguous_despite_a_status_header() {
    let (_temp, cfg, _) = project();
    let model = MetadataAndHeadingModel {
        inner: FakeModel::new(),
    };
    put(
        &cfg,
        "policies/POL-017.md",
        "# POL-017: Access governance\n\nStatus: Accepted\n\n## Decision\n\
         DECISION policy: The board adopted two-person approval for privileged access.\n\
         DECISION audit: The board adopted audit review for privileged actions.\n",
    );
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    put(
        &cfg,
        "policies/POL-021.md",
        "# POL-021: Access changes\n\nStatus: Accepted\n\n## Decision\n\
         DECISION policy: POL-021 supersedes POL-017 and selects three-person approvals for privileged access.\n",
    );
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let conn = storage::read_only(&cfg.state.join("state.db")).unwrap();
    assert!(
        !storage::relations(&conn)
            .unwrap()
            .iter()
            .any(|r| r.active && r.kind == "supersedes")
    );
    let ambiguous: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM review_items WHERE status='pending' \
             AND reason LIKE 'An explicit reference to %matches multiple accepted%'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(ambiguous > 0);
}
