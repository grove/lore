//! Offline regressions through the existing registry, reconciliation and CLI.
mod common;

use lore::{
    context::{self, ContextOptions},
    engine::{self, UpdateOptions},
    inference::{
        GenerationRequest, GenerationResponse, GenerativeModel, ModelDescriptor, ModelFuture,
    },
    insights, reviews, storage, util,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::process::Command;

#[derive(Deserialize)]
struct Case {
    name: String,
    before: [String; 4],
    after: [String; 4],
    suggested: String,
    relation: Option<String>,
    old_lifecycle: String,
}

struct Model<'a> {
    case: &'a Case,
    delegate: common::FakeModel,
}

impl GenerativeModel for Model<'_> {
    fn descriptor(&self) -> &ModelDescriptor {
        self.delegate.descriptor()
    }

    fn generate<'a>(
        &'a self,
        request: &'a GenerationRequest,
    ) -> ModelFuture<'a, GenerationResponse> {
        Box::pin(async move {
            let input: Value = serde_json::from_str(&request.input).unwrap();
            let output = match input["task"].as_str().unwrap() {
                "extract" => {
                    let text = input["text"].as_str().unwrap();
                    let [quote, kind, lifecycle, scope] = [&self.case.before, &self.case.after]
                        .into_iter()
                        .find(|record| text.contains(&record[0]))
                        .unwrap();
                    json!({"assertions":[{
                        "topic":"releases", "topic_title":"Releases", "subject":"release signatures",
                        "statement":quote, "quote":quote, "kind":kind, "lifecycle":lifecycle,
                        "scope":scope, "effective_at":""
                    }]})
                }
                "reconcile" => json!({
                    "equivalent_to":"", "uncertain":false,
                    "relations":input["candidates"].as_array().unwrap().iter().map(|candidate| json!({
                        "target_id":candidate["id"], "kind":self.case.suggested,
                        "quote":input["assertion"]["quote"], "reason":"Deliberately overconfident fixture judgment."
                    })).collect::<Vec<_>>()
                }),
                _ => return self.delegate.generate(request).await,
            };
            Ok(GenerationResponse {
                usage: None,
                model: self.descriptor().model.clone(),
                text: output.to_string(),
            })
        })
    }
}

#[tokio::test]
async fn documentary_applicability_matrix_preserves_source_roles_and_conflict_endpoints() {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("fixtures/decision-applicability/matrix.json")).unwrap();
    for case in cases {
        let (_dir, config, delegate) = common::project();
        let model = Model {
            case: &case,
            delegate,
        };
        common::put(
            &config,
            "ADR-001.md",
            &format!("# Release policy\n\n{}\n", case.before[0]),
        );
        engine::update(&config, &model, None, UpdateOptions::default())
            .await
            .unwrap();
        let first = storage::views(&storage::read_only(&config.state.join("state.db")).unwrap())
            .unwrap()
            .remove(0);
        common::put(
            &config,
            "ADR-002.md",
            &format!("# Release policy\n\n{}\n", case.after[0]),
        );
        engine::update(&config, &model, None, UpdateOptions::default())
            .await
            .unwrap();
        let conn = storage::read_only(&config.state.join("state.db")).unwrap();
        let views = storage::views(&conn).unwrap();
        assert_eq!(views.len(), 2, "{}", case.name);
        let old = views.iter().find(|view| view.id == first.id).unwrap();
        let new = views.iter().find(|view| view.id != first.id).unwrap();
        assert_eq!(old.lifecycle, case.old_lifecycle, "{}", case.name);
        assert_eq!(
            old.evidence[0].source_revision_id,
            first.evidence[0].source_revision_id
        );
        for (view, record) in [(old, &case.before), (new, &case.after)] {
            assert_eq!(view.statement, record[0]);
            assert_eq!(view.kind, record[1]);
            assert_eq!(view.base_lifecycle, record[2]);
            assert_eq!(view.scope, record[3]);
            assert_eq!(view.evidence.len(), 1);
            let evidence = &view.evidence[0];
            assert!(evidence.active);
            let snapshot = storage::evidence_snapshot(&conn, &evidence.id).unwrap();
            assert_eq!(snapshot.excerpt, record[0]);
            assert_eq!(snapshot.digest, util::digest(&record[0]));
            assert_eq!(snapshot.source_revision_id, evidence.source_revision_id);
            assert_eq!(snapshot.material, lore::domain::SourceMaterial::Primary);
        }
        assert_eq!(new.lifecycle, case.after[2]);
        let relations = storage::relation_facts(&conn).unwrap();
        let active: Vec<_> = relations
            .iter()
            .filter(|relation| relation.active)
            .collect();
        assert_eq!(
            active.first().map(|relation| relation.kind.as_str()),
            case.relation.as_deref(),
            "{}",
            case.name
        );
        assert_eq!(active.len(), usize::from(case.relation.is_some()));
        let lenses = insights::decisions(&conn, "release signatures", 24_000).unwrap();
        assert_eq!(lenses.model_calls, 0);
        if let Some(lens) = lenses.lenses.iter().find(|lens| lens.record.id == old.id) {
            assert_eq!(lens.original_lifecycle, case.before[2]);
        }
        let conflicts = reviews::list(&conn, true)
            .unwrap()
            .into_iter()
            .filter(|review| review.category.as_deref() == Some("conflict"))
            .count();
        assert_eq!(
            conflicts,
            usize::from(case.relation.as_deref() == Some("contradicts")),
            "{}",
            case.name
        );
        if matches!(case.relation.as_deref(), Some("contradicts" | "supersedes")) {
            let mut omitted = false;
            for max_tokens in [16_000, 4_000, 2_000, 1_000] {
                let result = context::build_context(
                    &conn,
                    &ContextOptions {
                        task: "release signatures".into(),
                        paths: vec![],
                        max_tokens,
                    },
                )
                .unwrap();
                let retained: Vec<_> = result
                    .evidence
                    .iter()
                    .filter(|evidence| {
                        evidence.id == old.evidence[0].id || evidence.id == new.evidence[0].id
                    })
                    .collect();
                assert!(
                    retained.len() == 2 || retained.is_empty(),
                    "{}: partial conflict at {max_tokens}",
                    case.name
                );
                if max_tokens == 16_000 {
                    assert_eq!(
                        retained.len(),
                        2,
                        "{}: complete evidence must fit",
                        case.name
                    );
                }
                if retained.is_empty() {
                    omitted = true;
                    assert!(result.omissions.knowledge_units > 0);
                } else {
                    assert!(
                        result
                            .relations
                            .iter()
                            .any(|relation| relation.from == new.id
                                && relation.to == old.id
                                && Some(&relation.kind) == case.relation.as_ref())
                    );
                }
            }
            assert!(omitted, "{}: exercise whole-group compression", case.name);
            for view in [old, new] {
                let output = Command::new(env!("CARGO_BIN_EXE_lore"))
                    .arg("--config")
                    .arg(&config.config_path)
                    .args(["--json", "evidence", &view.evidence[0].id])
                    .output()
                    .unwrap();
                assert!(
                    output.status.success(),
                    "{}",
                    String::from_utf8_lossy(&output.stdout)
                );
                assert!(String::from_utf8_lossy(&output.stdout).contains(&view.statement));
            }
            let output = Command::new(env!("CARGO_BIN_EXE_lore"))
                .arg("--config")
                .arg(&config.config_path)
                .args(["--json", "review"])
                .output()
                .unwrap();
            assert!(output.status.success());
            if conflicts > 0 {
                let review = reviews::list(&conn, true)
                    .unwrap()
                    .into_iter()
                    .find(|review| review.category.as_deref() == Some("conflict"))
                    .unwrap();
                assert!(String::from_utf8_lossy(&output.stdout).contains(&review.id));
            }
        }
        assert_eq!(engine::audit(&config).unwrap()["ok"], true, "{}", case.name);
    }
}

#[tokio::test]
async fn a_b_a_keeps_distinct_history_and_requires_explicit_replacement() {
    for explicit in [false, true] {
        let (_dir, config, model) = common::project();
        let first = "DECISION database: MySQL is the selected database.\n";
        let second =
            "DECISION database: ADR-002 replaces ADR-001: PostgreSQL is the selected database.\n";
        let third = if explicit {
            "DECISION database: ADR-003 supersedes ADR-002: MySQL is the selected database.\n"
        } else {
            first
        };
        let mut ids = Vec::new();
        for (file, text) in [
            ("ADR-001.md", first),
            ("ADR-002.md", second),
            ("ADR-003.md", third),
        ] {
            common::put(&config, file, text);
            engine::update(&config, &model, None, UpdateOptions::default())
                .await
                .unwrap();
            let conn = storage::read_only(&config.state.join("state.db")).unwrap();
            let views = storage::views(&conn).unwrap();
            let view = views
                .iter()
                .find(|view| view.evidence.iter().any(|e| e.source.ends_with(file)))
                .unwrap();
            ids.push(view.id.clone());
            assert_eq!(
                views.len(),
                ids.len(),
                "a return to A is a new decision event"
            );
        }
        let conn = storage::read_only(&config.state.join("state.db")).unwrap();
        let views = storage::views(&conn).unwrap();
        for (index, id) in ids.iter().enumerate() {
            let view = views.iter().find(|view| &view.id == id).unwrap();
            assert_eq!(view.base_lifecycle, "accepted");
            assert_eq!(
                view.lifecycle,
                if index == 0 || (index == 1 && explicit) {
                    "superseded"
                } else {
                    "accepted"
                }
            );
            assert_eq!(view.scope, "production");
            assert!(view.evidence.iter().all(|e| e.active));
        }
        let relations = storage::relation_facts(&conn).unwrap();
        assert_eq!(
            relations
                .iter()
                .filter(|r| r.active && r.kind == "supersedes")
                .count(),
            if explicit { 2 } else { 1 }
        );
        for relation in relations
            .iter()
            .filter(|r| r.active && r.kind == "supersedes")
        {
            let evidence = storage::evidence_snapshot(&conn, &relation.evidence_id).unwrap();
            assert!(
                evidence.excerpt.contains("replaces ADR-001")
                    || evidence.excerpt.contains("supersedes ADR-002")
            );
        }
        let lenses = insights::decisions(&conn, "database", 32_000).unwrap();
        for lens in &lenses.lenses {
            assert_eq!(lens.original_lifecycle, "accepted");
        }
        let again = engine::update(&config, &model, None, UpdateOptions::default())
            .await
            .unwrap();
        assert!(again.no_op);
        assert_eq!(again.model_calls, 0);
    }
}
