//! Provider accounting is separate from response-envelope tokenization.
//!
//! A session records attempts, including failures and transport retries. It is
//! in memory unless the invoking host explicitly supplies `LORE_USAGE_LEDGER`.
//! The ledger contains no prompts, response text, URLs, keys or provider bodies.
//! JSON schema 2/3/4 need no additional public fields to opt into that sidecar.

use super::*;
use crate::util;
use anyhow::{Context, Result, ensure};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Instant,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageStatus {
    Started,
    Completed,
    HttpError,
    TransportError,
    TimedOut,
    Refused,
    Incomplete,
    ValidationFailed,
    Cancelled,
    Cached,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderUsage {
    /// Unknown for an embedding application's uninstrumented adapter. A live
    /// HTTP attempt is exactly one; a local cache or preflight denial is zero.
    pub provider_request_count: Option<u64>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
    pub billed_cost_usd: Option<f64>,
    pub billing_source: Option<String>,
    pub status: UsageStatus,
    pub cache_hit: bool,
}

impl ProviderUsage {
    pub fn unknown(status: UsageStatus) -> Self {
        Self {
            provider_request_count: None,
            input_tokens: None,
            output_tokens: None,
            total_tokens: None,
            billed_cost_usd: None,
            billing_source: None,
            status,
            cache_hit: false,
        }
    }

    pub fn request(status: UsageStatus) -> Self {
        Self {
            provider_request_count: Some(1),
            ..Self::unknown(status)
        }
    }

    pub fn no_request(status: UsageStatus) -> Self {
        Self {
            provider_request_count: Some(0),
            input_tokens: Some(0),
            output_tokens: Some(0),
            total_tokens: Some(0),
            billed_cost_usd: Some(0.0),
            billing_source: Some("no_provider_requests".into()),
            status,
            cache_hit: status == UsageStatus::Cached,
        }
    }

    /// Reject malformed optional accounting without rejecting an otherwise
    /// useful answer or manufacturing a price from a model's name.
    pub fn normalized(mut self) -> Self {
        if self
            .billed_cost_usd
            .is_some_and(|amount| !amount.is_finite() || amount < 0.0)
            || self.billing_source.as_ref().is_none_or(|source| {
                source.trim().is_empty()
                    || source.len() > 128
                    || source.chars().any(char::is_control)
            })
        {
            self.billed_cost_usd = None;
        }
        if self.billed_cost_usd.is_none() {
            self.billing_source = None;
        }
        if let (Some(input), Some(output), Some(total)) =
            (self.input_tokens, self.output_tokens, self.total_tokens)
            && input.checked_add(output) != Some(total)
        {
            self.total_tokens = None;
        }
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UsageEvent {
    pub id: String,
    /// Retries share this logical-call identity; cache events are separate.
    pub call_id: String,
    pub operation: String,
    pub provider: String,
    /// Configured model identity; never copied from an arbitrary error body.
    pub model: String,
    pub attempt: u32,
    pub elapsed_ms: u64,
    pub http_status: Option<u16>,
    #[serde(flatten)]
    pub usage: ProviderUsage,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LedgerReference {
    pub path: String,
    /// SHA-256 from `events_digest` of the prefix `events[..event_count]`.
    /// Later phases can append events without invalidating an earlier summary.
    pub events_sha256: String,
    pub event_count: usize,
}

/// Stable across JSON writers: sorted object keys, UTF-8, no whitespace, and
/// billing floats represented by their IEEE-754 bits for hashing only. The
/// ledger still exposes normal nullable JSON numbers for every measured value.
pub fn events_digest(events: &[UsageEvent]) -> Result<String> {
    use sha2::{Digest, Sha256};
    let mut value = serde_json::to_value(events)?;
    for (value, event) in value.as_array_mut().unwrap().iter_mut().zip(events) {
        if let Some(amount) = event.usage.billed_cost_usd {
            value["billed_cost_usd"] = format!("{:016x}", amount.to_bits()).into();
        }
    }
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(&value)?)))
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UsageSummary {
    pub schema_version: u32,
    pub model_calls: u64,
    pub provider_request_count: Option<u64>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
    pub billed_cost_usd: Option<f64>,
    pub billing_source: Option<String>,
    pub cache_hits: u64,
    pub event_count: usize,
    /// Present only for the caller's explicitly requested sidecar.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub ledger: Option<LedgerReference>,
}

impl Default for UsageSummary {
    fn default() -> Self {
        Self::from_events(&[])
    }
}

impl UsageSummary {
    pub fn from_events(events: &[UsageEvent]) -> Self {
        let sum = |field: fn(&ProviderUsage) -> Option<u64>| {
            events
                .iter()
                .try_fold(0u64, |total, event| total.checked_add(field(&event.usage)?))
        };
        let billed_cost_usd = events.iter().try_fold(0.0, |total, event| {
            let amount = total + event.usage.billed_cost_usd?;
            amount.is_finite().then_some(amount)
        });
        let provider_request_count = sum(|usage| usage.provider_request_count);
        Self {
            schema_version: 1,
            model_calls: events
                .iter()
                .filter(|event| !event.usage.cache_hit)
                .map(|event| &event.call_id)
                .collect::<BTreeSet<_>>()
                .len() as u64,
            provider_request_count,
            input_tokens: sum(|usage| usage.input_tokens),
            output_tokens: sum(|usage| usage.output_tokens),
            total_tokens: sum(|usage| usage.total_tokens),
            billed_cost_usd,
            billing_source: billed_cost_usd.map(|_| {
                if provider_request_count == Some(0) {
                    "no_provider_requests"
                } else {
                    "sum_of_explicit_event_billing"
                }
                .into()
            }),
            cache_hits: events.iter().filter(|event| event.usage.cache_hit).count() as u64,
            event_count: events.len(),
            ledger: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageLedger {
    pub contract: String,
    pub schema_version: u32,
    pub invocation_status: String,
    pub summary: UsageSummary,
    pub events: Vec<UsageEvent>,
}

#[derive(Default)]
struct State {
    events: Vec<UsageEvent>,
    path: Option<PathBuf>,
    claimed: bool,
    failure: bool,
    status: String,
}

/// Cloneable event sink. A file is a caller-owned audit output, never a cache.
#[derive(Clone, Default)]
pub struct UsageSession(Arc<Mutex<State>>);

tokio::task_local! {
    static SESSION: UsageSession;
    static CALL_ID: String;
}

impl UsageSession {
    pub fn memory() -> Self {
        Self::default()
    }

    /// An existing file or a symlink is rejected; source/config files cannot be
    /// overwritten by setting the environment variable to one of their paths.
    pub fn at_path(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        ensure!(
            path.is_absolute(),
            "LORE_USAGE_LEDGER must be an absolute new file path"
        );
        util::reject_symlinks(path)?;
        ensure!(
            !path.exists(),
            "LORE_USAGE_LEDGER already exists; choose a new file"
        );
        ensure!(
            path.parent().is_some_and(Path::is_dir),
            "usage ledger parent must already exist"
        );
        Ok(Self(Arc::new(Mutex::new(State {
            path: Some(path.to_owned()),
            status: "running".into(),
            ..State::default()
        }))))
    }

    pub fn from_environment() -> Result<Self> {
        match std::env::var_os("LORE_USAGE_LEDGER").filter(|path| !path.is_empty()) {
            Some(path) => Self::at_path(PathBuf::from(path)),
            None => Ok(Self::memory()),
        }
    }

    pub async fn scope<F: Future>(&self, future: F) -> F::Output {
        SESSION.scope(self.clone(), future).await
    }

    pub fn events(&self) -> Vec<UsageEvent> {
        self.0
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .events
            .clone()
    }

    pub fn summary(&self) -> UsageSummary {
        let state = self.0.lock().unwrap_or_else(|error| error.into_inner());
        let mut summary = UsageSummary::from_events(&state.events);
        if let Some(path) = &state.path
            && state.claimed
            && !state.failure
            && let Ok(events_sha256) = events_digest(&state.events)
        {
            summary.ledger = Some(LedgerReference {
                path: path.to_string_lossy().into_owned(),
                events_sha256,
                event_count: state.events.len(),
            });
        }
        summary
    }

    /// Finish an explicitly requested audit even when no inference occurred.
    /// Default memory-only sessions perform no filesystem operation.
    pub fn finish(&self, succeeded: bool) -> Result<()> {
        let mut state = self.0.lock().unwrap_or_else(|error| error.into_inner());
        state.status = if succeeded { "completed" } else { "failed" }.into();
        persist(&mut state)?;
        ensure!(
            !state.failure,
            "provider usage ledger could not be retained"
        );
        Ok(())
    }

    fn put(&self, event: UsageEvent) -> Result<(), ModelError> {
        let mut state = self.0.lock().unwrap_or_else(|error| error.into_inner());
        if let Some(existing) = state.events.iter_mut().find(|old| old.id == event.id) {
            *existing = event;
        } else {
            state.events.push(event);
        }
        persist(&mut state).map_err(|_| {
            ModelError::Unavailable("provider usage ledger could not be retained".into())
        })
    }

    fn has_call(&self, id: &str) -> bool {
        self.0
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .events
            .iter()
            .any(|event| event.call_id == id)
    }
}

fn ledger(state: &State) -> UsageLedger {
    UsageLedger {
        contract: "lore.provider_usage".into(),
        schema_version: 1,
        invocation_status: if state.status.is_empty() {
            "running".into()
        } else {
            state.status.clone()
        },
        summary: UsageSummary::from_events(&state.events),
        events: state.events.clone(),
    }
}

fn persist(state: &mut State) -> Result<()> {
    let Some(path) = state.path.clone() else {
        return Ok(());
    };
    let result = (|| -> Result<()> {
        util::reject_symlinks(&path)?;
        if !state.claimed {
            let parent = path
                .parent()
                .context("usage ledger needs a parent directory")?;
            ensure!(parent.is_dir(), "usage ledger parent must already exist");
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            options.open(&path).context("claim new usage ledger")?;
            state.claimed = true;
        }
        util::atomic_write(&path, &serde_json::to_vec(&ledger(state))?)
    })();
    if result.is_err() {
        state.failure = true;
    }
    result
}

pub fn current() -> Option<UsageSession> {
    SESSION.try_with(Clone::clone).ok()
}
pub fn summary() -> UsageSummary {
    current().map_or_else(UsageSummary::default, |session| session.summary())
}
pub fn events() -> Vec<UsageEvent> {
    current().map_or_else(Vec::new, |session| session.events())
}

/// Reserve compact schema-5 metadata before synthesis. The complete result is
/// still measured afterwards; provider tokens never use this tokenizer.
pub fn budget_overhead() -> usize {
    let mut summary = UsageSummary {
        model_calls: u64::MAX,
        provider_request_count: Some(u64::MAX),
        input_tokens: Some(u64::MAX),
        output_tokens: Some(u64::MAX),
        total_tokens: Some(u64::MAX),
        billed_cost_usd: Some(f64::MAX),
        billing_source: Some("sum_of_explicit_event_billing".into()),
        cache_hits: u64::MAX,
        event_count: usize::MAX,
        ..UsageSummary::default()
    };
    if let Some(session) = current() {
        let state = session.0.lock().unwrap_or_else(|error| error.into_inner());
        if let Some(path) = &state.path {
            summary.ledger = Some(LedgerReference {
                path: path.to_string_lossy().into_owned(),
                events_sha256: "f".repeat(64),
                event_count: usize::MAX,
            });
        }
    }
    crate::context::count_tokens(&serde_json::to_string(&summary).unwrap_or_default()) + 64
}

/// Reuse the CLI's invocation scope; library callers get an isolated in-memory
/// scope so concurrent tasks cannot mix their provider accounting.
pub fn scoped<F: Future>(future: F) -> impl Future<Output = F::Output> {
    // Allocate before constructing the async wrapper: otherwise its initial
    // state and both branches retain the large command future inline, even
    // when an existing invocation means the extra scope is never used.
    let future = Box::pin(future);
    async move {
        // The caller may enter its session after constructing this future.
        // Keep selection at poll time so all attempts join that invocation.
        if current().is_some() {
            future.await
        } else {
            UsageSession::memory().scope(future).await
        }
    }
}

pub fn call_id() -> String {
    CALL_ID
        .try_with(Clone::clone)
        .unwrap_or_else(|_| util::id("usage_call"))
}

fn descriptor_fields(descriptor: &ModelDescriptor) -> (String, String) {
    let provider = match descriptor.provider {
        Provider::Ollama => "ollama",
        Provider::OpenAi => "openai",
        Provider::TypeSafeCandidate => "typesafe",
    };
    // Providers may be supplied by an embedding host. Prevent control text or
    // unbounded metadata from making its way into audit logs.
    let model = descriptor
        .model
        .chars()
        .filter(|character| !character.is_control())
        .take(512)
        .collect();
    (provider.into(), model)
}

/// Write a started event before transmission. Drop closes an interrupted
/// future as cancelled, retaining unknown usage for work already sent.
pub struct Attempt {
    event: UsageEvent,
    sinks: Vec<UsageSession>,
    started: Instant,
    finished: bool,
}

impl Attempt {
    pub fn start(
        descriptor: &ModelDescriptor,
        operation: &str,
        call_id: &str,
        attempt: u32,
        local_sink: Option<UsageSession>,
        usage: ProviderUsage,
    ) -> Result<Self, ModelError> {
        let (provider, model) = descriptor_fields(descriptor);
        let mut sinks = current().into_iter().collect::<Vec<_>>();
        if let Some(sink) = local_sink
            && !sinks
                .iter()
                .any(|existing| Arc::ptr_eq(&existing.0, &sink.0))
        {
            sinks.push(sink);
        }
        let event = UsageEvent {
            id: util::id("usage"),
            call_id: call_id.into(),
            operation: operation.into(),
            provider,
            model,
            attempt,
            elapsed_ms: 0,
            http_status: None,
            usage,
        };
        for sink in &sinks {
            sink.put(event.clone())?;
        }
        Ok(Self {
            event,
            sinks,
            started: Instant::now(),
            finished: false,
        })
    }

    pub fn finish(
        mut self,
        usage: ProviderUsage,
        http_status: Option<u16>,
    ) -> Result<(), ModelError> {
        self.event.usage = usage.normalized();
        self.event.http_status = http_status;
        self.event.elapsed_ms =
            u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX);
        self.finished = true;
        for sink in &self.sinks {
            sink.put(self.event.clone())?;
        }
        Ok(())
    }
}

impl Drop for Attempt {
    fn drop(&mut self) {
        if !self.finished {
            self.event.usage.status = UsageStatus::Cancelled;
            self.event.elapsed_ms =
                u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX);
            for sink in &self.sinks {
                let _ = sink.put(self.event.clone());
            }
        }
    }
}

pub fn cached(descriptor: &ModelDescriptor, operation: &str) -> Result<(), ModelError> {
    Attempt::start(
        descriptor,
        operation,
        &call_id(),
        0,
        None,
        ProviderUsage::no_request(UsageStatus::Cached),
    )?
    .finish(ProviderUsage::no_request(UsageStatus::Cached), None)
}

pub fn status(error: &ModelError) -> UsageStatus {
    match error {
        ModelError::InvalidRequest(_) | ModelError::RemoteDisabled => UsageStatus::Rejected,
        ModelError::InvalidResponse(_) => UsageStatus::ValidationFailed,
        ModelError::Refused => UsageStatus::Refused,
        ModelError::Incomplete => UsageStatus::Incomplete,
        ModelError::Unavailable(_) => UsageStatus::TransportError,
    }
}

/// Additional local validation can reject an HTTP-successful answer. Mark the
/// actual response event; this does not invent another provider request.
pub fn validation_failed() {
    if let Some(session) = current() {
        let mut events = session.events();
        if let Some(event) = events.iter_mut().rev().find(|event| !event.usage.cache_hit)
            && event.usage.status == UsageStatus::Completed
        {
            event.usage.status = UsageStatus::ValidationFailed;
            let _ = session.put(event.clone());
        }
    }
}

struct UninstrumentedCall {
    id: String,
    descriptor: ModelDescriptor,
    operation: &'static str,
    started: Instant,
    complete: bool,
}

impl UninstrumentedCall {
    fn new(descriptor: &ModelDescriptor, operation: &'static str) -> Self {
        Self {
            id: util::id("usage_call"),
            descriptor: descriptor.clone(),
            operation,
            started: Instant::now(),
            complete: false,
        }
    }
    fn finish(&mut self, usage: ProviderUsage) -> Result<(), ModelError> {
        self.complete = true;
        if current().is_some_and(|session| !session.has_call(&self.id)) {
            let mut event = Attempt::start(
                &self.descriptor,
                self.operation,
                &self.id,
                1,
                None,
                usage.clone(),
            )?;
            event.started = self.started;
            event.finish(usage, None)?;
        }
        Ok(())
    }
}

impl Drop for UninstrumentedCall {
    fn drop(&mut self) {
        if !self.complete {
            let _ = self.finish(ProviderUsage::unknown(UsageStatus::Cancelled));
        }
    }
}

pub async fn generate(
    model: &dyn GenerativeModel,
    request: &GenerationRequest,
) -> Result<GenerationResponse, ModelError> {
    let mut call = UninstrumentedCall::new(model.descriptor(), "generation");
    let result = CALL_ID
        .scope(call.id.clone(), model.generate(request))
        .await;
    let usage = match &result {
        Ok(response) => response
            .usage
            .clone()
            .unwrap_or_else(|| ProviderUsage::unknown(UsageStatus::Completed)),
        Err(error) => ProviderUsage::unknown(status(error)),
    };
    call.finish(usage)?;
    result
}

pub async fn embed(
    model: &dyn EmbeddingModel,
    request: &EmbeddingRequest,
) -> Result<EmbeddingResponse, ModelError> {
    let mut call = UninstrumentedCall::new(model.descriptor(), "embedding");
    let result = CALL_ID.scope(call.id.clone(), model.embed(request)).await;
    let usage = match &result {
        Ok(response) => response
            .usage
            .clone()
            .unwrap_or_else(|| ProviderUsage::unknown(UsageStatus::Completed)),
        Err(error) => ProviderUsage::unknown(status(error)),
    };
    call.finish(usage)?;
    result
}

pub async fn decide(
    model: &dyn DecisionModel,
    request: &DecisionRequest,
) -> Result<DecisionResponse, ModelError> {
    let mut call = UninstrumentedCall::new(model.descriptor(), "decision");
    let result = CALL_ID.scope(call.id.clone(), model.decide(request)).await;
    let usage = match &result {
        Ok(response) => response
            .usage
            .clone()
            .unwrap_or_else(|| ProviderUsage::unknown(UsageStatus::Completed)),
        Err(error) => ProviderUsage::unknown(status(error)),
    };
    call.finish(usage)?;
    result
}
