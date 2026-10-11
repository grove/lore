mod common;

use common::*;
use lore::engine::{self, UpdateOptions};

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
            let mut response = [0; 32];
            let count = stream.read(&mut response).unwrap();
            assert!(String::from_utf8_lossy(&response[..count]).starts_with("HTTP/1.1 200"));
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
