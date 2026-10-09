use lore::inference::*;
use lore::provider_wire::*;
use serde_json::json;

fn sample() -> DecisionRequest {
    DecisionRequest {
        input: "ADR replaces MySQL with PostgreSQL".into(),
        questions: vec![
            DecisionQuestion {
                name: "is_relevant".into(),
                instructions: "Does this change project knowledge?".into(),
                kind: QuestionKind::Predicate,
            },
            DecisionQuestion {
                name: "topic".into(),
                instructions: "Which area?".into(),
                kind: QuestionKind::Choice {
                    options: vec![
                        OptionDefinition {
                            id: "database".into(),
                            description: "Data storage".into(),
                        },
                        OptionDefinition {
                            id: "auth".into(),
                            description: "Access controls".into(),
                        },
                    ],
                },
            },
            DecisionQuestion {
                name: "impact".into(),
                instructions: "How impactful?".into(),
                kind: QuestionKind::Score {
                    levels: vec![
                        OptionDefinition {
                            id: "low".into(),
                            description: "Minor".into(),
                        },
                        OptionDefinition {
                            id: "high".into(),
                            description: "Architectural".into(),
                        },
                    ],
                },
            },
        ],
    }
}
#[test]
fn openai_and_systemone_wire_shapes_are_distinct() {
    let r = sample();
    let openai = openai_decisions_request(&r, "gpt-6-luna").unwrap();
    let local = systemone_request(&r, "clef-flash").unwrap();
    assert_eq!(openai["questions"][0]["type"], "predicate");
    assert_eq!(openai["questions"][1]["choices"][0]["value"], "database");
    assert_eq!(local["questions"]["is_relevant"]["type"], "noul");
    assert_eq!(
        local["questions"]["topic"]["criteria"]["database"],
        "Data storage"
    );
    assert_eq!(
        local["questions"]["impact"]["criteria"][1],
        "high: Architectural"
    );
}
#[test]
fn both_decision_backends_normalize_to_same_answers() {
    let r = sample();
    let a = json!({"model":"gpt-6-luna","answers":[
        {"name":"is_relevant","type":"predicate","probability":0.9},
        {"name":"topic","type":"choice","choice":"database","confidence":0.8,"probabilities":[
            {"value":"database","probability":0.8},{"value":"auth","probability":0.2}]},
        {"name":"impact","type":"score","score":0.6,"confidence":0.5,"probabilities":[
            {"value":0,"label":"low","probability":0.4},{"value":1,"label":"high","probability":0.6}]}
    ]});
    let b = json!({"model":"clef-flash","answers":{
        "is_relevant":{"type":"noul","noul":0.9},
        "topic":{"type":"choice","choice":"database","confidence":0.8,"probabilities":{"database":0.8,"auth":0.2}},
        "impact":{"type":"score","score":0.6,"confidence":0.5,"probabilities":{"0":0.4,"1":0.6}}
    }});
    let x = openai_decisions_response(&r, &a).unwrap();
    let y = systemone_response(&r, &b).unwrap();
    assert_eq!(x.answers, y.answers);
}
#[test]
fn refusal_and_bad_probabilities_do_not_become_negative_answers() {
    let r = DecisionRequest {
        input: "content".into(),
        questions: vec![DecisionQuestion {
            name: "check".into(),
            instructions: "Is it relevant?".into(),
            kind: QuestionKind::Predicate,
        }],
    };
    let refused = json!({"model":"gpt-6-luna","answers":[{"name":"check","type":"refusal"}]});
    let parsed = openai_decisions_response(&r, &refused).unwrap();
    assert_eq!(parsed.answers[0].value, DecisionValue::Refusal);
    let invalid = json!({"model":"gpt-6-luna","answers":[{"name":"check","type":"predicate","probability":1.3}]});
    assert!(openai_decisions_response(&r, &invalid).is_err());
}
#[test]
fn malformed_requests_and_wrong_choice_are_rejected() {
    let mut r = sample();
    r.questions[1].name = "is_relevant".into();
    assert!(r.validate().is_err());
    let r = sample();
    let raw = json!({"model":"gpt-6-luna","answers":[
        {"name":"is_relevant","type":"predicate","probability":0.9},
        {"name":"topic","type":"choice","choice":"unknown","probabilities":[
            {"value":"database","probability":0.8},{"value":"auth","probability":0.2}]},
        {"name":"impact","type":"score","score":0.6,"probabilities":[
            {"value":0,"probability":0.4},{"value":1,"probability":0.6}]}
    ]});
    assert!(openai_decisions_response(&r, &raw).is_err());
}
#[test]
fn generative_requests_are_provider_specific_and_remote_is_opt_in() {
    let req = GenerationRequest { reasoning_effort: Some(ReasoningEffort::Low),
        instructions: "Extract assertions".into(),
        input: "An ADR replaces another".into(),
        schema: Some(
            json!({"type":"object","properties":{"facts":{"type":"array","items":{"type":"string"}}},
            "required":["facts"],"additionalProperties":false}),
        ),
    };
    let hosted = openai_responses_request(&req, "gpt-6-astra");
    let local = ollama_chat_request(&req, "gemma4:12b");
    assert_eq!(hosted["text"]["format"]["strict"], true);
    assert_eq!(hosted["store"], false);
    assert_eq!(hosted["reasoning"]["effort"], "low");
    assert!(local.get("reasoning").is_none());
    assert_eq!(local["stream"], false);
    assert!(local["format"].is_object());
    let descriptor = ModelDescriptor {
        provider: Provider::OpenAi,
        model: "gpt-6-astra".into(),
        location: ExecutionLocation::Hosted,
    };
    assert_eq!(
        EgressPolicy::LocalOnly.authorize(&descriptor),
        Err(ModelError::RemoteDisabled)
    );
}
#[allow(dead_code)]
fn ensure_object_safe_traits(_: &dyn GenerativeModel, _: &dyn DecisionModel) {}

#[test]
fn openai_uses_passage_ids_without_changing_ollama_schema() {
    let schema = lore::domain::extraction_schema_for("First source line.\nSecond source line.");
    let request = GenerationRequest { reasoning_effort: None,
        instructions: "Extract assertions".into(),
        input: "First source line.\nSecond source line.".into(),
        schema: Some(schema.clone()),
    };
    let hosted = openai_responses_request(&request, "gpt-6-luna");
    let properties =
        &hosted["text"]["format"]["schema"]["properties"]["assertions"]["items"]["properties"];
    assert_eq!(properties["quote"]["type"], "string");
    assert_eq!(
        properties["quote"]["enum"],
        json!(["lore_passage_0", "lore_passage_1", "lore_passage_2"])
    );
    assert!(properties["quote"].get("pattern").is_none());
    let table: serde_json::Value =
        serde_json::from_str(hosted["input"][2]["content"].as_str().unwrap()).unwrap();
    assert_eq!(
        table["source_passages"][1]["text"],
        "First source line.\nSecond source line."
    );
    let output =
        json!({"assertions":[{"quote":"lore_passage_1","statement":"unchanged"}]}).to_string();
    let restored: serde_json::Value = serde_json::from_str(
        &lore::provider_wire::restore_openai_output(&schema, &output).unwrap(),
    )
    .unwrap();
    assert_eq!(
        restored["assertions"][0]["quote"],
        table["source_passages"][1]["text"]
    );
    assert_eq!(restored["assertions"][0]["statement"], "unchanged");
    let invented = json!({"assertions":[{"quote":"lore_passage_999"}]}).to_string();
    assert!(lore::provider_wire::restore_openai_output(&schema, &invented).is_err());
    assert_eq!(
        properties["kind"],
        schema["properties"]["assertions"]["items"]["properties"]["kind"]
    );
    assert_eq!(
        ollama_chat_request(&request, "gemma4:12b")["format"],
        schema
    );
    assert_eq!(request.schema, Some(schema));
}

#[test]
fn openai_passage_ids_preserve_source_bytes_and_empty_quotes() {
    let request = GenerationRequest { reasoning_effort: None,
        instructions: "Reconcile assertions".into(),
        input: "Source text".into(),
        schema: Some(json!({"type":"string","enum":["", "(a)[b]{c}.*+?^$|\\\r\n\t"]})),
    };
    let hosted = openai_responses_request(&request, "gpt-6-luna");
    assert_eq!(
        hosted["text"]["format"]["schema"]["enum"],
        json!(["", "lore_passage_0"])
    );
    let schema = request.schema.as_ref().unwrap();
    assert_eq!(
        lore::provider_wire::restore_openai_output(schema, "\"\"").unwrap(),
        "\"\""
    );
    let restored =
        lore::provider_wire::restore_openai_output(schema, "\"lore_passage_0\"").unwrap();
    assert_eq!(
        serde_json::from_str::<String>(&restored).unwrap(),
        "(a)[b]{c}.*+?^$|\\\r\n\t"
    );
}
