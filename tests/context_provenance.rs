//! Exercise ordinary multi-root Markdown through update, immutable evidence,
//! and offline context selection, using a deterministic inference fixture.
mod common;
use common::{FakeModel, project};
use lore::{
    config::{ResolvedConfig, SourceRoot},
    context::{self, ContextOptions},
    domain::SourceMaterial,
    engine::{self, UpdateOptions},
    inference::*,
    storage,
};
use serde_json::{Value, json};
use std::{
    fs,
    sync::atomic::{AtomicUsize, Ordering},
};

struct MarkdownModel {
    delegate: FakeModel,
    calls: AtomicUsize,
}
impl MarkdownModel {
    fn new() -> Self {
        Self {
            delegate: FakeModel::new(),
            calls: AtomicUsize::new(0),
        }
    }
}
impl GenerativeModel for MarkdownModel {
    fn descriptor(&self) -> &ModelDescriptor {
        self.delegate.descriptor()
    }
    fn generate<'a>(
        &'a self,
        request: &'a GenerationRequest,
    ) -> ModelFuture<'a, GenerationResponse> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let input: Value = serde_json::from_str(&request.input).unwrap();
            if input["task"] != "extract" {
                return self.delegate.generate(request).await;
            }
            let assertions = input["text"]
                .as_str()
                .unwrap()
                .lines()
                .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
                .map(|line| {
                    json!({
                        "topic":"delivery", "topic_title":"Delivery", "subject":"delivery worker",
                        "statement":line, "kind":"constraint", "lifecycle":"active",
                        "scope":"delivery workers", "effective_at":"", "quote":line,
                    })
                })
                .collect::<Vec<_>>();
            Ok(GenerationResponse {
                model: self.descriptor().model.clone(),
                text: json!({"assertions":assertions}).to_string(),
            })
        })
    }
}

#[tokio::test]
async fn context_keeps_source_identity_and_does_not_promote_derived_only_support() {
    let (_dir, initial, _) = project();
    let model = MarkdownModel::new();
    let text =
        "# Retry policy\nThe delivery worker must keep event identifiers stable across retries.\n";
    fs::write(initial.base.join("docs/policy.md"), text).unwrap();
    fs::create_dir(initial.base.join("openwiki")).unwrap();
    fs::write(initial.base.join("openwiki/policy.md"), text).unwrap();
    let mut config = initial.config.clone();
    config.sources.roots[0].origin = Some("repository:delivery-team".into());
    config.sources.roots.push(SourceRoot {
        id: "generated".into(),
        path: "./openwiki".into(),
        material: SourceMaterial::Derived,
        origin: Some("repository:partner-service".into()),
    });
    let cfg = ResolvedConfig::resolve(config.clone(), &initial.config_path).unwrap();
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let query = ContextOptions {
        task: "Change delivery retries".into(),
        paths: vec!["policy.md".into()],
        max_tokens: 20_000,
    };
    let calls = model.calls.load(Ordering::SeqCst);
    let db = storage::read_only(&cfg.state.join("state.db")).unwrap();
    let initial_context = context::build_context(&db, &query).unwrap();
    assert!(!initial_context.empty);
    assert_eq!(initial_context.model_calls, 0);
    assert_eq!(initial_context.sections.constraints.len(), 1);
    let primary = initial_context
        .evidence
        .iter()
        .find(|e| e.source == "docs:policy.md")
        .unwrap();
    let derived = initial_context
        .evidence
        .iter()
        .find(|e| e.source == "generated:policy.md")
        .unwrap();
    assert_eq!(primary.material, SourceMaterial::Primary);
    assert_eq!(derived.material, SourceMaterial::Derived);
    assert!(primary.provenance_recorded && derived.provenance_recorded);
    assert_eq!(primary.origin.as_deref(), Some("repository:delivery-team"));
    assert_eq!(
        derived.origin.as_deref(),
        Some("repository:partner-service")
    );
    assert_eq!(primary.excerpt, derived.excerpt);
    let old_primary = primary.id.clone();
    let primary_snapshot = storage::evidence_snapshot(&db, &primary.id).unwrap();
    let derived_snapshot = storage::evidence_snapshot(&db, &derived.id).unwrap();
    assert_ne!(primary_snapshot.source_id, derived_snapshot.source_id);
    assert!(
        initial_context
            .suggested_inspection
            .iter()
            .any(|p| p.root_id == "docs" && p.path == "policy.md")
    );
    assert!(
        initial_context
            .suggested_inspection
            .iter()
            .any(|p| p.root_id == "generated" && p.path == "policy.md")
    );
    assert_eq!(model.calls.load(Ordering::SeqCst), calls);
    drop(db);

    // The operator corrects the original root's provenance. No source bytes
    // change, so a digest-only updater would accidentally retain primary support.
    config.sources.roots[0].material = SourceMaterial::Derived;
    let cfg = ResolvedConfig::resolve(config, &initial.config_path).unwrap();
    assert!(
        engine::status(&cfg)
            .unwrap()
            .changed_files
            .contains(&"docs:policy.md".to_owned())
    );
    engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let calls = model.calls.load(Ordering::SeqCst);
    let db = storage::read_only(&cfg.state.join("state.db")).unwrap();
    let result = context::build_context(&db, &query).unwrap();
    assert!(result.sections.constraints.is_empty());
    assert!(result.sections.decisions.is_empty());
    assert_eq!(result.sections.needs_verification.len(), 1);
    assert!(
        result.sections.needs_verification[0]
            .qualifications
            .iter()
            .any(|q| q.contains("derived material only"))
    );
    assert!(
        result
            .evidence
            .iter()
            .any(|e| e.id == old_primary && !e.current && e.material == SourceMaterial::Primary)
    );
    assert!(
        result
            .evidence
            .iter()
            .filter(|e| e.current)
            .all(|e| e.material == SourceMaterial::Derived)
    );
    assert_eq!(
        storage::evidence_snapshot(&db, &old_primary)
            .unwrap()
            .material,
        SourceMaterial::Primary
    );
    for evidence in &result.evidence {
        let snapshot = storage::evidence_snapshot(&db, &evidence.id).unwrap();
        assert_eq!(snapshot.excerpt, evidence.excerpt);
        assert_eq!(snapshot.material, evidence.material);
        assert_eq!(snapshot.origin, evidence.origin);
    }
    let repeated = context::build_context(&db, &query).unwrap();
    assert_eq!(
        serde_json::to_value(&result).unwrap(),
        serde_json::to_value(&repeated).unwrap()
    );
    assert_eq!(result.model_calls, 0);
    assert_eq!(model.calls.load(Ordering::SeqCst), calls);
}
