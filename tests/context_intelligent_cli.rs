mod common;

use common::*;
use lore::{
    config::{ModelRole, ProviderSettings, ResolvedConfig},
    context::count_tokens,
    engine::{self, UpdateOptions},
};
use serde_json::{Value, json};
use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread,
    time::Duration,
};

struct ModelServer {
    address: String,
    calls: Arc<AtomicUsize>,
    stopped: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

impl ModelServer {
    fn new(invent_citation: bool) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = format!("http://{}", listener.local_addr().unwrap());
        listener.set_nonblocking(true).unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let stopped = Arc::new(AtomicBool::new(false));
        let seen = calls.clone();
        let stop = stopped.clone();
        let handle = thread::spawn(move || {
            while !stop.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        // Explicitly reset the mode inherited from the
                        // nonblocking listener on BSD/macOS; request parsing
                        // below uses blocking reads with bounded timeouts.
                        stream.set_nonblocking(false).unwrap();
                        stream
                            .set_read_timeout(Some(Duration::from_secs(5)))
                            .unwrap();
                        let Some((path, body)) = read_request(&mut stream) else {
                            continue;
                        };
                        seen.fetch_add(1, Ordering::SeqCst);
                        let (status, response) = if path == "/api/embed" {
                            (
                                503,
                                json!({"error":"synthetic unavailable embedding model"}),
                            )
                        } else {
                            assert_eq!(path, "/api/chat");
                            let input: Value = serde_json::from_str(
                                body["messages"][1]["content"].as_str().unwrap(),
                            )
                            .unwrap();
                            assert_eq!(input["task"], "context_synthesis");
                            let source = &input["source_context"];
                            let record = &source["knowledge"][0];
                            let evidence = record["evidence_ids"].clone();
                            let checks: Vec<_> = source["adopted_constraints"].as_object().unwrap().iter()
                                .map(|(id, ids)| json!({"knowledge_id":id,"disposition":"preserved","explanation":"Retain the documented queue arrangement.","evidence_ids":ids})).collect();
                            let draft = json!({
                                "preferred_approach": {"text":format!("Keep the documented arrangement: {}", record["statement"].as_str().unwrap()), "evidence_ids": if invent_citation { json!(["ev_invented"]) } else { evidence.clone() }},
                                "known_record_ids":[record["id"]],
                                "inferred":[],"recommended":[],"risks":[],
                                "next_steps":[{"text":"Inspect the documented queue before adjusting its configuration.","evidence_ids":evidence}],
                                "constraint_checks":checks,
                            });
                            (
                                200,
                                json!({"model":"context-fixture","done":true,"message":{"role":"assistant","content":draft.to_string()}}),
                            )
                        };
                        let body = response.to_string();
                        let _ = write!(
                            stream,
                            "HTTP/1.1 {status} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        );
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5))
                    }
                    Err(error) => panic!("mock listener failed: {error}"),
                }
            }
        });
        Self {
            address,
            calls,
            stopped,
            thread: Some(handle),
        }
    }
}

impl Drop for ModelServer {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::SeqCst);
        if let Some(handle) = self.thread.take() {
            if let Err(error) = handle.join() {
                if !thread::panicking() {
                    std::panic::resume_unwind(error);
                }
            }
        }
    }
}

fn read_request(stream: &mut TcpStream) -> Option<(String, Value)> {
    let mut bytes = Vec::new();
    let mut buffer = [0; 4096];
    let header_end = loop {
        let length = stream.read(&mut buffer).ok()?;
        if length == 0 {
            return None;
        }
        bytes.extend_from_slice(&buffer[..length]);
        if let Some(index) = bytes.windows(4).position(|value| value == b"\r\n\r\n") {
            break index + 4;
        }
        assert!(bytes.len() < 100_000);
    };
    let headers = String::from_utf8(bytes[..header_end].to_vec()).unwrap();
    let length = headers
        .lines()
        .find_map(|line| {
            line.split_once(':')
                .filter(|(key, _)| key.eq_ignore_ascii_case("content-length"))
                .map(|(_, value)| value.trim().parse::<usize>().unwrap())
        })
        .unwrap();
    while bytes.len() < header_end + length {
        let count = stream.read(&mut buffer).ok()?;
        if count == 0 {
            return None;
        }
        bytes.extend_from_slice(&buffer[..count]);
    }
    Some((
        headers
            .lines()
            .next()
            .unwrap()
            .split_whitespace()
            .nth(1)
            .unwrap()
            .into(),
        serde_json::from_slice(&bytes[header_end..header_end + length]).unwrap(),
    ))
}

async fn fixture(server: &ModelServer) -> (tempfile::TempDir, ResolvedConfig, FakeModel) {
    let (temp, mut config, model) = project();
    config.config.models.generative.model = "context-fixture".into();
    config.config.processing.timeout_seconds = 1;
    config.config.processing.retry_attempts = 0;
    config.config.providers.insert(
        "ollama".into(),
        ProviderSettings {
            base_url: Some(server.address.clone()),
            api_key_env: None,
        },
    );
    fs::write(
        &config.config_path,
        serde_yaml::to_string(&config.config).unwrap(),
    )
    .unwrap();
    let config = ResolvedConfig::load(&config.config_path).unwrap();
    put(
        &config,
        "design.md",
        "DECISION workers: Worker events use one durable queue.\n",
    );
    engine::update(&config, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    (temp, config, model)
}

fn run(config: &ResolvedConfig, extra: &[&str]) -> (Value, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_lore"))
        .arg("--config")
        .arg(&config.config_path)
        .args(["--json", "context", "Adjust the worker queue"])
        .args(extra)
        .env_remove("OPENAI_API_KEY")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let text = String::from_utf8(output.stdout).unwrap();
    (serde_json::from_str(&text).unwrap(), text)
}

#[tokio::test]
async fn default_cli_returns_guidance_reuses_cache_and_refreshes_changed_evidence() {
    let server = ModelServer::new(false);
    let (_temp, config, model) = fixture(&server).await;
    let database = fs::read(config.state.join("state.db")).unwrap();
    let source = fs::read(config.base.join("docs/design.md")).unwrap();
    let (first, text) = run(&config, &[]);
    assert_eq!(first["schema_version"], 3);
    assert_eq!(first["mode"], "intelligent", "{text}");
    assert_eq!(first["model_calls"], 1);
    assert_eq!(first["cache_status"], "miss");
    assert!(
        first["brief"]["preferred_approach"]["text"]
            .as_str()
            .unwrap()
            .contains("one durable queue")
    );
    assert!(count_tokens(&text) <= 3000);
    let (second, _) = run(&config, &[]);
    assert_eq!(second["cache_status"], "hit");
    assert_eq!(second["model_calls"], 0);
    assert_eq!(second["brief"], first["brief"]);
    assert_eq!(server.calls.load(Ordering::SeqCst), 1);
    let (fast, first_fast) = run(&config, &["--fast"]);
    let (_, second_fast) = run(&config, &["--fast"]);
    assert_eq!(first_fast, second_fast);
    assert_eq!(fast["schema_version"], 2);
    assert_eq!(fast["model_calls"], 0);
    assert!(fast.get("brief").is_none());
    let (uncached, _) = run(&config, &["--no-cache"]);
    assert_eq!(uncached["mode"], "intelligent");
    assert_eq!(uncached["cache_status"], "disabled");
    assert_eq!(server.calls.load(Ordering::SeqCst), 2);
    assert_eq!(database, fs::read(config.state.join("state.db")).unwrap());
    assert_eq!(
        source,
        fs::read(config.base.join("docs/design.md")).unwrap()
    );
    for reference in first["evidence"].as_array().unwrap() {
        let output = Command::new(env!("CARGO_BIN_EXE_lore"))
            .arg("--config")
            .arg(&config.config_path)
            .args(["--json", "evidence", reference["id"].as_str().unwrap()])
            .output()
            .unwrap();
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains("one durable queue"));
    }
    put(
        &config,
        "design.md",
        "DECISION workers: Worker events use one durable queue with a bounded buffer.\n",
    );
    engine::update(&config, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    let (updated, text) = run(&config, &[]);
    assert_eq!(updated["mode"], "intelligent", "{text}");
    assert_eq!(updated["cache_status"], "miss");
    assert_ne!(updated["evidence"], first["evidence"]);
    assert_eq!(server.calls.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn embedding_failure_still_returns_lexical_guidance_with_visible_degradation() {
    let server = ModelServer::new(false);
    let (_temp, config, _) = fixture(&server).await;
    let mut changed = config.config.clone();
    changed.models.embedding = Some(ModelRole {
        model: "embedding-fixture".into(),
        ..ModelRole::default()
    });
    fs::write(
        &config.config_path,
        serde_yaml::to_string(&changed).unwrap(),
    )
    .unwrap();
    let (result, text) = run(&config, &[]);
    assert_eq!(result["mode"], "intelligent", "{text}");
    assert_eq!(result["model_calls"], 2);
    assert!(
        result["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|warning| warning
                .as_str()
                .unwrap()
                .contains("Semantic retrieval unavailable"))
    );
    assert_eq!(server.calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn unsafe_or_disabled_model_configuration_falls_back_without_network_or_cache() {
    let server = ModelServer::new(false);
    let (_temp, config, _) = fixture(&server).await;
    for (provider, name, enabled) in [
        ("ollama", "fixture:cloud", true),
        ("openai", "hosted-fixture", true),
        ("ollama", "context-fixture", false),
    ] {
        let mut changed = config.config.clone();
        changed.models.generative = ModelRole {
            provider: provider.into(),
            model: name.into(),
            enabled,
        };
        fs::write(
            &config.config_path,
            serde_yaml::to_string(&changed).unwrap(),
        )
        .unwrap();
        let (result, text) = run(&config, &["--max-tokens", "512"]);
        assert_eq!(result["mode"], "fast_fallback", "{text}");
        assert_eq!(result["schema_version"], 3);
        assert_eq!(result["model_calls"], 0);
        assert!(count_tokens(&text) <= 512);
    }
    assert_eq!(server.calls.load(Ordering::SeqCst), 0);
    assert!(!config.state.join("context-cache").exists());
    assert!(!config.state.join("semantic.sqlite3").exists());
}

#[tokio::test]
async fn too_small_inference_budget_falls_back_before_any_calls_or_cache_writes() {
    let server = ModelServer::new(false);
    let (_temp, config, _) = fixture(&server).await;
    let mut changed = config.config.clone();
    changed.processing.max_context_bytes = 1000;
    changed.models.embedding = Some(ModelRole {
        model: "embedding-fixture".into(),
        ..ModelRole::default()
    });
    fs::write(
        &config.config_path,
        serde_yaml::to_string(&changed).unwrap(),
    )
    .unwrap();
    let (result, text) = run(&config, &[]);
    assert_eq!(result["mode"], "fast_fallback", "{text}");
    assert!(
        result["fallback_reason"]
            .as_str()
            .unwrap()
            .contains("input budget")
    );
    assert_eq!(server.calls.load(Ordering::SeqCst), 0);
    assert!(!config.state.join("context-cache").exists());
    assert!(!config.state.join("semantic.sqlite3").exists());
}

#[tokio::test]
async fn fabricated_model_citation_falls_back_and_does_not_poison_cache() {
    let server = ModelServer::new(true);
    let (_temp, config, _) = fixture(&server).await;
    let (result, text) = run(&config, &[]);
    assert_eq!(result["mode"], "fast_fallback", "{text}");
    assert_eq!(result["model_calls"], 1);
    assert!(!text.contains("ev_invented"));
    assert!(!config.state.join("context-cache").exists());
}
