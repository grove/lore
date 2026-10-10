//! Fast exact-source regression replay; outcome scoring remains in the comparator.
mod common;

use lore::{
    knowledge::{self, ExploreOptions},
    storage,
};
use rusqlite::Connection;
use std::fs;

fn assert_compact_budget(result: &knowledge::CompactExploreResult) {
    let actual = lore::context::count_tokens(&(serde_json::to_string(result).unwrap() + "\n")).max(
        lore::context::count_tokens(&knowledge::render_compact_markdown(result).unwrap()),
    );
    assert!(
        actual <= result.used_tokens,
        "{actual} > {}",
        result.used_tokens
    );
    assert!(result.used_tokens <= result.max_tokens);
}

fn assert_compact_originals(conn: &Connection, result: &knowledge::CompactExploreResult) {
    assert_compact_budget(result);
    let resolved = result.resolve().unwrap();
    let originals = storage::views(conn).unwrap();
    for record in &resolved.knowledge {
        let original = originals
            .iter()
            .find(|original| original.id == record.id)
            .unwrap();
        assert_eq!(
            serde_json::to_value(record).unwrap(),
            serde_json::to_value(original).unwrap()
        );
    }
    for evidence in &resolved.evidence {
        let original = storage::evidence_snapshot(conn, &evidence.id).unwrap();
        assert_eq!(
            serde_json::to_value(evidence).unwrap(),
            serde_json::to_value(original).unwrap()
        );
    }
    for relation in &resolved.relations {
        assert!(
            resolved
                .knowledge
                .iter()
                .any(|record| record.id == relation.from)
        );
        assert!(
            resolved
                .knowledge
                .iter()
                .any(|record| record.id == relation.to)
        );
        assert!(
            resolved
                .evidence
                .iter()
                .any(|evidence| evidence.id == relation.evidence_id)
        );
    }
}

#[tokio::test]
async fn reviewed_upstream_queries_retain_their_complete_original_at_1500_tokens() {
    use lore::{
        engine::{self, UpdateOptions},
        inference::{
            GenerationRequest, GenerationResponse, GenerativeModel, ModelDescriptor, ModelFuture,
        },
    };
    use serde_json::{Value, json};
    use sha2::{Digest, Sha256};
    use std::path::Path;

    // A quick regression replay of independently authored, source-reviewed
    // documentary cases. It uses the unchanged manifest, query and quotation,
    // and does not claim an extraction-model or coding-outcome measurement.
    struct ExactSourceCapture {
        base: common::FakeModel,
        cases: Vec<Value>,
    }
    impl GenerativeModel for ExactSourceCapture {
        fn descriptor(&self) -> &ModelDescriptor {
            self.base.descriptor()
        }
        fn generate<'a>(
            &'a self,
            request: &'a GenerationRequest,
        ) -> ModelFuture<'a, GenerationResponse> {
            Box::pin(async move {
                let input: Value = serde_json::from_str(&request.input).unwrap();
                if input["task"] != "extract" {
                    return self.base.generate(request).await;
                }
                let text = input["text"].as_str().unwrap();
                let assertions: Vec<_> = self
                    .cases
                    .iter()
                    .filter(|case| text.contains(case["quote"].as_str().unwrap()))
                    .map(|case| {
                        json!({
                            "topic":case["topic"],"topic_title":case["topic"],
                            "subject":case["subject"],"statement":case["statement"],
                            "quote":case["quote"],"kind":case["kind"],
                            "lifecycle":case["lifecycle"],"scope":case["scope"],"effective_at":"",
                        })
                    })
                    .collect();
                Ok(GenerationResponse {
                    usage: None,
                    model: self.descriptor().model.clone(),
                    text: json!({"assertions":assertions}).to_string(),
                })
            })
        }
    }
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("evaluation/corpora/zoom-reviewed-v1");
    let manifest: Value =
        serde_json::from_slice(&fs::read(corpus.join("manifest.json")).unwrap()).unwrap();
    let mut checked = 0;
    for project in manifest["projects"].as_array().unwrap() {
        let (_temp, mut config, base) = common::project();
        config.config.project.name = project["id"].as_str().unwrap().into();
        config.config.sources.roots[0].origin = Some(format!(
            "https://github.com/{}@{}",
            project["repository"].as_str().unwrap(),
            project["commit"].as_str().unwrap()
        ));
        let cases = project["cases"].as_array().unwrap();
        for case in cases {
            let source = fs::read(corpus.join(case["source"].as_str().unwrap())).unwrap();
            assert_eq!(
                format!("{:x}", Sha256::digest(&source)),
                case["excerpt_sha256"].as_str().unwrap()
            );
            let source = String::from_utf8(source).unwrap();
            assert_eq!(source, format!("{}\n", case["quote"].as_str().unwrap()));
            common::put(
                &config,
                &format!("{}.md", case["id"].as_str().unwrap()),
                &source,
            );
        }
        let model = ExactSourceCapture {
            base,
            cases: cases.clone(),
        };
        engine::update(&config, &model, None, UpdateOptions::default())
            .await
            .unwrap();
        let conn = storage::read_only(&config.state.join("state.db")).unwrap();
        let originals = storage::views(&conn).unwrap();
        assert_eq!(originals.len(), cases.len());
        for case in cases {
            let original = originals
                .iter()
                .find(|record| record.statement == case["statement"].as_str().unwrap())
                .unwrap();
            let result = knowledge::explore_compact(
                &conn,
                &ExploreOptions {
                    query: case["query"].as_str().unwrap().into(),
                    max_tokens: 1_500,
                    ..ExploreOptions::default()
                },
            )
            .unwrap();
            assert_compact_originals(&conn, &result);
            let resolved = result.resolve().unwrap();
            assert!(
                resolved
                    .knowledge
                    .iter()
                    .any(|record| record.id == original.id),
                "{}: expected subject {:?}; selected {:?}",
                case["id"],
                original.subject,
                resolved
                    .knowledge
                    .iter()
                    .map(|record| &record.subject)
                    .collect::<Vec<_>>()
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 24);
}
