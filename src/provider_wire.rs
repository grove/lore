//! Pure JSON wire adapters. No HTTP or API-key access occurs here.
use crate::inference::*;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

fn invalid(msg: &str) -> ModelError {
    ModelError::InvalidResponse(msg.into())
}
fn string<'a>(v: &'a Value, key: &str) -> Result<&'a str, ModelError> {
    v.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("missing string"))
}
fn number(v: &Value, key: &str) -> Result<f64, ModelError> {
    v.get(key)
        .and_then(Value::as_f64)
        .ok_or_else(|| invalid("missing number"))
}
fn optional_number(v: &Value, key: &str) -> Result<Option<f64>, ModelError> {
    match v.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(x) => x
            .as_f64()
            .map(Some)
            .ok_or_else(|| invalid("invalid number")),
    }
}

/// OpenAI Decisions: input and an array of named predicate/choice/score questions.
pub fn openai_decisions_request(r: &DecisionRequest, model: &str) -> Result<Value, ModelError> {
    r.validate()?;
    let questions:Vec<Value>=r.questions.iter().map(|q|match &q.kind {
        QuestionKind::Predicate=>json!({"name":q.name,"type":"predicate","instructions":q.instructions}),
        QuestionKind::Choice{options}=>json!({"name":q.name,"type":"choice","instructions":q.instructions,
            "choices":options.iter().map(|o|json!({"value":o.id,"description":o.description})).collect::<Vec<_>>()}),
        QuestionKind::Score{levels}=>json!({"name":q.name,"type":"score","instructions":q.instructions,
            "levels":levels.iter().map(|o|json!({"label":o.id,"description":o.description})).collect::<Vec<_>>()}),
    }).collect();
    Ok(json!({"model":model,"input":r.input,"questions":questions}))
}

/// System One: state and a named map; the yes/no primitive is called noul.
pub fn systemone_request(r: &DecisionRequest, model: &str) -> Result<Value, ModelError> {
    r.validate()?;
    let mut questions = Map::new();
    for q in &r.questions {
        let obj = match &q.kind {
            QuestionKind::Predicate => json!({"type":"noul","instructions":q.instructions}),
            QuestionKind::Choice { options } => {
                let map: Map<String, Value> = options
                    .iter()
                    .map(|o| (o.id.clone(), json!(o.description)))
                    .collect();
                json!({"type":"choice","instructions":q.instructions,"criteria":map})
            }
            QuestionKind::Score { levels } => json!({"type":"score","instructions":q.instructions,
                "criteria":levels.iter().map(|o|format!("{}: {}",o.id,o.description)).collect::<Vec<_>>()}),
        };
        questions.insert(q.name.clone(), obj);
    }
    Ok(json!({"model":model,"state":r.input,"questions":questions}))
}

pub fn openai_decisions_response(
    r: &DecisionRequest,
    raw: &Value,
) -> Result<DecisionResponse, ModelError> {
    let array = raw
        .get("answers")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("expected answer array"))?;
    let mut answers = Vec::new();
    for answer in array {
        let name = string(answer, "name")?.to_owned();
        let value = match string(answer, "type")? {
            "predicate" => DecisionValue::Predicate(number(answer, "probability")?),
            "choice" => {
                let mut probabilities = BTreeMap::new();
                for item in answer
                    .get("probabilities")
                    .and_then(Value::as_array)
                    .ok_or_else(|| invalid("missing probabilities"))?
                {
                    let id = string(item, "value")?.to_owned();
                    if probabilities
                        .insert(id, number(item, "probability")?)
                        .is_some()
                    {
                        return Err(invalid("duplicate probability"));
                    }
                }
                DecisionValue::Choice {
                    selected: string(answer, "choice")?.into(),
                    probabilities,
                    confidence: optional_number(answer, "confidence")?,
                }
            }
            "score" => {
                let mut distribution = BTreeMap::new();
                for item in answer
                    .get("probabilities")
                    .and_then(Value::as_array)
                    .ok_or_else(|| invalid("missing score distribution"))?
                {
                    let i = item
                        .get("value")
                        .and_then(Value::as_u64)
                        .ok_or_else(|| invalid("invalid score index"))?;
                    if distribution
                        .insert(i, number(item, "probability")?)
                        .is_some()
                    {
                        return Err(invalid("duplicate score index"));
                    }
                }
                DecisionValue::Score {
                    weighted_index: number(answer, "score")?,
                    probabilities: indexed(distribution)?,
                    confidence: optional_number(answer, "confidence")?,
                }
            }
            "refusal" => DecisionValue::Refusal,
            _ => return Err(invalid("unknown decision answer")),
        };
        answers.push(DecisionAnswer { name, value });
    }
    let result = DecisionResponse {
        model: string(raw, "model")?.into(),
        answers,
    };
    result.validate(r)?;
    let mut result = result;
    result.answers.sort_by_key(|answer| {
        r.questions
            .iter()
            .position(|q| q.name == answer.name)
            .unwrap()
    });
    Ok(result)
}

pub fn systemone_response(
    r: &DecisionRequest,
    raw: &Value,
) -> Result<DecisionResponse, ModelError> {
    let map = raw
        .get("answers")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("expected answer map"))?;
    if map.len() != r.questions.len() {
        return Err(invalid("unexpected System One answer count"));
    }
    let mut answers = Vec::new();
    // Normalize to the request's declared question order. System One answers
    // arrive in a JSON map, whose iteration order need not match the request.
    for q in &r.questions {
        let name = &q.name;
        let answer = map
            .get(name)
            .ok_or_else(|| invalid("missing System One answer"))?;
        let value = match string(answer, "type")? {
            "noul" => DecisionValue::Predicate(number(answer, "noul")?),
            "choice" => DecisionValue::Choice {
                selected: string(answer, "choice")?.into(),
                probabilities: prob_map(answer)?,
                confidence: optional_number(answer, "confidence")?,
            },
            "score" => {
                let mut dist = BTreeMap::new();
                for (index, p) in prob_map(answer)? {
                    let index = index
                        .parse::<u64>()
                        .map_err(|_| invalid("invalid score index"))?;
                    if dist.insert(index, p).is_some() {
                        return Err(invalid("duplicate score index"));
                    }
                }
                DecisionValue::Score {
                    weighted_index: number(answer, "score")?,
                    probabilities: indexed(dist)?,
                    confidence: optional_number(answer, "confidence")?,
                }
            }
            "refusal" => DecisionValue::Refusal,
            _ => return Err(invalid("unknown System One answer")),
        };
        answers.push(DecisionAnswer {
            name: name.clone(),
            value,
        });
    }
    let result = DecisionResponse {
        model: string(raw, "model")?.into(),
        answers,
    };
    result.validate(r)?;
    Ok(result)
}
fn prob_map(answer: &Value) -> Result<BTreeMap<String, f64>, ModelError> {
    let map = answer
        .get("probabilities")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("missing probability map"))?;
    map.iter()
        .map(|(id, v)| {
            v.as_f64()
                .map(|p| (id.clone(), p))
                .ok_or_else(|| invalid("invalid probability"))
        })
        .collect()
}
fn indexed(map: BTreeMap<u64, f64>) -> Result<Vec<f64>, ModelError> {
    let mut values = Vec::new();
    for (i, p) in map {
        if i != values.len() as u64 {
            return Err(invalid("gapped score levels"));
        }
        values.push(p);
    }
    Ok(values)
}

/// Generative request bodies: Responses uses strict text.format; Ollama uses format.
pub fn openai_responses_request(r: &GenerationRequest, model: &str) -> Value {
    let mut body = json!({"model":model,"store":false,"input":[
        {"role":"system","content":r.instructions},{"role":"user","content":r.input}]});
    if let Some(schema) = &r.schema {
        body["text"] = json!({"format":{"type":"json_schema","name":"lore_extract","schema":schema,"strict":true}});
    }
    body
}
pub fn ollama_chat_request(r: &GenerationRequest, model: &str) -> Value {
    let mut body = json!({"model":model,"stream":false,"messages":[
        {"role":"system","content":r.instructions},{"role":"user","content":r.input}]});
    if let Some(schema) = &r.schema {
        body["format"] = schema.clone();
    }
    body
}
