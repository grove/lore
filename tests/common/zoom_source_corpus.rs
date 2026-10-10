//! Source-pinned gold is authored from upstream documents, independently of the
//! selector. A deterministic extraction adapter only captures those exact quotes.
//! This is not an extraction-model, coding-task or human-outcome experiment.
use super::*;
use lore::inference::{
    GenerationRequest, GenerationResponse, GenerativeModel, ModelDescriptor, ModelFuture,
};
use sha2::{Digest, Sha256};
use std::sync::atomic::AtomicUsize;

struct CaptureModel {
    base: common::FakeModel,
    cases: Vec<Value>,
    calls: AtomicUsize,
}

impl GenerativeModel for CaptureModel {
    fn descriptor(&self) -> &ModelDescriptor {
        self.base.descriptor()
    }
    fn generate<'a>(
        &'a self,
        request: &'a GenerationRequest,
    ) -> ModelFuture<'a, GenerationResponse> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
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
                        "topic":case["topic"],"topic_title":case["topic"],"subject":case["subject"],
                        "statement":case["statement"],"quote":case["quote"],"kind":case["kind"],
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

#[tokio::test]
async fn measured_source_pinned_three_repository_cases() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("evaluation/corpora/zoom-reviewed-v1");
    let manifest_bytes = fs::read(root.join("manifest.json")).unwrap();
    assert!(manifest_bytes.len() <= 1_048_576);
    let manifest: Value = serde_json::from_slice(&manifest_bytes).unwrap();
    assert_eq!(manifest["schema_version"], 1);
    let review_bytes = fs::read(root.join("review.json")).unwrap();
    assert!(review_bytes.len() <= 1_048_576);
    let review: Value = serde_json::from_slice(&review_bytes).unwrap();
    assert_eq!(
        review["manifest_sha256"],
        format!("{:x}", Sha256::digest(&manifest_bytes))
    );
    assert_eq!(review["reviewer"]["authored_retrieval_cases"], false);
    assert_eq!(review["reviewer"]["implemented_retrieval_selector"], false);
    assert_eq!(
        review["reviewer"]["inspected_selector_or_scored_outputs"],
        false
    );
    assert_eq!(review["outcomes"]["unresolved_source_gold_issues"], 0);
    let projects = manifest["projects"].as_array().unwrap();
    assert!(projects.len() >= 3);
    let mut results = Vec::new();
    let mut total_cases = 0;
    for project in projects {
        let commit = project["commit"].as_str().unwrap();
        assert_eq!(commit.len(), 40);
        assert!(commit.bytes().all(|b| b.is_ascii_hexdigit()));
        let gold = project["cases"].as_array().unwrap();
        total_cases += gold.len();
        let (_temp, mut config, base) = common::project();
        config.config.project.name = project["id"].as_str().unwrap().into();
        config.config.sources.roots[0].origin = Some(format!(
            "https://github.com/{}@{}",
            project["repository"].as_str().unwrap(),
            commit
        ));
        for case in gold {
            let relative = case["source"].as_str().unwrap();
            assert!(!relative.contains("..") && !Path::new(relative).is_absolute());
            let source = fs::read(root.join(relative)).unwrap();
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
        let model = CaptureModel {
            base,
            cases: gold.clone(),
            calls: AtomicUsize::new(0),
        };
        engine::update(&config, &model, None, UpdateOptions::default())
            .await
            .unwrap();
        let database = config.state.join("state.db");
        let bytes_before = fs::read(&database).unwrap();
        let conn = storage::read_only(&database).unwrap();
        let originals = original_records(&conn);
        assert_eq!(
            originals.len(),
            gold.len(),
            "every independent source quote must compile exactly once"
        );
        let cases: Vec<_> = gold
            .iter()
            .map(|case| {
                let matching: Vec<_> = originals
                    .values()
                    .filter(|record| record["statement"] == case["statement"])
                    .collect();
                assert_eq!(matching.len(), 1);
                let record = matching[0];
                let id = record["id"].as_str().unwrap().to_owned();
                QueryCase {
                    id: case["id"].as_str().unwrap().into(),
                    query: case["query"].as_str().unwrap().into(),
                    budgets: vec![1500, 8000],
                    expected_knowledge_ids: vec![id.clone()],
                    expected_evidence_ids: record["evidence"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|e| e["id"].as_str().unwrap().into())
                        .collect(),
                    critical_conditions: vec![Condition {
                        knowledge_id: id,
                        statement: case["statement"].as_str().unwrap().into(),
                        kind: case["kind"].as_str().unwrap().into(),
                        lifecycle: case["lifecycle"].as_str().unwrap().into(),
                        scope: case["scope"].as_str().unwrap().into(),
                    }],
                }
            })
            .collect();
        let calls = model.calls.load(Ordering::SeqCst);
        let mut report = comparison(
            &conn,
            &cases,
            "source_pinned_documentary_retention_diagnostic",
        );
        let required: Vec<_> = gold
            .iter()
            .map(|case| case["id"].as_str().unwrap())
            .collect();
        assert_compact_condition_regressions(&report, &required);
        assert_eq!(calls, model.calls.load(Ordering::SeqCst));
        assert_eq!(bytes_before, fs::read(database).unwrap());
        report["upstream_repository"] = project["repository"].clone();
        report["upstream_commit"] = project["commit"].clone();
        report["source_capture"] = project.clone();
        report["fixture_ingestion"] = json!({"model":"exact-source-capture-v1","provider_requests":0,"excluded_ingestion_calls":calls});
        results.push(report);
    }
    assert!(total_cases >= 24);
    let report = json!({"schema_version":1,"classification":manifest["classification"],"manifest_sha256":format!("{:x}",Sha256::digest(&manifest_bytes)),
        "review":review,"review_sha256":format!("{:x}",Sha256::digest(&review_bytes)),"original_manifest_review_status":manifest["review"],"authorship":manifest["authorship"],"case_count":total_cases,"projects":results,"limits":manifest["measurement_limits"]});
    if let Some(path) = std::env::var_os("LORE_ZOOM_REVIEWED_OUTPUT") {
        let path = PathBuf::from(path);
        assert!(path.is_absolute());
        let text = serde_json::to_string_pretty(&report).unwrap() + "\n";
        assert!(text.len() <= 16 * 1024 * 1024);
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .unwrap();
        file.write_all(text.as_bytes()).unwrap();
        file.sync_all().unwrap();
    }
}
