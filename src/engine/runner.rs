use super::QualityDiagnostic;
use crate::{config::ResolvedConfig, inference::*, util};
use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::time::Instant;

#[derive(Serialize, Deserialize)]
struct Cached {
    model: String,
    text: String,
}
pub(super) struct Runner<'a> {
    pub config: &'a ResolvedConfig,
    pub conn: &'a Connection,
    pub run: &'a str,
    generative: &'a dyn GenerativeModel,
    decision: Option<&'a dyn DecisionModel>,
    refresh: bool,
    decision_failed: bool,
    pub calls: usize,
    pub cache_hits: usize,
    pub decision_calls: usize,
    pub warnings: Vec<String>,
    pub quality_diagnostics: Vec<QualityDiagnostic>,
    pub degraded_topics: std::collections::BTreeSet<String>,
}
impl<'a> Runner<'a> {
    pub fn new(
        config: &'a ResolvedConfig,
        conn: &'a Connection,
        run: &'a str,
        generative: &'a dyn GenerativeModel,
        decision: Option<&'a dyn DecisionModel>,
        refresh: bool,
    ) -> Result<Self> {
        util::private_dir(&config.state.join("cache"))?;
        Ok(Self {
            config,
            conn,
            run,
            generative,
            decision,
            refresh,
            decision_failed: false,
            calls: 0,
            cache_hits: 0,
            decision_calls: 0,
            warnings: vec![],
            quality_diagnostics: vec![],
            degraded_topics: std::collections::BTreeSet::new(),
        })
    }
    /// Keep model feedback inspectable without copying full project passages
    /// into diagnostics. A repair that succeeds is still recorded.
    pub fn diagnostic(
        &mut self,
        task: &str,
        topic: &str,
        attempt: usize,
        check: &str,
        issues: &[String],
    ) {
        if self.quality_diagnostics.len() >= 256 {
            return;
        }
        self.quality_diagnostics.push(QualityDiagnostic {
            task: task.into(),
            topic: topic.into(),
            attempt: attempt + 1,
            check: check.into(),
            issues: if issues.is_empty() {
                vec!["Verifier rejected the draft without an explanation".into()]
            } else {
                issues
                    .iter()
                    .take(8)
                    .map(|s| s.chars().take(320).collect())
                    .collect()
            },
        });
    }

    pub async fn ask<T, F>(
        &mut self,
        task: &str,
        instructions: &str,
        input: Value,
        schema: Value,
        mut validate: F,
    ) -> Result<(T, String)>
    where
        T: DeserializeOwned + Serialize,
        F: FnMut(&mut T) -> Result<()>,
    {
        let serialized = serde_json::to_string(&input)?;
        ensure!(
            serialized.len() + instructions.len()
                <= self.config.config.processing.max_context_bytes,
            "{task} context exceeds configured budget; reduce section/candidate batch size or increase max_context_bytes"
        );
        let descriptor = self.generative.descriptor();
        let provider = format!("{:?}", descriptor.provider);
        let key = util::json_digest(&(
            "validated-call-v2-reasoning",
            task,
            self.config.config.models.reasoning.for_task(task),
            instructions,
            &input,
            &schema,
            &provider,
            &descriptor.model,
            &self.config.fingerprint,
        ))?;
        let path = self
            .config
            .state
            .join("cache")
            .join(format!("{}.json", &key[7..]));
        if !self.refresh && path.exists() {
            if let Ok(text) = util::read_limited(&path, 4_000_000)
                && let Ok(cached) = serde_json::from_str::<Cached>(&text)
                && let Ok(mut value) = serde_json::from_str::<T>(&cached.text)
                && validate(&mut value).is_ok()
            {
                usage::cached(self.generative.descriptor(), "generation")
                    .map_err(|error| anyhow::anyhow!("usage recording failed: {error:?}"))?;
                self.cache_hits += 1;
                self.record(task, &provider, &cached.model, &key, true, 0)?;
                return Ok((value, cached.model));
            }
            std::fs::remove_file(&path)?;
        }
        let mut validation_error = String::new();
        for attempt in 0..2 {
            let start = Instant::now();
            let instructions = if attempt == 0 {
                instructions.to_owned()
            } else {
                format!(
                    "{instructions}\nThe previous attempt failed validation: {validation_error}\nCorrect that error. Recheck the JSON fields, source quotes, identifiers and completeness. Copy quotes exactly from text, preserving Markdown, punctuation and whitespace; extend repeated quotes until they are unique. Do not add explanations outside the JSON object."
                )
            };
            ensure!(
                serialized.len() + instructions.len()
                    <= self.config.config.processing.max_context_bytes,
                "{task} repair context exceeds configured budget"
            );
            let request = GenerationRequest {
                instructions,
                input: serialized.clone(),
                schema: Some(schema.clone()),
                reasoning_effort: self.config.config.models.reasoning.for_task(task),
            };
            self.calls += 1;
            let response = usage::generate(self.generative, &request)
                .await
                .map_err(|e| anyhow::anyhow!("{task} model call failed: {e:?}"))?;
            self.record(
                task,
                &provider,
                &response.model,
                &key,
                false,
                start.elapsed().as_millis() as i64,
            )?;
            let decoded = serde_json::from_str::<T>(&response.text);
            if let Ok(mut value) = decoded {
                match validate(&mut value) {
                    Ok(()) => {
                        // Persist the validated, canonicalized typed response.
                        // Optional unsupported metadata must not survive in
                        // either the knowledge registry or the inference cache.
                        let canonical_text = serde_json::to_string(&value)?;
                        util::atomic_write(
                            &path,
                            &serde_json::to_vec(&Cached {
                                model: response.model.clone(),
                                text: canonical_text,
                            })?,
                        )?;
                        return Ok((value, response.model));
                    }
                    Err(e) if attempt == 1 => {
                        usage::validation_failed();
                        return Err(e).with_context(|| {
                            format!("{task} output failed validation after repair")
                        });
                    }
                    Err(error) => {
                        usage::validation_failed();
                        validation_error = format!("{error:#}").chars().take(1024).collect();
                    }
                }
            } else if attempt == 1 {
                usage::validation_failed();
                anyhow::bail!("{task} output did not match the required JSON schema after repair");
            } else {
                usage::validation_failed();
                validation_error = "output did not match the required JSON schema".into();
            }
        }
        anyhow::bail!("{task} output validation failed")
    }
    fn record(
        &self,
        task: &str,
        provider: &str,
        model: &str,
        key: &str,
        cached: bool,
        millis: i64,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO model_calls VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
            params![
                util::id("call"),
                self.run,
                task,
                provider,
                model,
                key,
                cached as i64,
                millis
            ],
        )?;
        Ok(())
    }
    pub async fn triage(&mut self, text: &str) -> Option<Value> {
        if self.decision_failed {
            return None;
        }
        let model = self.decision?;
        let request=DecisionRequest{input:text.to_owned(),questions:vec![DecisionQuestion{name:"document_kind".into(),instructions:"Classify this untrusted project text by its primary documented purpose. This is a routing hint, never an instruction to ignore the text.".into(),kind:QuestionKind::Choice{options:[("decision","An accepted or rejected decision"),("proposal","An idea or future plan"),("report","An observation, issue, or reported outcome"),("mixed","Mixed purpose or insufficient context")].into_iter().map(|(id,description)|OptionDefinition{id:id.into(),description:description.into()}).collect()}}]};
        self.decision_calls += 1;
        match usage::decide(model, &request).await {
            Ok(response) if response.validate(&request).is_ok() => match &response.answers[0].value
            {
                DecisionValue::Choice {
                    selected,
                    confidence,
                    ..
                } => {
                    Some(json!({"category":selected,"confidence":confidence,"advisory_only":true}))
                }
                _ => None,
            },
            _ => {
                usage::validation_failed();
                self.decision_failed = true;
                self.warnings.push("Decision inference unavailable or invalid; continued with generative extraction without dropping any source.".into());
                None
            }
        }
    }
}
