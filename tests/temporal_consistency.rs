mod common;
use common::*;
use lore::{
    engine::{self, UpdateOptions},
    storage,
};
use rusqlite::Connection;
use std::fs;

#[tokio::test]
async fn documentary_decision_timeline_invalidates_related_topics() {
    let (_directory, cfg, fake) = project();
    put(
        &cfg,
        "ADR-001.md",
        "DECISION legacy: MySQL is the selected database.\n",
    );
    engine::update(&cfg, &fake, None, UpdateOptions::default())
        .await
        .unwrap();
    let before = fs::read_to_string(cfg.wiki.join("topics/legacy.md")).unwrap();
    put(
        &cfg,
        "review.md",
        "DECISION reaffirmation: The review reaffirms ADR-001: MySQL remains the selected database.\n",
    );
    engine::update(&cfg, &fake, None, UpdateOptions::default())
        .await
        .unwrap();
    let reassessed = fs::read_to_string(cfg.wiki.join("topics/legacy.md")).unwrap();
    assert_ne!(before, reassessed);
    assert!(reassessed.contains("reaffirms"));
    assert!(reassessed.contains("reaffirmation.md"));

    put(
        &cfg,
        "ADR-027.md",
        "DECISION replacement: PostgreSQL replaces ADR-001 for the production database.\n",
    );
    let update = engine::update(&cfg, &fake, None, UpdateOptions::default())
        .await
        .unwrap();
    assert!(!update.no_op);
    let legacy = fs::read_to_string(cfg.wiki.join("topics/legacy.md")).unwrap();
    assert!(legacy.contains("explicitly supersedes"));
    assert!(legacy.contains("replacement.md"));
    assert!(legacy.contains("not independent verification of deployment"));
    let index = fs::read_to_string(cfg.wiki.join("index.md")).unwrap();
    assert!(index.contains("explicitly supersedes"));
    assert!(index.contains("reaffirms"));

    let conn = Connection::open(cfg.state.join("state.db")).unwrap();
    let relations = storage::relations(&conn).unwrap();
    assert_eq!(
        relations
            .iter()
            .filter(|r| r.kind == "reaffirms" && r.active)
            .count(),
        1
    );
    assert!(relations.iter().any(|r| r.kind == "supersedes" && r.active));
    let noop = engine::update(&cfg, &fake, None, UpdateOptions::default())
        .await
        .unwrap();
    assert!(noop.no_op);
    assert_eq!(noop.model_calls, 0);
}

#[test]
fn v2_database_can_upgrade_without_losing_records() {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(storage::SCHEMA_V1).unwrap();
    db.execute_batch(storage::SCHEMA_V2).unwrap();
    db.execute_batch("INSERT INTO projects VALUES('test','Test','2026-01-01');")
        .unwrap();
    storage::migrate(&db).unwrap();
    let version: i64 = db
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, lore::storage::SCHEMA_VERSION);
    let count: i64 = db
        .query_row("SELECT count(*) FROM projects", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1);
    let exists: i64 = db
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE name='reaffirmation_links'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(exists, 1);
}

#[test]
fn publication_date_is_not_decision_effective_date() {
    use lore::domain::effective_time_grounded;
    assert!(!effective_time_grounded(
        "We selected PostgreSQL for the ledger.",
        "# ADR-027\nDate: 2026-07-01\nStatus: Accepted",
        "2026-07-01"
    ));
    assert!(effective_time_grounded(
        "The service went live on 2026-08-18.",
        "# Deployment\nDate: 2026-08-20",
        "2026-08-18"
    ));
    assert!(effective_time_grounded(
        "PostgreSQL is selected.",
        "Effective date: 2026-07-15",
        "2026-07-15"
    ));
    assert!(effective_time_grounded(
        "Any source text",
        "Date: 2026-07-01",
        ""
    ));
}

struct RepeatedBadEffectiveDate {
    inner: FakeModel,
    extraction_calls: std::sync::atomic::AtomicUsize,
}
impl RepeatedBadEffectiveDate {
    fn new() -> Self {
        Self {
            inner: FakeModel::new(),
            extraction_calls: std::sync::atomic::AtomicUsize::new(0),
        }
    }
}
impl lore::inference::GenerativeModel for RepeatedBadEffectiveDate {
    fn descriptor(&self) -> &lore::inference::ModelDescriptor {
        lore::inference::GenerativeModel::descriptor(&self.inner)
    }

    fn generate<'a>(
        &'a self,
        request: &'a lore::inference::GenerationRequest,
    ) -> lore::inference::ModelFuture<'a, lore::inference::GenerationResponse> {
        use lore::inference::GenerativeModel;
        use std::sync::atomic::Ordering;

        Box::pin(async move {
            let mut response = self.inner.generate(request).await?;
            let input: serde_json::Value = serde_json::from_str(&request.input).unwrap();
            if input["task"] == "extract" {
                self.extraction_calls.fetch_add(1, Ordering::SeqCst);
                let mut payload: serde_json::Value = serde_json::from_str(&response.text).unwrap();
                for assertion in payload["assertions"].as_array_mut().unwrap() {
                    let quote = assertion["quote"].as_str().unwrap();
                    let date = if quote.contains("became the primary on 2026-08-18") {
                        "2026-08-18"
                    } else {
                        "2026-01-19"
                    };
                    assertion["effective_at"] = serde_json::json!(date);
                }
                response.text = payload.to_string();
            }
            Ok(response)
        })
    }
}

#[tokio::test]
async fn unsupported_model_effective_dates_are_cleared_without_aborting_extraction() {
    use std::sync::atomic::Ordering;

    let (_directory, cfg, _) = project();
    put(
        &cfg,
        "notes/architecture-review.md",
        "# Architecture review — 2026-01-19\n\nDECISION history: The review reaffirmed that MySQL is selected.\n",
    );
    put(
        &cfg,
        "notes/rollout.md",
        "# Rollout record\n\nDECISION rollout: PostgreSQL became the primary on 2026-08-18.\n",
    );
    let model = RepeatedBadEffectiveDate::new();
    let result = engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    assert!(!result.no_op);
    assert_eq!(result.processed_sections, 2);
    // An unsupported *optional* date must not consume a failed repair call.
    assert_eq!(model.extraction_calls.load(Ordering::SeqCst), 2);
    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.contains("Cleared 1 unsupported effective time"))
    );

    let conn = storage::read_only(&cfg.state.join("state.db")).unwrap();
    let units = storage::views(&conn).unwrap();
    let past = units.iter().find(|u| u.topic == "history").unwrap();
    let active = units.iter().find(|u| u.topic == "rollout").unwrap();
    assert_eq!(past.effective_at, "");
    assert_eq!(active.effective_at, "2026-08-18");

    // The cached typed extraction must itself contain canonicalized dates.
    let mut checked = 0;
    for file in fs::read_dir(cfg.state.join("cache")).unwrap() {
        let cached: serde_json::Value =
            serde_json::from_slice(&fs::read(file.unwrap().path()).unwrap()).unwrap();
        let payload: serde_json::Value =
            serde_json::from_str(cached["text"].as_str().unwrap()).unwrap();
        if let Some(assertions) = payload["assertions"].as_array() {
            for assertion in assertions {
                let quote = assertion["quote"].as_str().unwrap();
                let date = assertion["effective_at"].as_str().unwrap();
                if quote.contains("review reaffirmed") {
                    assert_eq!(date, "");
                    checked += 1;
                } else if quote.contains("became the primary") {
                    assert_eq!(date, "2026-08-18");
                    checked += 1;
                }
            }
        }
    }
    assert_eq!(checked, 2);
    let noop = engine::update(&cfg, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    assert!(noop.no_op);
    assert_eq!(noop.model_calls, 0);
}
