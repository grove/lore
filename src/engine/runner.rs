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
        })
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
        T: DeserializeOwned,
        F: FnMut(&T) -> Result<()>,
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
            "validated-call-v1",
            task,
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
            if let Ok(text) = util::read_limited(&path, 4_000_000) {
                if let Ok(cached) = serde_json::from_str::<Cached>(&text) {
                    if let Ok(value) = serde_json::from_str::<T>(&cached.text) {
                        if validate(&value).is_ok() {
                            self.cache_hits += 1;
                            self.record(task, &provider, &cached.model, &key, true, 0)?;
                            return Ok((value, cached.model));
                        }
                    }
                }
            }
            std::fs::remove_file(&path)?;
        }
        for attempt in 0..2 {
            let start = Instant::now();
            let instructions = if attempt == 0 {
                instructions.to_owned()
            } else {
                format!(
                    "{instructions}\nThe previous attempt failed validation. Recheck the JSON fields, source quotes, identifiers and completeness. Do not add explanations outside the JSON object."
                )
            };
            let request = GenerationRequest {
                instructions,
                input: serialized.clone(),
                schema: Some(schema.clone()),
            };
            self.calls += 1;
            let response = self
                .generative
                .generate(&request)
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
            if let Ok(value) = decoded {
                match validate(&value) {
                    Ok(()) => {
                        util::atomic_write(
                            &path,
                            &serde_json::to_vec(&Cached {
                                model: response.model.clone(),
                                text: response.text,
                            })?,
                        )?;
                        return Ok((value, response.model));
                    }
                    Err(e) if attempt == 1 => {
                        return Err(e).with_context(|| {
                            format!("{task} output failed validation after repair")
                        });
                    }
                    Err(_) => {}
                }
            } else if attempt == 1 {
                anyhow::bail!("{task} output did not match the required JSON schema after repair");
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
        match model.decide(&request).await {
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
                self.decision_failed = true;
                self.warnings.push("Decision inference unavailable or invalid; continued with generative extraction without dropping any source.".into());
                None
            }
        }
    }
}
