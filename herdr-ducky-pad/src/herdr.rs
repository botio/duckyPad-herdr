//! Minimal herdr socket client.
//!
//! The herdr API socket (`herdr.sock`) handles **one request per connection**:
//! the server reads a single request line, writes a single response line, then
//! drops the stream and closes the connection. The only exceptions are the
//! streaming methods (`events.subscribe`, `events.wait`, `pane.graphics.stream`),
//! which hold the connection open for their duration. See herdr
//! `src/api/server.rs::handle_connection_with_stop`.
//!
//! So every call here opens a fresh Unix stream, sends one request, reads the
//! one response, and closes. There is no long-lived subscription the daemon
//! holds open: a periodic `agent.list` poll is the update mechanism.
//!
//! Wire format is newline-delimited JSON:
//!   request:   `{"id":"<id>","method":"<m>","params":{...}}\n`
//!   response:  `{"id":"<id>","result":{...}}\n`  or  `{"id":"","error":{...}}\n`

use anyhow::{bail, Context, Result};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

/// How long to wait for herdr to answer a single request before giving up.
const RESP_TIMEOUT: Duration = Duration::from_secs(3);

/// A client bound to a specific herdr socket. Holds no open connection — each
/// request opens and closes its own — so it is cheap to keep around for the
/// daemon's whole lifetime.
#[derive(Clone)]
pub struct HerdrClient {
    path: String,
}

impl HerdrClient {
    pub fn new(path: impl AsRef<std::path::Path>) -> Self {
        Self {
            path: path.as_ref().to_string_lossy().into_owned(),
        }
    }

    /// One request → one response, over a fresh connection. Returns the parsed
    /// response value, or an error if herdr reported one (or was unreachable).
    pub fn call(&self, method: &str, params: &serde_json::Value) -> Result<serde_json::Value> {
        let mut stream =
            UnixStream::connect(&self.path).with_context(|| format!("connect {}", self.path))?;
        stream
            .set_read_timeout(Some(RESP_TIMEOUT))
            .with_context(|| "set read timeout")?;
        stream
            .set_write_timeout(Some(RESP_TIMEOUT))
            .with_context(|| "set write timeout")?;

        let mut line =
            serde_json::json!({ "id": "dp", "method": method, "params": params }).to_string();
        line.push('\n');
        stream
            .write_all(line.as_bytes())
            .with_context(|| format!("send {method}"))?;

        // `stream` is moved into a `BufReader`; the read above already sent the
        // request. `read_line` reads until the response's trailing newline.
        let mut reader = BufReader::new(stream);
        let mut resp = String::new();
        let n = reader
            .read_line(&mut resp)
            .with_context(|| format!("read response to {method}"))?;
        if n == 0 && resp.trim().is_empty() {
            bail!("herdr closed the connection without answering {method}");
        }

        let v: serde_json::Value =
            serde_json::from_str(resp.trim()).with_context(|| "parse herdr response")?;
        if let Some(err) = v.get("error") {
            bail!("herdr {method}: {err}");
        }
        Ok(v)
    }

    /// `agent.list` → `{"type":"agent_list","agents":[...]}`.
    pub fn agent_list(&self) -> Result<serde_json::Value> {
        self.call("agent.list", &serde_json::json!({}))
    }

    /// `agent.focus` → focus the pane identified by `target` (a `pane_id`).
    pub fn focus(&self, target: &str) -> Result<serde_json::Value> {
        self.call("agent.focus", &serde_json::json!({ "target": target }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::os::unix::net::UnixListener;

    #[test]
    fn focus_sends_target_pane_id() {
        let path =
            std::env::temp_dir().join(format!("ducky-bridge-focus-test-{}", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let listener = UnixListener::bind(&path).expect("bind mock herdr socket");

        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept focus request");
            let mut request = Vec::new();
            loop {
                let mut byte = [0u8; 1];
                stream.read_exact(&mut byte).expect("read focus request");
                request.push(byte[0]);
                if byte[0] == b'\n' {
                    break;
                }
            }

            let value: serde_json::Value =
                serde_json::from_slice(&request).expect("parse focus request");
            assert_eq!(value["method"], "agent.focus");
            assert_eq!(value["params"]["target"], "w1:p2");
            stream
                .write_all(b"{\"id\":\"dp\",\"result\":{\"type\":\"ok\"}}\n")
                .expect("write focus response");
        });

        let response = HerdrClient::new(&path)
            .focus("w1:p2")
            .expect("focus request succeeds");
        assert_eq!(response["result"]["type"], "ok");

        server.join().expect("mock herdr server exits");
        let _ = std::fs::remove_file(path);
    }
}
