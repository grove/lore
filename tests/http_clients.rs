mod common;
use common::*;
use lore::{
    config::{ModelRole, ProviderSettings, ResolvedConfig},
    http::{HttpModel, decode_generation},
    inference::*,
};
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    process::Command,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

// Isolate fixture runtimes and subprocesses across test-harness workers. The
// compiler itself deliberately uses sequential inference in this release.
static HTTP_FIXTURE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone)]
struct Request {
    path: String,
    body: Value,
    authorized: bool,
}
struct Server {
    address: String,
    requests: Arc<Mutex<Vec<Request>>>,
    stopped: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}
impl Server {
    fn new(f: impl Fn(&Request, usize) -> (u16, Value) + Send + 'static) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = format!("http://{}/", listener.local_addr().unwrap());
        listener.set_nonblocking(true).unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let stopped = Arc::new(AtomicBool::new(false));
        let reqs = requests.clone();
        let stop = stopped.clone();
        let handle = thread::spawn(move || {
            while !stop.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        stream
                            .set_read_timeout(Some(Duration::from_secs(5)))
                            .unwrap();
                        stream
                            .set_write_timeout(Some(Duration::from_secs(5)))
                            .unwrap();
                        // A client can close an accepted connection before sending
                        // a request. This is a normal network race, not a malformed
                        // test HTTP call; keep the fixture listener alive.
                        let Some(request) = read_request(&mut stream) else {
                            continue;
                        };
                        let count = {
                            let mut r = reqs.lock().unwrap();
                            r.push(request.clone());
                            r.len()
                        };
                        let (status, body) = f(&request, count);
                        let body = body.to_string();
                        let _ = write!(
                            stream,
                            "HTTP/1.1 {status} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\nRetry-After: 0\r\n\r\n{body}",
                            if status == 200 { "OK" } else { "Error" },
                            body.len()
                        );
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5))
                    }
                    Err(e) => panic!("mock server failed: {e}"),
                }
            }
        });
        Self {
            address,
            requests,
            stopped,
            handle: Some(handle),
        }
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::SeqCst);
        if let Some(h) = self.handle.take() {
            if let Err(panic) = h.join() {
                // A second panic during unwinding aborts the test executable and
                // hides the original HTTP/client failure on Windows.
                if !thread::panicking() {
                    std::panic::resume_unwind(panic);
                }
            }
        }
    }
}
fn read_request(stream: &mut TcpStream) -> Option<Request> {
    let mut bytes = Vec::new();
    let mut buffer = [0; 4096];
    let header_end;
    loop {
        let n = match stream.read(&mut buffer) {
            Ok(0) => return None,
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return None,
        };
        bytes.extend_from_slice(&buffer[..n]);
        if let Some(i) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
            header_end = i + 4;
            break;
        }
        assert!(bytes.len() < 100_000);
    }
    let headers = String::from_utf8(bytes[..header_end].to_vec()).unwrap();
    let size = headers
        .lines()
        .find_map(|l| {
            l.split_once(':')
                .filter(|(k, _)| k.eq_ignore_ascii_case("content-length"))
                .map(|(_, v)| v.trim().parse::<usize>().unwrap())
        })
        .unwrap_or(0);
    while bytes.len() < header_end + size {
        let n = match stream.read(&mut buffer) {
            Ok(0) => return None,
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return None,
        };
        bytes.extend_from_slice(&buffer[..n]);
    }
    Some(Request {
        path: headers
            .lines()
            .next()
            .unwrap()
            .split_whitespace()
            .nth(1)
            .unwrap()
            .into(),
        body: if size == 0 {
            Value::Null
        } else {
            serde_json::from_slice(&bytes[header_end..header_end + size]).unwrap()
        },
        authorized: headers
            .to_lowercase()
            .contains("authorization: bearer test-key"),
    })
}
fn configured(
    cfg: &ResolvedConfig,
    server: &Server,
    provider: &str,
) -> (ResolvedConfig, ModelRole) {
    let mut config = cfg.config.clone();
    config.privacy.local_only = provider == "ollama";
    config.processing.retry_attempts = 1;
    let role = ModelRole {
        provider: provider.into(),
        model: if provider == "ollama" {
            "fixture-v1".into()
        } else {
            "hosted-test".into()
        },
        enabled: true,
    };
    if provider != "typesafe" {
        config.models.generative = role.clone();
    } else {
        config.models.decision = Some(role.clone());
    }
    config.providers.insert(
        provider.into(),
        ProviderSettings {
            base_url: Some(format!(
                "{}{}",
                server.address,
                if provider == "ollama" { "" } else { "v1/" }
            )),
            api_key_env: None,
        },
    );
    (
        ResolvedConfig::resolve(config, &cfg.config_path).unwrap(),
        role,
    )
}
fn question() -> DecisionRequest {
    DecisionRequest {
        input: "Synthetic text".into(),
        questions: vec![DecisionQuestion {
            name: "check".into(),
            instructions: "Is this synthetic?".into(),
            kind: QuestionKind::Predicate,
        }],
    }
}
#[tokio::test]
async fn calls_openai_responses_with_strict_schema_and_no_storage() {
    let _serial = HTTP_FIXTURE_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let (_dir, cfg, _) = project();
    let server = Server::new(|r, _| {
        assert_eq!(r.path, "/v1/responses");
        assert!(r.authorized);
        assert_eq!(r.body["store"], false);
        assert_eq!(r.body["text"]["format"]["strict"], true);
        assert_eq!(r.body["reasoning"]["effort"], "high");
        (
            200,
            json!({"model":"resolved-version","status":"completed","output":[{"type":"message","content":[{"type":"output_text","text":"{\"ok\":true}"}]}]}),
        )
    });
    let (c, role) = configured(&cfg, &server, "openai");
    let model = HttpModel::new(&c, &role)
        .unwrap()
        .with_credential("test-key".into());
    let r=model.generate(&GenerationRequest{ reasoning_effort: Some(ReasoningEffort::High),instructions:"Return JSON".into(),input:"Synthetic test".into(),schema:Some(json!({"type":"object","properties":{"ok":{"type":"boolean"}},"required":["ok"],"additionalProperties":false}))}).await.unwrap();
    assert_eq!(r.model, "resolved-version");
    assert_eq!(serde_json::from_str::<Value>(&r.text).unwrap()["ok"], true);
}
#[tokio::test]
async fn openai_http_restores_source_passages_before_validation() {
    let _serial = HTTP_FIXTURE_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let (_dir, cfg, _) = project();
    let quote = "Synthetic source: use `database`.\r\nKeep the qualifier (local-only).";
    let request = GenerationRequest {
        reasoning_effort: None,
        instructions: "Extract evidence".into(),
        input: "Synthetic test".into(),
        schema: Some(
            json!({"type":"object","properties":{"quote":{"type":"string","enum":[quote]}},"required":["quote"],"additionalProperties":false}),
        ),
    };
    let server = Server::new(move |request, _| {
        assert_eq!(request.path, "/v1/responses");
        assert_eq!(request.body["text"]["format"]["strict"], true);
        assert_eq!(
            request.body["text"]["format"]["schema"]["properties"]["quote"]["enum"],
            json!(["lore_passage_0"])
        );
        let table: Value =
            serde_json::from_str(request.body["input"][2]["content"].as_str().unwrap()).unwrap();
        assert_eq!(table["source_passages"][0]["text"], quote);
        (
            200,
            json!({"model":"resolved-version","status":"completed","output":[{"type":"message","content":[{"type":"output_text","text":"{\"quote\":\"lore_passage_0\"}"}]}]}),
        )
    });
    let (config, role) = configured(&cfg, &server, "openai");
    let model = HttpModel::new(&config, &role)
        .unwrap()
        .with_credential("test-key".into());
    let response = model.generate(&request).await.unwrap();
    let output: Value = serde_json::from_str(&response.text).unwrap();
    assert_eq!(output["quote"], quote);
}
#[tokio::test]
#[ignore = "requires LORE_OPENAI_LIVE_CONFIG and its configured credential environment variable"]
async fn openai_live_preserves_multiline_quote_constraints() {
    let path = std::env::var("LORE_OPENAI_LIVE_CONFIG").unwrap();
    let config = ResolvedConfig::load(std::path::Path::new(&path)).unwrap();
    let role = &config.config.models.generative;
    assert_eq!(role.provider, "openai");
    let model = HttpModel::new(&config, role).unwrap();
    let quote = "Synthetic source: use `database`.\r\nKeep the qualifier (local-only).";
    let request = GenerationRequest {
        reasoning_effort: None,
        instructions: "Return a JSON object whose quote is only the word fabricated.".into(),
        input: "Synthetic constraint test; no project material.".into(),
        schema: Some(
            json!({"type":"object","properties":{"quote":{"type":"string","enum":[quote]}},"required":["quote"],"additionalProperties":false}),
        ),
    };
    let response = model.generate(&request).await.unwrap();
    let output: Value = serde_json::from_str(&response.text).unwrap();
    assert_eq!(output["quote"], quote);
}
#[tokio::test]
async fn decision_http_paths_and_predicate_mappings_match_each_backend() {
    let _serial = HTTP_FIXTURE_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let (_dir, cfg, _) = project();
    for provider in ["openai", "ollama", "typesafe"] {
        let p = provider.to_owned();
        let server = Server::new(move |r, _| {
            if p == "openai" {
                assert_eq!(r.path, "/v1/decisions");
                assert_eq!(r.body["questions"][0]["type"], "predicate");
                (
                    200,
                    json!({"model":"hosted-test","answers":[{"name":"check","type":"predicate","probability":0.9}]}),
                )
            } else {
                assert_eq!(r.path, "/v1/systemone");
                assert_eq!(r.body["questions"]["check"]["type"], "noul");
                (
                    200,
                    json!({"model":"fixture-v1","answers":{"check":{"type":"noul","noul":0.9}}}),
                )
            }
        });
        let (c, role) = configured(&cfg, &server, provider);
        let model = HttpModel::new(&c, &role)
            .unwrap()
            .with_credential("test-key".into());
        let result = model.decide(&question()).await.unwrap();
        assert_eq!(result.answers[0].value, DecisionValue::Predicate(0.9));
    }
}
#[tokio::test]
async fn retries_transient_errors_and_withholds_failure_body() {
    let _serial = HTTP_FIXTURE_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let (_dir, cfg, _) = project();
    let server = Server::new(|_, attempt| {
        if attempt == 1 {
            (503, json!({"error":"secret source excerpt"}))
        } else {
            (
                200,
                json!({"model":"fixture-v1","done":true,"message":{"role":"assistant","content":"done"}}),
            )
        }
    });
    let (c, role) = configured(&cfg, &server, "ollama");
    let model = HttpModel::new(&c, &role).unwrap();
    let request = GenerationRequest {
        reasoning_effort: None,
        instructions: "Answer".into(),
        input: "Synthetic".into(),
        schema: None,
    };
    assert_eq!(model.generate(&request).await.unwrap().text, "done");
    assert_eq!(server.requests.lock().unwrap().len(), 2);
    let server = Server::new(|_, _| (401, json!({"error":"must-not-be-logged"})));
    let (c, role) = configured(&cfg, &server, "ollama");
    let error = HttpModel::new(&c, &role)
        .unwrap()
        .generate(&request)
        .await
        .unwrap_err();
    assert!(!format!("{error:?}").contains("must-not-be-logged"));
    assert_eq!(server.requests.lock().unwrap().len(), 1);
}
#[test]
fn generation_refusals_truncation_and_tool_calls_are_not_success() {
    let refusal = json!({"model":"m","status":"completed","output":[{"content":[{"type":"refusal","refusal":"No"}]}]});
    assert!(matches!(
        decode_generation(&Provider::OpenAi, &refusal),
        Err(ModelError::Refused)
    ));
    let incomplete = json!({"model":"m","status":"incomplete","output":[]});
    assert!(matches!(
        decode_generation(&Provider::OpenAi, &incomplete),
        Err(ModelError::Incomplete)
    ));
    let truncated =
        json!({"model":"m","done":true,"done_reason":"length","message":{"content":"partial"}});
    assert!(decode_generation(&Provider::Ollama, &truncated).is_err());
    let tools = json!({"model":"m","done":true,"message":{"content":"","tool_calls":[{}]}});
    assert!(decode_generation(&Provider::Ollama, &tools).is_err());
}
#[test]
fn local_only_rejects_remote_hosts_before_network_io() {
    let (_dir, cfg, _) = project();
    let mut config = cfg.config.clone();
    config.providers.insert(
        "ollama".into(),
        ProviderSettings {
            base_url: Some("https://example.invalid".into()),
            api_key_env: None,
        },
    );
    let c = ResolvedConfig::resolve(config, &cfg.config_path).unwrap();
    assert!(matches!(
        HttpModel::new(&c, &c.config.models.generative),
        Err(ModelError::RemoteDisabled)
    ));
}
#[test]
fn real_cli_compiles_over_http_then_updates_with_server_offline() {
    let _serial = HTTP_FIXTURE_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let (_dir, cfg, _) = project();
    put(
        &cfg,
        "decision.md",
        "# Database\nDECISION database: MySQL is the selected database.\n",
    );
    let fake = FakeModel::new();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let server = Server::new(move |request, _| {
        assert_eq!(request.path, "/api/chat");
        assert_eq!(request.body["stream"], false);
        assert!(request.body.get("reasoning").is_none());
        let r = GenerationRequest {
            reasoning_effort: None,
            instructions: request.body["messages"][0]["content"]
                .as_str()
                .unwrap()
                .into(),
            input: request.body["messages"][1]["content"]
                .as_str()
                .unwrap()
                .into(),
            schema: request.body.get("format").cloned(),
        };
        let answer = runtime.block_on(fake.generate(&r)).unwrap();
        (
            200,
            json!({"model":"fixture-v1","done":true,"done_reason":"stop","message":{"role":"assistant","content":answer.text}}),
        )
    });
    let (c, _) = configured(&cfg, &server, "ollama");
    std::fs::write(&cfg.config_path, serde_yaml::to_string(&c.config).unwrap()).unwrap();
    let binary = env!("CARGO_BIN_EXE_lore");
    let run = |args: &[&str]| {
        Command::new(binary)
            .arg("--config")
            .arg(&cfg.config_path)
            .arg("--json")
            .args(args)
            .output()
            .unwrap()
    };
    let first = run(&["init"]);
    assert!(
        first.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&first.stdout),
        String::from_utf8_lossy(&first.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&first.stdout).unwrap()["knowledge_units"],
        1
    );
    drop(server);
    let noop = run(&["update"]);
    assert!(
        noop.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&noop.stdout),
        String::from_utf8_lossy(&noop.stderr)
    );
    let result: Value = serde_json::from_slice(&noop.stdout).unwrap();
    assert_eq!(result["no_op"], true);
    assert_eq!(result["model_calls"], 0);
    assert!(run(&["status"]).status.success());
    assert!(run(&["audit"]).status.success());
    assert!(run(&["search", "MySQL"]).status.success());
    assert!(run(&["read", "database"]).status.success());
}
