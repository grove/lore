mod common;

use lore::{
    engine::{self, UpdateOptions},
    inference::*,
    storage,
};
use std::sync::atomic::Ordering;

#[tokio::test]
async fn optional_usage_scope_is_small_and_joins_the_session_active_when_polled() {
    assert!(usage::current().is_none());
    let payload = [17u8; 32 * 1024];
    let operation = usage::scoped(async move {
        tokio::task::yield_now().await;
        let model = common::FakeModel::new();
        usage::cached(model.descriptor(), "generation").unwrap();
        (std::hint::black_box(payload)[0], usage::events())
    });
    let bytes = std::mem::size_of_val(&operation);
    assert!(
        bytes <= 1024,
        "the optional accounting wrapper retains an inline operation: {bytes} bytes"
    );

    // Scope selection must remain lazy: the operation was constructed outside
    // this invocation but is first polled inside it, including after yielding.
    let session = usage::UsageSession::memory();
    let (value, observed) = session.scope(operation).await;
    assert_eq!(value, 17);
    assert_eq!(observed.len(), 1);
    assert_eq!(session.events(), observed);
    assert_eq!(session.summary().cache_hits, 1);
    assert!(usage::current().is_none());
}

#[test]
fn event_hash_matches_python_for_unicode_and_small_currency_amounts() {
    let event: usage::UsageEvent = serde_json::from_value(serde_json::json!({
        "id":"event-1","call_id":"call-1","operation":"generation","provider":"ollama",
        "model":"modèle","attempt":1,"elapsed_ms":23,"http_status":200,
        "provider_request_count":1,"input_tokens":11,"output_tokens":3,"total_tokens":14,
        "billed_cost_usd":0.000001,"billing_source":"offline fixture","status":"completed","cache_hit":false
    })).unwrap();
    assert_eq!(
        usage::events_digest(&[event]).unwrap(),
        "bf9c88fabbaf5a0a8be487cc01fc566dcbfdc107a7afe460b9aa5d07c51551c1"
    );
}

struct MeteredFixture(common::FakeModel);
impl GenerativeModel for MeteredFixture {
    fn descriptor(&self) -> &ModelDescriptor {
        self.0.descriptor()
    }
    fn generate<'a>(
        &'a self,
        request: &'a GenerationRequest,
    ) -> ModelFuture<'a, GenerationResponse> {
        Box::pin(async move {
            let mut result = self.0.generate(request).await?;
            result.usage = Some(ProviderUsage {
                input_tokens: Some(17),
                output_tokens: Some(5),
                total_tokens: Some(22),
                ..ProviderUsage::request(usage::UsageStatus::Completed)
            });
            Ok(result)
        })
    }
}

#[tokio::test]
async fn extraction_repair_and_no_op_aggregate_from_retained_attempts() {
    let (_dir, config, fake) = common::project();
    common::put(
        &config,
        "database.md",
        "# Database\nDECISION database: MySQL is the selected database.\n",
    );
    fake.invalid_quote.store(true, Ordering::SeqCst);
    fake.repair_quote.store(true, Ordering::SeqCst);
    let model = MeteredFixture(fake);
    let report = engine::update(&config, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let conn = storage::read_only(&config.state.join("state.db")).unwrap();
    let events = storage::provider_usage(&conn, report.generation.as_deref().unwrap())
        .unwrap()
        .unwrap();
    let independent = usage::UsageSummary::from_events(&events);
    assert_eq!(report.usage, independent);
    assert!(
        events
            .iter()
            .any(|event| event.usage.status == usage::UsageStatus::ValidationFailed)
    );
    assert_eq!(
        report.usage.provider_request_count,
        Some(model.0.calls.load(Ordering::SeqCst) as u64)
    );
    assert_eq!(
        report.usage.input_tokens,
        report.usage.provider_request_count.map(|calls| calls * 17)
    );
    assert_eq!(
        report.usage.total_tokens,
        report.usage.provider_request_count.map(|calls| calls * 22)
    );
    assert_eq!(report.usage.billed_cost_usd, None);
    drop(conn);
    let unchanged = engine::update(&config, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    assert!(unchanged.no_op);
    assert_eq!(unchanged.usage, usage::UsageSummary::default());
}

#[tokio::test]
async fn cache_reuse_has_no_new_request_or_historical_token_charge() {
    let (_dir, config, fake) = common::project();
    common::put(
        &config,
        "database.md",
        "# Database\nDECISION database: MySQL is the selected database.\n",
    );
    let model = MeteredFixture(fake);
    let first = engine::update(&config, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    assert!(first.model_calls > 0);
    let repeat = engine::update(
        &config,
        &model,
        None,
        UpdateOptions {
            rebuild: true,
            ..UpdateOptions::default()
        },
    )
    .await
    .unwrap();
    assert!(repeat.usage.cache_hits > 0);
    assert_eq!(repeat.usage.provider_request_count, Some(0));
    assert_eq!(repeat.usage.input_tokens, Some(0));
    assert_eq!(repeat.usage.total_tokens, Some(0));
    assert_eq!(repeat.usage.billed_cost_usd, Some(0.0));
}

#[test]
fn migration_from_v07_preserves_historical_usage_as_unknown() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    for migration in [
        storage::SCHEMA_V1,
        storage::SCHEMA_V2,
        storage::SCHEMA_V3,
        storage::SCHEMA_V4,
        storage::SCHEMA_V5,
        storage::SCHEMA_V6,
        storage::SCHEMA_V7,
    ] {
        conn.execute_batch(migration).unwrap();
    }
    conn.execute_batch("INSERT INTO projects VALUES('p','Project','2026-10-10');
        INSERT INTO runs(id,project_id,phase,source_inventory_digest,started_at) VALUES('old','p','completed','fixture','2026-10-10');
        INSERT INTO model_calls VALUES('old-call','old','extract','Ollama','fixture','digest',0,123);").unwrap();
    assert!(storage::provider_usage(&conn, "old").unwrap().is_none());
    storage::migrate(&conn).unwrap();
    storage::migrate(&conn).unwrap();
    assert!(storage::provider_usage(&conn, "old").unwrap().is_none());
    let old: (String, i64) = conn
        .query_row(
            "SELECT model,duration_ms FROM model_calls WHERE id='old-call'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(old, ("fixture".into(), 123));
    conn.execute_batch("INSERT INTO runs(id,project_id,phase,source_inventory_digest,started_at) VALUES('new','p','completed','fixture','2026-10-10');").unwrap();
    storage::save_provider_usage(&conn, "new", &[]).unwrap();
    assert_eq!(storage::provider_usage(&conn, "new").unwrap(), Some(vec![]));
}

#[tokio::test]
async fn explicit_ledger_is_non_sensitive_and_existing_files_are_never_overwritten() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().canonicalize().unwrap().join("usage.json");
    let session = usage::UsageSession::at_path(&path).unwrap();
    let model = common::FakeModel::new();
    session
        .scope(async {
            usage::cached(model.descriptor(), "generation").unwrap();
        })
        .await;
    session.finish(true).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    let ledger: usage::UsageLedger = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        ledger.summary,
        usage::UsageSummary::from_events(&ledger.events)
    );
    assert_eq!(ledger.invocation_status, "completed");
    assert_eq!(ledger.summary.provider_request_count, Some(0));
    assert!(usage::UsageSession::at_path(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    let reference = session.summary().ledger.unwrap();
    assert_eq!(
        reference.events_sha256,
        usage::events_digest(&ledger.events).unwrap()
    );
}

#[tokio::test]
async fn uninstrumented_adapter_does_not_invent_provider_requests_or_tokens() {
    let session = usage::UsageSession::memory();
    let model = common::FakeModel::new();
    let request = GenerationRequest {
        instructions: "fixture".into(),
        input: serde_json::json!({"task":"extract","text":""}).to_string(),
        schema: None,
        reasoning_effort: None,
    };
    session
        .scope(usage::generate(&model, &request))
        .await
        .unwrap();
    assert_eq!(session.summary().model_calls, 1);
    assert_eq!(session.summary().provider_request_count, None);
    assert_eq!(session.summary().total_tokens, None);
    assert_eq!(session.summary().billed_cost_usd, None);
}
