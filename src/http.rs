//! Live HTTP clients. Egress is checked at construction and before every call;
//! endpoints cannot redirect, and response bodies never appear in error logs.
use crate::{
    config::{ModelRole, ResolvedConfig},
    inference::*,
    provider_wire::*,
};
use reqwest::{Client, StatusCode};
use serde_json::Value;
use std::{
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};
use url::{Host, Url};

pub struct HttpModel {
    descriptor: ModelDescriptor,
    base: Url,
    client: Client,
    key_env: Option<String>,
    credential: Option<String>,
    retries: u32,
    max_context: usize,
    local_only: bool,
    pub requests: AtomicUsize,
}
impl HttpModel {
    pub fn new(config: &ResolvedConfig, role: &ModelRole) -> Result<Self, ModelError> {
        let (provider, default_url, default_key) = match role.provider.as_str() {
            "ollama" => (Provider::Ollama, "http://127.0.0.1:11434/", None),
            "openai" => (
                Provider::OpenAi,
                "https://api.openai.com/v1/",
                Some("OPENAI_API_KEY"),
            ),
            "typesafe" => (
                Provider::TypeSafeCandidate,
                "https://api.typesafe.ai/v1/",
                Some("TYPESAFE_API_KEY"),
            ),
            _ => return Err(ModelError::InvalidRequest("unsupported provider".into())),
        };
        let settings = config.config.providers.get(&role.provider);
        let raw = settings
            .and_then(|s| s.base_url.as_deref())
            .unwrap_or(default_url);
        let base = Url::parse(&format!("{}/", raw.trim_end_matches('/')))
            .map_err(|_| ModelError::InvalidRequest("invalid provider base_url".into()))?;
        let local = matches!(base.host(), Some(Host::Ipv4(ip)) if ip.is_loopback())
            || matches!(base.host(), Some(Host::Ipv6(ip)) if ip.is_loopback());
        // Literal loopback addresses avoid DNS rebinding and localhost proxying.
        if !base.username().is_empty()
            || base.password().is_some()
            || base.query().is_some()
            || base.fragment().is_some()
            || !["http", "https"].contains(&base.scheme())
            || (!local && base.scheme() != "https")
        {
            return Err(ModelError::InvalidRequest("use an HTTPS endpoint or literal loopback HTTP address, without credentials/query/fragment".into()));
        }
        if config.config.privacy.local_only && (!local || provider != Provider::Ollama) {
            return Err(ModelError::RemoteDisabled);
        }
        let mut builder = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(
                config.config.processing.timeout_seconds,
            ));
        if local {
            builder = builder.no_proxy();
        }
        let client = builder
            .build()
            .map_err(|_| ModelError::Unavailable("HTTP client initialization failed".into()))?;
        Ok(Self {
            descriptor: ModelDescriptor {
                provider,
                model: role.model.clone(),
                location: if local {
                    ExecutionLocation::Local
                } else {
                    ExecutionLocation::Hosted
                },
            },
            base,
            client,
            key_env: settings
                .and_then(|s| s.api_key_env.clone())
                .or_else(|| default_key.map(str::to_owned)),
            credential: None,
            retries: config.config.processing.retry_attempts,
            max_context: config.config.processing.max_context_bytes,
            local_only: config.config.privacy.local_only,
            requests: AtomicUsize::new(0),
        })
    }
    /// Explicit credential injection for embedding applications and mock tests.
    /// CLI users supply the configured environment variable instead.
    pub fn with_credential(mut self, credential: String) -> Self {
        self.credential = Some(credential);
        self
    }
    fn authorize(&self) -> Result<Option<String>, ModelError> {
        if self.local_only
            && (self.descriptor.location != ExecutionLocation::Local
                || self.descriptor.provider != Provider::Ollama)
        {
            return Err(ModelError::RemoteDisabled);
        }
        if let Some(value) = &self.credential {
            return Ok(Some(value.clone()));
        }
        if let Some(name) = &self.key_env {
            return std::env::var(name)
                .ok()
                .filter(|s| !s.trim().is_empty())
                .map(Some)
                .ok_or_else(|| {
                    ModelError::Unavailable(format!("set credential environment variable {name}"))
                });
        }
        Ok(None)
    }
    async fn send(&self, endpoint: &str, body: Option<&Value>) -> Result<Value, ModelError> {
        let key = self.authorize()?;
        if let Some(body) = body {
            if body.to_string().len() > self.max_context * 3 {
                return Err(ModelError::InvalidRequest(
                    "request exceeds context budget".into(),
                ));
            }
        }
        let url = self
            .base
            .join(endpoint)
            .map_err(|_| ModelError::InvalidRequest("invalid endpoint".into()))?;
        for attempt in 0..=self.retries {
            let mut request = if let Some(body) = body {
                self.client.post(url.clone()).json(body)
            } else {
                self.client.get(url.clone())
            };
            if let Some(key) = &key {
                request = request.bearer_auth(key);
            }
            self.requests.fetch_add(1, Ordering::Relaxed);
            let response = request.send().await;
            match response {
                Err(_) if attempt < self.retries => {
                    tokio::time::sleep(Duration::from_millis(200 * (1 << attempt))).await
                }
                Err(_) => {
                    return Err(ModelError::Unavailable(
                        "provider connection failed or timed out".into(),
                    ));
                }
                Ok(mut response) => {
                    let status = response.status();
                    if (status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error())
                        && attempt < self.retries
                    {
                        let seconds = response
                            .headers()
                            .get("retry-after")
                            .and_then(|v| v.to_str().ok())
                            .and_then(|v| v.parse::<u64>().ok())
                            .unwrap_or(1 << attempt)
                            .min(30);
                        tokio::time::sleep(Duration::from_secs(seconds)).await;
                        continue;
                    }
                    if !status.is_success() {
                        return Err(ModelError::Unavailable(format!(
                            "provider HTTP {} (response body withheld)",
                            status.as_u16()
                        )));
                    }
                    let maximum = 4_000_000usize;
                    if response
                        .content_length()
                        .is_some_and(|n| n > maximum as u64)
                    {
                        return Err(ModelError::InvalidResponse("response exceeds limit".into()));
                    }
                    let mut bytes = Vec::new();
                    while let Some(chunk) =
                        response.chunk().await.map_err(|_| ModelError::Incomplete)?
                    {
                        if bytes.len() + chunk.len() > maximum {
                            return Err(ModelError::InvalidResponse(
                                "response exceeds limit".into(),
                            ));
                        }
                        bytes.extend_from_slice(&chunk);
                    }
                    return serde_json::from_slice(&bytes).map_err(|_| {
                        ModelError::InvalidResponse("provider did not return JSON".into())
                    });
                }
            }
        }
        Err(ModelError::Unavailable("retry budget exhausted".into()))
    }
    pub async fn doctor(&self) -> Result<Value, ModelError> {
        match self.descriptor.provider {
            Provider::Ollama => {
                let tags = self.send("api/tags", None).await?;
                let model = &self.descriptor.model;
                let found = tags
                    .get("models")
                    .and_then(Value::as_array)
                    .is_some_and(|models| {
                        models.iter().any(|m| {
                            let name = m.get("name").and_then(Value::as_str).unwrap_or("");
                            name == model || name == format!("{model}:latest")
                        })
                    });
                if !found {
                    return Err(ModelError::Unavailable(format!(
                        "model {model} not installed; pull it with Ollama"
                    )));
                }
                Ok(serde_json::json!({"provider":"ollama","model":model,"available":true}))
            }
            Provider::OpenAi => {
                let encoded: String =
                    url::form_urlencoded::byte_serialize(self.descriptor.model.as_bytes())
                        .collect();
                self.send(&format!("models/{encoded}"), None).await?;
                Ok(
                    serde_json::json!({"provider":"openai","model":self.descriptor.model,"available":true,"note":"model listing is not an inference/Decisions eligibility test"}),
                )
            }
            Provider::TypeSafeCandidate => {
                self.authorize()?;
                Ok(
                    serde_json::json!({"provider":"typesafe","configured":true,"note":"experimental adapter; use synthetic inference to test endpoint access"}),
                )
            }
        }
    }
}
impl GenerativeModel for HttpModel {
    fn descriptor(&self) -> &ModelDescriptor {
        &self.descriptor
    }
    fn generate<'a>(
        &'a self,
        request: &'a GenerationRequest,
    ) -> ModelFuture<'a, GenerationResponse> {
        Box::pin(async move {
            if request.input.len() + request.instructions.len() > self.max_context {
                return Err(ModelError::InvalidRequest(
                    "generative context exceeds configured budget".into(),
                ));
            }
            let (endpoint, body) = match self.descriptor.provider {
                Provider::OpenAi => (
                    "responses",
                    openai_responses_request(request, &self.descriptor.model),
                ),
                Provider::Ollama => (
                    "api/chat",
                    ollama_chat_request(request, &self.descriptor.model),
                ),
                Provider::TypeSafeCandidate => {
                    return Err(ModelError::InvalidRequest(
                        "TypeSafe is decision-only".into(),
                    ));
                }
            };
            let raw = self.send(endpoint, Some(&body)).await?;
            let mut response = decode_generation(&self.descriptor.provider, &raw)?;
            if self.descriptor.provider == Provider::OpenAi {
                if let Some(schema) = &request.schema {
                    response.text = restore_openai_output(schema, &response.text)?;
                }
            }
            Ok(response)
        })
    }
}
impl DecisionModel for HttpModel {
    fn descriptor(&self) -> &ModelDescriptor {
        &self.descriptor
    }
    fn decide<'a>(&'a self, request: &'a DecisionRequest) -> ModelFuture<'a, DecisionResponse> {
        Box::pin(async move {
            request.validate()?;
            if request.input.len() > self.max_context {
                return Err(ModelError::InvalidRequest(
                    "decision context exceeds budget".into(),
                ));
            }
            let (endpoint, body) = match self.descriptor.provider {
                Provider::OpenAi => (
                    "decisions",
                    openai_decisions_request(request, &self.descriptor.model)?,
                ),
                Provider::Ollama => (
                    "v1/systemone",
                    systemone_request(request, &self.descriptor.model)?,
                ),
                Provider::TypeSafeCandidate => (
                    "systemone",
                    systemone_request(request, &self.descriptor.model)?,
                ),
            };
            let raw = self.send(endpoint, Some(&body)).await?;
            if self.descriptor.provider == Provider::OpenAi {
                openai_decisions_response(request, &raw)
            } else {
                systemone_response(request, &raw)
            }
        })
    }
}

pub fn decode_generation(
    provider: &Provider,
    raw: &Value,
) -> Result<GenerationResponse, ModelError> {
    let model = raw
        .get("model")
        .and_then(Value::as_str)
        .ok_or_else(|| ModelError::InvalidResponse("missing response model".into()))?
        .to_owned();
    let text = match provider {
        Provider::OpenAi => {
            if raw.get("status").and_then(Value::as_str) != Some("completed") {
                return Err(ModelError::Incomplete);
            }
            let output = raw
                .get("output")
                .and_then(Value::as_array)
                .ok_or_else(|| ModelError::InvalidResponse("missing Responses output".into()))?;
            let mut text = Vec::new();
            for item in output {
                if item
                    .get("type")
                    .and_then(Value::as_str)
                    .is_some_and(|t| t.ends_with("_call"))
                {
                    return Err(ModelError::InvalidResponse(
                        "unexpected tool call in a text-only generation".into(),
                    ));
                }
                if let Some(contents) = item.get("content").and_then(Value::as_array) {
                    for content in contents {
                        match content.get("type").and_then(Value::as_str) {
                            Some("refusal") => return Err(ModelError::Refused),
                            Some("output_text") => text.push(
                                content
                                    .get("text")
                                    .and_then(Value::as_str)
                                    .ok_or_else(|| {
                                        ModelError::InvalidResponse("invalid output_text".into())
                                    })?
                                    .to_owned(),
                            ),
                            _ => {}
                        }
                    }
                }
            }
            text.join("\n")
        }
        Provider::Ollama => {
            if raw.get("done").and_then(Value::as_bool) != Some(true)
                || raw.get("done_reason").and_then(Value::as_str) == Some("length")
            {
                return Err(ModelError::Incomplete);
            }
            let message = raw
                .get("message")
                .ok_or_else(|| ModelError::InvalidResponse("missing Ollama message".into()))?;
            if message
                .get("tool_calls")
                .and_then(Value::as_array)
                .is_some_and(|calls| !calls.is_empty())
            {
                return Err(ModelError::InvalidResponse("unexpected tool call".into()));
            }
            message
                .get("content")
                .and_then(Value::as_str)
                .ok_or_else(|| ModelError::InvalidResponse("missing generated text".into()))?
                .to_owned()
        }
        Provider::TypeSafeCandidate => {
            return Err(ModelError::InvalidRequest("decision-only provider".into()));
        }
    };
    if text.trim().is_empty() {
        return Err(ModelError::InvalidResponse("empty generated text".into()));
    }
    Ok(GenerationResponse { model, text })
}
