//! Provider-neutral, object-safe asynchronous model contracts.
pub mod usage;
use serde::{Deserialize, Serialize};
use serde_json::Value;
pub use usage::ProviderUsage;

/// A typed Responses API reasoning effort. Model support varies; GPT-6 Luna
/// supports every level represented here. This is not a Decisions API setting.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReasoningEffort {
    None,
    Low,
    #[default]
    Medium,
    High,
    Xhigh,
    Max,
}

use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
    pin::Pin,
};

pub type ModelFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, ModelError>> + Send + 'a>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Provider {
    Ollama,
    OpenAi,
    TypeSafeCandidate,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionLocation {
    Local,
    Hosted,
}
#[derive(Debug, Clone)]
pub struct ModelDescriptor {
    pub provider: Provider,
    pub model: String,
    pub location: ExecutionLocation,
}
#[derive(Debug, Clone, Copy)]
pub enum EgressPolicy {
    LocalOnly,
    ExplicitHosted,
}
impl EgressPolicy {
    pub fn authorize(self, descriptor: &ModelDescriptor) -> Result<(), ModelError> {
        if matches!(self, Self::LocalOnly) && descriptor.location == ExecutionLocation::Hosted {
            Err(ModelError::RemoteDisabled)
        } else {
            Ok(())
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelError {
    InvalidRequest(String),
    InvalidResponse(String),
    Refused,
    Incomplete,
    RemoteDisabled,
    Unavailable(String),
}

#[derive(Debug, Clone)]
pub struct GenerationRequest {
    pub instructions: String,
    pub input: String,
    pub schema: Option<Value>,
    /// Optional per-request reasoning for OpenAI Responses only. Omit it to
    /// use a provider's default (or a model that does not support the option).
    pub reasoning_effort: Option<ReasoningEffort>,
}
#[derive(Debug, Clone)]
pub struct GenerationResponse {
    pub model: String,
    pub text: String,
    /// Provider accounting for this response, never an offline token estimate.
    /// Transport retries and unsuccessful attempts belong to the event ledger.
    pub usage: Option<ProviderUsage>,
}
pub trait GenerativeModel: Send + Sync {
    fn descriptor(&self) -> &ModelDescriptor;
    /// Stable model/configuration identity for disposable derived caches.
    /// Remote clients also include their endpoint, never credentials.
    fn cache_identity(&self) -> String {
        model_cache_identity(self.descriptor())
    }
    fn generate<'a>(
        &'a self,
        request: &'a GenerationRequest,
    ) -> ModelFuture<'a, GenerationResponse>;
}

pub fn model_cache_identity(descriptor: &ModelDescriptor) -> String {
    format!(
        "{:?}:{}:{:?}",
        descriptor.provider, descriptor.model, descriptor.location
    )
}

/// The embedding interface is separate from generation: configuring a chat
/// model does not imply that it supports vector inference or permit a second
/// provider. Callers must apply the same egress policy before invoking it.
#[derive(Debug, Clone)]
pub struct EmbeddingRequest {
    pub inputs: Vec<String>,
}

pub const MAX_EMBEDDING_INPUTS: usize = 32;
pub const MAX_EMBEDDING_DIMENSIONS: usize = 16_384;

impl EmbeddingRequest {
    pub fn validate(&self, max_input_bytes: usize) -> Result<(), ModelError> {
        if self.inputs.is_empty()
            || self.inputs.len() > MAX_EMBEDDING_INPUTS
            || self.inputs.iter().any(|input| input.trim().is_empty())
            || self.inputs.iter().map(String::len).sum::<usize>() > max_input_bytes
        {
            return Err(ModelError::InvalidRequest(
                "embedding input is empty or exceeds its batch/context budget".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct EmbeddingResponse {
    pub model: String,
    /// Position corresponds exactly to the original request input position.
    pub embeddings: Vec<Vec<f32>>,
    pub usage: Option<ProviderUsage>,
}

impl EmbeddingResponse {
    /// Reject partial batches, model substitution, malformed vectors and
    /// mixed dimensions before any vector is cached or used for retrieval.
    pub fn validate(
        &self,
        request: &EmbeddingRequest,
        descriptor: &ModelDescriptor,
    ) -> Result<usize, ModelError> {
        let model_matches = self.model == descriptor.model
            || descriptor.provider == Provider::Ollama
                && self.model.strip_suffix(":latest").unwrap_or(&self.model)
                    == descriptor
                        .model
                        .strip_suffix(":latest")
                        .unwrap_or(&descriptor.model);
        if self.model.trim().is_empty() || !model_matches {
            return Err(ModelError::InvalidResponse(
                "embedding response model does not match the configured model".into(),
            ));
        }
        if request.inputs.is_empty() || self.embeddings.len() != request.inputs.len() {
            return Err(ModelError::InvalidResponse(
                "embedding response has the wrong vector count".into(),
            ));
        }
        let dimensions = self.embeddings.first().map_or(0, Vec::len);
        if dimensions == 0
            || dimensions > MAX_EMBEDDING_DIMENSIONS
            || self
                .embeddings
                .iter()
                .any(|vector| vector.len() != dimensions || !valid_embedding(vector))
        {
            return Err(ModelError::InvalidResponse(
                "embedding vectors have invalid dimensions, magnitude or numeric values".into(),
            ));
        }
        Ok(dimensions)
    }
}

pub fn valid_embedding(vector: &[f32]) -> bool {
    !vector.is_empty()
        && vector.len() <= MAX_EMBEDDING_DIMENSIONS
        && vector.iter().all(|value| value.is_finite())
        && vector
            .iter()
            .map(|value| f64::from(*value).powi(2))
            .sum::<f64>()
            > 0.0
}

pub trait EmbeddingModel: Send + Sync {
    fn descriptor(&self) -> &ModelDescriptor;
    fn cache_identity(&self) -> String {
        model_cache_identity(self.descriptor())
    }
    fn embed<'a>(&'a self, request: &'a EmbeddingRequest) -> ModelFuture<'a, EmbeddingResponse>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptionDefinition {
    pub id: String,
    pub description: String,
}
#[derive(Debug, Clone)]
pub enum QuestionKind {
    Predicate,
    Choice { options: Vec<OptionDefinition> },
    Score { levels: Vec<OptionDefinition> },
}
#[derive(Debug, Clone)]
pub struct DecisionQuestion {
    pub name: String,
    pub instructions: String,
    pub kind: QuestionKind,
}
#[derive(Debug, Clone)]
pub struct DecisionRequest {
    pub input: String,
    pub questions: Vec<DecisionQuestion>,
}
impl DecisionRequest {
    pub fn validate(&self) -> Result<(), ModelError> {
        if self.input.trim().is_empty()
            || self.input.len() > 1_000_000
            || self.questions.is_empty()
            || self.questions.len() > 64
        {
            return Err(ModelError::InvalidRequest(
                "missing input or questions".into(),
            ));
        }
        let mut names = BTreeSet::new();
        for q in &self.questions {
            if q.name.trim().is_empty()
                || q.instructions.trim().is_empty()
                || !names.insert(&q.name)
            {
                return Err(ModelError::InvalidRequest(
                    "blank or duplicate question".into(),
                ));
            }
            let definitions = match &q.kind {
                QuestionKind::Predicate => None,
                QuestionKind::Choice { options } => Some(options),
                QuestionKind::Score { levels } => Some(levels),
            };
            if let Some(options) = definitions {
                if options.len() < 2
                    || options.len() > 255
                    || matches!(&q.kind, QuestionKind::Score { .. }) && options.len() > 10
                {
                    return Err(ModelError::InvalidRequest("too few choices/levels".into()));
                }
                let mut ids = BTreeSet::new();
                for o in options {
                    if o.id.trim().is_empty()
                        || o.description.trim().is_empty()
                        || !ids.insert(&o.id)
                    {
                        return Err(ModelError::InvalidRequest(
                            "blank or duplicate option".into(),
                        ));
                    }
                }
            }
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq)]
pub enum DecisionValue {
    Predicate(f64),
    Choice {
        selected: String,
        probabilities: BTreeMap<String, f64>,
        confidence: Option<f64>,
    },
    Score {
        weighted_index: f64,
        probabilities: Vec<f64>,
        confidence: Option<f64>,
    },
    Refusal,
}
#[derive(Debug, Clone, PartialEq)]
pub struct DecisionAnswer {
    pub name: String,
    pub value: DecisionValue,
}
#[derive(Debug, Clone)]
pub struct DecisionResponse {
    pub model: String,
    pub answers: Vec<DecisionAnswer>,
    pub usage: Option<ProviderUsage>,
}
impl DecisionResponse {
    pub fn validate(&self, request: &DecisionRequest) -> Result<(), ModelError> {
        request.validate()?;
        if self.model.trim().is_empty() || self.answers.len() != request.questions.len() {
            return Err(ModelError::InvalidResponse("wrong answer count".into()));
        }
        let mut observed = BTreeMap::new();
        for a in &self.answers {
            if observed.insert(&a.name, &a.value).is_some() {
                return Err(ModelError::InvalidResponse("duplicate answer".into()));
            }
        }
        for q in &request.questions {
            let value = observed
                .get(&q.name)
                .ok_or_else(|| ModelError::InvalidResponse("missing answer".into()))?;
            match (&q.kind, value) {
                (_, DecisionValue::Refusal) => {}
                (QuestionKind::Predicate, DecisionValue::Predicate(p)) => check_probability(*p)?,
                (
                    QuestionKind::Choice { options },
                    DecisionValue::Choice {
                        selected,
                        probabilities,
                        confidence,
                    },
                ) => {
                    if !options.iter().any(|o| o.id == *selected)
                        || probabilities.len() != options.len()
                    {
                        return Err(ModelError::InvalidResponse("unknown choice".into()));
                    }
                    for o in options {
                        check_probability(*probabilities.get(&o.id).ok_or_else(|| {
                            ModelError::InvalidResponse("missing probability".into())
                        })?)?;
                    }
                    check_sum(probabilities.values().copied())?;
                    let chosen = probabilities.get(selected).copied().unwrap_or(0.0);
                    if probabilities.values().any(|p| *p > chosen + 0.01) {
                        return Err(ModelError::InvalidResponse(
                            "choice contradicts its probability distribution".into(),
                        ));
                    }
                    if let Some(c) = confidence {
                        check_probability(*c)?;
                    }
                }
                (
                    QuestionKind::Score { levels },
                    DecisionValue::Score {
                        weighted_index,
                        probabilities,
                        confidence,
                    },
                ) => {
                    if probabilities.len() != levels.len() {
                        return Err(ModelError::InvalidResponse("wrong score length".into()));
                    }
                    check_sum(probabilities.iter().copied())?;
                    let expected: f64 = probabilities
                        .iter()
                        .enumerate()
                        .map(|(i, p)| i as f64 * p)
                        .sum();
                    if !weighted_index.is_finite()
                        || *weighted_index < 0.0
                        || *weighted_index > (levels.len() - 1) as f64
                        || (expected - weighted_index).abs() > 0.02
                    {
                        return Err(ModelError::InvalidResponse(
                            "score distribution mismatch".into(),
                        ));
                    }
                    if let Some(c) = confidence {
                        check_probability(*c)?;
                    }
                }
                _ => return Err(ModelError::InvalidResponse("wrong answer type".into())),
            }
        }
        Ok(())
    }
}
fn check_probability(p: f64) -> Result<(), ModelError> {
    if p.is_finite() && (0.0..=1.0).contains(&p) {
        Ok(())
    } else {
        Err(ModelError::InvalidResponse("invalid probability".into()))
    }
}
fn check_sum(values: impl Iterator<Item = f64>) -> Result<(), ModelError> {
    let mut total = 0.0;
    for p in values {
        check_probability(p)?;
        total += p;
    }
    if (total - 1.0).abs() > 0.01 {
        Err(ModelError::InvalidResponse("invalid distribution".into()))
    } else {
        Ok(())
    }
}
pub trait DecisionModel: Send + Sync {
    fn descriptor(&self) -> &ModelDescriptor;
    fn decide<'a>(&'a self, request: &'a DecisionRequest) -> ModelFuture<'a, DecisionResponse>;
}
