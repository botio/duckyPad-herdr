//! End-to-end test with NO hardware: spawns the daemon in DRY RUN against a mock
//! herdr Unix socket that implements the REAL protocol — one request per
//! connection (the server answers a single request, then drops the connection).
//! The daemon polls `agent.list`; the test flips an agent's state in the served
//! list and verifies the daemon re-polls and emits a new (re-sorted) RGB frame.

use parking_lot::Mutex;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixListener;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

fn socket_path() -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("ducky-bridge-test-{}", std::process::id()));
    let _ = std::fs::remove_file(&p);
    p
}

fn payload_hex(line: &str) -> Option<String> {
    let i = line.find("payload=")?;
    Some(line[i + "payload=".len()..].to_string())
}

fn hex_to_bytes(hex: &str) -> Vec<u8> {
    let clean: String = hex.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    (0..clean.len() / 2)
        .map(|i| u8::from_str_radix(&clean[i * 2..i * 2 + 2], 16).unwrap_or(0))
        .collect()
}

/// Seed agents: A=blocked, B=working, C=idle.
const SEED: &str = r#"[{"pane_id":"w1:p1","agent_status":"blocked","display_agent":"A"},{"pane_id":"w1:p2","agent_status":"working","display_agent":"B"},{"pane_id":"w1:p3","agent_status":"idle","display_agent":"C"}]"#;
/// After C transitions to blocked: A=blocked, B=working, C=blocked.
const V2: &str = r#"[{"pane_id":"w1:p1","agent_status":"blocked","display_agent":"A"},{"pane_id":"w1:p2","agent_status":"working","display_agent":"B"},{"pane_id":"w1:p3","agent_status":"blocked","display_agent":"C"}]"#;

/// Poll `lines` until `pred` holds or the deadline passes.
fn wait_for<F: Fn(&Vec<String>) -> bool>(
    lines: &Arc<Mutex<Vec<String>>>,
    pred: F,
    deadline: Duration,
) -> bool {
    let end = Instant::now() + deadline;
    loop {
        if pred(&lines.lock()) {
            return true;
        }
        if Instant::now() > end {
            return false;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn full_loop_mock_socket() {
    let sock = socket_path();
    let listener = UnixListener::bind(&sock).expect("bind mock socket");
    let lines: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    // The served `agent.list` body; the test swaps it to drive a state change.
    let state: Arc<Mutex<String>> = Arc::new(Mutex::new(SEED.to_string()));

    let mut child = Command::new(env!("CARGO_BIN_EXE_ducky-pad-bridge"))
        .env("HERDR_SOCKET_PATH", sock.to_str().unwrap())
        .env("DUCKY_DRY_RUN", "1")
        .env("RUST_LOG", "debug")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn daemon");
    let stderr = child.stderr.take().expect("take stderr");

    // Reader thread: drain the daemon's stderr into `lines`.
    {
        let lr = lines.clone();
        std::thread::spawn(move || {
            let mut r = BufReader::new(stderr);
            let mut b = String::new();
            while let Ok(n) = r.read_line(&mut b) {
                if n == 0 {
                    break;
                }
                lr.lock().push(b.trim_end_matches(['\r', '\n']).to_string());
                b.clear();
            }
        });
    }

    // Server thread: the real herdr model — ONE request per connection. Accept
    // in a loop; each connection gets exactly one request, one response, then
    // is dropped.
    {
        let st = state.clone();
        std::thread::spawn(move || {
            for _ in 0..500 {
                let (mut stream, _) = match listener.accept() {
                    Ok(s) => s,
                    Err(_) => break,
                };
                // Read one request line.
                let mut acc: Vec<u8> = Vec::new();
                let mut tmp = [0u8; 4096];
                loop {
                    match stream.read(&mut tmp) {
                        Ok(0) => break,
                        Ok(k) => {
                            acc.extend_from_slice(&tmp[..k]);
                            if acc.iter().any(|&b| b == b'\n') {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
                let line = String::from_utf8_lossy(&acc).trim().to_string();
                let (id, method) = match serde_json::from_str::<serde_json::Value>(&line) {
                    Ok(v) => (
                        v.get("id")
                            .and_then(|x| x.as_str())
                            .unwrap_or("null")
                            .to_string(),
                        v.get("method")
                            .and_then(|x| x.as_str())
                            .unwrap_or("")
                            .to_string(),
                    ),
                    Err(_) => continue,
                };
                let resp = match method.as_str() {
                    "agent.list" => {
                        let body = st.lock().clone();
                        format!(
                            "{{\"id\":\"{id}\",\"result\":{{\"type\":\"agent_list\",\"agents\":{body}}}}}"
                        )
                    }
                    "agent.focus" => format!("{{\"id\":\"{id}\",\"result\":{{\"type\":\"ok\"}}}}"),
                    _ => format!("{{\"id\":\"{id}\",\"result\":{{}}}}"),
                };
                let mut out = resp;
                out.push('\n');
                let _ = stream.write_all(out.as_bytes());
                drop(stream); // one-shot: close after the response
            }
        });
    }

    // First poll is immediate -> seed frame (A red, B green, C dim).
    assert!(
        wait_for(
            &lines,
            |l| l.iter().any(|x| x.contains("cmd=34")),
            Duration::from_secs(15)
        ),
        "daemon never emitted an RGB frame; log:\n{}",
        lines.lock().join("\n")
    );

    // Flip C idle -> blocked; the next ~2s poll picks it up and re-sorts.
    *state.lock() = V2.to_string();
    assert!(
        wait_for(
            &lines,
            |l| l.iter().filter(|x| x.contains("cmd=34")).count() >= 2,
            Duration::from_secs(15)
        ),
        "daemon never emitted the second frame; log:\n{}",
        lines.lock().join("\n")
    );

    let _ = child.kill();
    let _ = child.wait();
    let _ = std::fs::remove_file(&sock);

    let collected = lines.lock().clone();
    let dump = collected.join("\n");
    assert!(dump.contains("cmd=35"), "OLED text never emitted:\n{dump}");

    let frames: Vec<Vec<u8>> = collected
        .iter()
        .filter(|l| l.contains("cmd=34") && l.contains("payload="))
        .filter_map(|l| payload_hex(l).as_deref().map(hex_to_bytes))
        .filter(|b| b.len() == 45)
        .collect();
    assert!(
        frames.len() >= 2,
        "expected >=2 RGB frames, got {}:\n{dump}",
        frames.len()
    );

    // Frame 0 (seed): slot0=A blocked(red), slot1=B working(green), slot2=C idle(dim).
    let f0 = &frames[0];
    assert_eq!([f0[0], f0[1], f0[2]], [255, 0, 0], "slot0 red (A blocked)");
    assert_eq!(
        [f0[3], f0[4], f0[5]],
        [0, 255, 0],
        "slot1 green (B working)"
    );
    assert_eq!([f0[6], f0[7], f0[8]], [48, 48, 48], "slot2 dim (C idle)");

    // Frame 1 (C -> blocked): re-sorted to A(blocked,p1), C(blocked,p3), B(working,p2).
    let f1 = &frames[1];
    assert_eq!([f1[0], f1[1], f1[2]], [255, 0, 0], "slot0 red (A)");
    assert_eq!(
        [f1[3], f1[4], f1[5]],
        [255, 0, 0],
        "slot1 red (C now blocked)"
    );
    assert_eq!([f1[6], f1[7], f1[8]], [0, 255, 0], "slot2 green (B)");
}
