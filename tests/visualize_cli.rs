mod common;

use common::*;
use lore::engine::{self, UpdateOptions};

#[tokio::test]
async fn viewer_serves_citation_support_and_named_related_records_read_only() {
    use std::sync::atomic::Ordering;

    let (_dir, config, model) = project();
    put(
        &config,
        "ADR-001.md",
        "DECISION database: MySQL is the selected database.\n",
    );
    engine::update(&config, &model, None, UpdateOptions::default())
        .await
        .unwrap();
    put(
        &config,
        "ADR-002.md",
        "DECISION database: PostgreSQL is the selected database.\nREPORT rollout: The deployment was reported complete.\n",
    );
    engine::update(&config, &model, None, UpdateOptions::default())
        .await
        .unwrap();

    let database = config.state.join("state.db");
    let before = std::fs::read(&database).unwrap();
    let calls = model.calls.load(Ordering::SeqCst);
    let viewer = lore::visualize::Viewer::open(&config).unwrap();
    let read_json = |path: &str| {
        let (status, _, bytes) = viewer.reply(path).unwrap();
        assert_eq!(status, 200);
        serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()
    };
    let site = read_json("/api/site");
    for page in site["pages"].as_array().unwrap() {
        assert!(!page["title"].as_str().unwrap().contains('/'));
        let id = percent_encoding::utf8_percent_encode(
            page["id"].as_str().unwrap(),
            percent_encoding::NON_ALPHANUMERIC,
        );
        let rendered = read_json(&format!("/api/page?id={id}"));
        let html = rendered["html"].as_str().unwrap();
        let search_text = page["search_text"].as_str().unwrap();
        assert!(search_text.chars().count() <= 64_000);
        if html.contains("MySQL") {
            assert!(search_text.contains("MySQL"));
        }
        if html.contains("PostgreSQL") {
            assert!(search_text.contains("PostgreSQL"));
        }
        assert!(!html.contains("about:blank"));
        if html.contains("footnote-definition") {
            assert!(html.contains("supporting source"), "{html}");
        }
    }
    let decision = site["decisions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|record| record["summary"].as_str().unwrap().contains("PostgreSQL"))
        .unwrap();
    assert!(decision["topic_title"].is_string());
    assert!(decision["scope"].is_string());
    assert!(decision["support_state"].is_string());
    assert!(decision["source"].as_str().unwrap().contains("ADR-002.md"));
    let record = read_json(&format!(
        "/api/record?id={}",
        decision["id"].as_str().unwrap()
    ));
    assert!(!record["relations"].as_array().unwrap().is_empty());
    for relation in record["relations"].as_array().unwrap() {
        for endpoint in ["from", "to"] {
            assert!(
                record["related_records"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|related| related["id"] == relation[endpoint]
                        && related["title"].is_string())
            );
        }
    }
    let conn = lore::storage::read_only(&database).unwrap();
    let graph = lore::knowledge::build(&conn, &lore::knowledge::ZoomOptions::default()).unwrap();
    let report = graph
        .knowledge
        .iter()
        .find(|record| record.kind == "reported_outcome")
        .unwrap();
    assert!(
        !site["decisions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|record| record["id"] == report.id)
    );
    assert_eq!(
        read_json(&format!("/api/record?id={}", report.id))["record"]["subject"],
        report.subject
    );
    assert!(viewer.reply("/api/record?id=unknown").is_err());
    assert_eq!(model.calls.load(Ordering::SeqCst), calls);
    assert_eq!(std::fs::read(database).unwrap(), before);
}

#[cfg(unix)]
#[tokio::test]
async fn ctrl_c_stops_visualizer_and_releases_listener() {
    use std::{
        io::{Read, Write},
        net::{TcpListener, TcpStream},
        process::{Command, Stdio},
        time::{Duration, Instant},
    };

    let (_dir, config, model) = project();
    put(
        &config,
        "guide.md",
        "DECISION dispatch: Keep the queue bounded.\n",
    );
    engine::update(&config, &model, None, UpdateOptions::default())
        .await
        .unwrap();

    let reservation = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = reservation.local_addr().unwrap();
    drop(reservation);

    let mut child = Command::new(env!("CARGO_BIN_EXE_lore"))
        .arg("--config")
        .arg(&config.config_path)
        .arg("visualize")
        .arg("--port")
        .arg(address.port().to_string())
        .arg("--no-open")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut ready = false;
    while Instant::now() < deadline {
        if let Some(status) = child.try_wait().unwrap() {
            let output = child.wait_with_output().unwrap();
            panic!(
                "visualizer exited before starting ({status}):\n{}",
                String::from_utf8_lossy(&output.stdout)
            );
        }
        if let Ok(mut stream) = TcpStream::connect_timeout(&address, Duration::from_millis(100)) {
            stream
                .set_read_timeout(Some(Duration::from_secs(1)))
                .unwrap();
            stream
                .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n")
                .unwrap();
            let mut response = [0; 12];
            stream.read_exact(&mut response).unwrap();
            assert_eq!(&response, b"HTTP/1.1 200");
            ready = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    if !ready {
        let _ = child.kill();
        let output = child.wait_with_output().unwrap();
        panic!(
            "visualizer did not become ready:\n{}",
            String::from_utf8_lossy(&output.stdout)
        );
    }

    assert_eq!(unsafe { libc::kill(child.id() as i32, libc::SIGINT) }, 0);
    let deadline = Instant::now() + Duration::from_secs(3);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("visualizer did not stop after Ctrl+C");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let output = child.wait_with_output().unwrap();
    assert!(
        status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("Lore visualizer stopped."));
    assert!(TcpListener::bind(address).is_ok());
}
