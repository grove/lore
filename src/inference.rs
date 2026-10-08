//! Provider-neutral, object-safe asynchronous model contracts.
use std::{collections::{BTreeMap, BTreeSet}, future::Future, pin::Pin};
use serde_json::Value;

pub type ModelFuture<'a, T> = Pin<Box<dyn Future<Output=Result<T, ModelError>> + Send + 'a>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Provider { Ollama, OpenAi, TypeSafeCandidate }
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionLocation { Local, Hosted }
#[derive(Debug, Clone)]
pub struct ModelDescriptor {
    pub provider: Provider,
    pub model: String,
    pub location: ExecutionLocation,
}
#[derive(Debug, Clone, Copy)]
pub enum EgressPolicy { LocalOnly, ExplicitHosted }
impl EgressPolicy {
    pub fn authorize(self, descriptor: &ModelDescriptor) -> Result<(), ModelError> {
        if matches!(self, Self::LocalOnly) && descriptor.location == ExecutionLocation::Hosted {
            Err(ModelError::RemoteDisabled)
        } else { Ok(()) }
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
}
#[derive(Debug, Clone)]
pub struct GenerationResponse { pub model: String, pub text: String }
pub trait GenerativeModel: Send + Sync {
    fn descriptor(&self) -> &ModelDescriptor;
    fn generate<'a>(&'a self, request: &'a GenerationRequest) -> ModelFuture<'a, GenerationResponse>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptionDefinition { pub id: String, pub description: String }
#[derive(Debug, Clone)]
pub enum QuestionKind {
    Predicate,
    Choice { options: Vec<OptionDefinition> },
    Score { levels: Vec<OptionDefinition> },
}
#[derive(Debug, Clone)]
pub struct DecisionQuestion { pub name: String, pub instructions: String, pub kind: QuestionKind }
#[derive(Debug, Clone)]
pub struct DecisionRequest { pub input: String, pub questions: Vec<DecisionQuestion> }
impl DecisionRequest {
    pub fn validate(&self) -> Result<(), ModelError> {
        if self.input.trim().is_empty() || self.questions.is_empty() {
            return Err(ModelError::InvalidRequest("missing input or questions".into()));
        }
        let mut names=BTreeSet::new();
        for q in &self.questions {
            if q.name.trim().is_empty() || q.instructions.trim().is_empty() || !names.insert(&q.name) {
                return Err(ModelError::InvalidRequest("blank or duplicate question".into()));
            }
            let definitions=match &q.kind {
                QuestionKind::Predicate => None,
                QuestionKind::Choice { options } => Some(options),
                QuestionKind::Score { levels } => Some(levels),
            };
            if let Some(options)=definitions {
                if options.len()<2 { return Err(ModelError::InvalidRequest("too few choices/levels".into())); }
                let mut ids=BTreeSet::new();
                for o in options {
                    if o.id.trim().is_empty() || o.description.trim().is_empty() || !ids.insert(&o.id) {
                        return Err(ModelError::InvalidRequest("blank or duplicate option".into()));
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
    Choice { selected: String, probabilities: BTreeMap<String, f64>, confidence: Option<f64> },
    Score { weighted_index: f64, probabilities: Vec<f64>, confidence: Option<f64> },
    Refusal,
}
#[derive(Debug, Clone, PartialEq)]
pub struct DecisionAnswer { pub name: String, pub value: DecisionValue }
#[derive(Debug, Clone)]
pub struct DecisionResponse { pub model: String, pub answers: Vec<DecisionAnswer> }
impl DecisionResponse {
    pub fn validate(&self, request: &DecisionRequest) -> Result<(), ModelError> {
        request.validate()?;
        if self.answers.len()!=request.questions.len() { return Err(ModelError::InvalidResponse("wrong answer count".into())); }
        let mut observed=BTreeMap::new();
        for a in &self.answers {
            if observed.insert(&a.name, &a.value).is_some() { return Err(ModelError::InvalidResponse("duplicate answer".into())); }
        }
        for q in &request.questions {
            let value=observed.get(&q.name).ok_or_else(||ModelError::InvalidResponse("missing answer".into()))?;
            match (&q.kind,value) {
                (_, DecisionValue::Refusal) => {},
                (QuestionKind::Predicate,DecisionValue::Predicate(p)) => check_probability(*p)?,
                (QuestionKind::Choice { options },DecisionValue::Choice { selected,probabilities,confidence }) => {
                    if !options.iter().any(|o|o.id==*selected) || probabilities.len()!=options.len() {
                        return Err(ModelError::InvalidResponse("unknown choice".into()));
                    }
                    for o in options {
                        check_probability(*probabilities.get(&o.id).ok_or_else(||ModelError::InvalidResponse("missing probability".into()))?)?;
                    }
                    check_sum(probabilities.values().copied())?;
                    if let Some(c)=confidence {check_probability(*c)?;}
                },
                (QuestionKind::Score { levels },DecisionValue::Score { weighted_index,probabilities,confidence }) => {
                    if probabilities.len()!=levels.len() { return Err(ModelError::InvalidResponse("wrong score length".into())); }
                    check_sum(probabilities.iter().copied())?;
                    let expected: f64=probabilities.iter().enumerate().map(|(i,p)|i as f64*p).sum();
                    if !weighted_index.is_finite() || (expected-weighted_index).abs()>0.02 {
                        return Err(ModelError::InvalidResponse("score distribution mismatch".into()));
                    }
                    if let Some(c)=confidence {check_probability(*c)?;}
                },
                _ => return Err(ModelError::InvalidResponse("wrong answer type".into())),
            }
        }
        Ok(())
    }
}
fn check_probability(p:f64)->Result<(),ModelError>{
    if p.is_finite() && (0.0..=1.0).contains(&p){Ok(())}
    else {Err(ModelError::InvalidResponse("invalid probability".into()))}
}
fn check_sum(values:impl Iterator<Item=f64>)->Result<(),ModelError>{
    let mut total=0.0;
    for p in values{check_probability(p)?;total+=p;}
    if (total-1.0).abs()>0.01{Err(ModelError::InvalidResponse("invalid distribution".into()))} else {Ok(())}
}
pub trait DecisionModel: Send + Sync {
    fn descriptor(&self) -> &ModelDescriptor;
    fn decide<'a>(&'a self, request: &'a DecisionRequest) -> ModelFuture<'a,DecisionResponse>;
}
