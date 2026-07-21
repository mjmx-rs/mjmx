//! End-to-end smoke test: drive the real `mjmx-language-server` binary over stdio
//! with a genuine JSON-RPC session (`Content-Length` framing).
//!
//! This deliberately does **not** call the library directly — it spawns the shipped
//! executable exactly as an editor would, so the binary, the stdio transport, the
//! framing and the capability handshake are all exercised for real.

use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use serde_json::{Value, json};

/// A known-bad document: `<mj-text>` is not a legal direct child of `<mj-body>`.
const BAD_DOCUMENT: &str =
    "<mjml>\n  <mj-body>\n    <mj-text>oops</mj-text>\n  </mj-body>\n</mjml>\n";
const URI: &str = "file:///smoke/bad.mjml";

/// Fail rather than hang if the server stops talking.
const TIMEOUT: Duration = Duration::from_secs(20);

struct Session {
    child: Child,
    /// `None` once the client end has hung up (an editor closing the pipe).
    stdin: Option<ChildStdin>,
    incoming: mpsc::Receiver<Value>,
    /// Every message received, for the failure report.
    seen: Vec<Value>,
}

impl Session {
    fn start() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_mjmx-language-server"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("failed to spawn mjmx-language-server");

        let stdin = child.stdin.take().expect("stdin");
        let stdout = child.stdout.take().expect("stdout");
        let (sender, incoming) = mpsc::channel();
        thread::spawn(move || read_messages(stdout, &sender));

        Self {
            child,
            stdin: Some(stdin),
            incoming,
            seen: Vec::new(),
        }
    }

    fn send(&mut self, message: &Value) {
        let body = serde_json::to_string(message).unwrap();
        let stdin = self.stdin.as_mut().expect("stdin still open");
        write!(stdin, "Content-Length: {}\r\n\r\n{body}", body.len()).unwrap();
        stdin.flush().unwrap();
    }

    /// Close the client end of the pipe, as an editor does after `exit`.
    fn close_stdin(&mut self) {
        self.stdin = None;
    }

    /// Wait for the process to exit, failing (rather than hanging) on timeout.
    fn wait_for_exit(&mut self) -> std::process::ExitStatus {
        let deadline = std::time::Instant::now() + TIMEOUT;
        loop {
            match self.child.try_wait().expect("try_wait") {
                Some(status) => return status,
                None if std::time::Instant::now() >= deadline => {
                    let _ = self.child.kill();
                    panic!("the server did not exit within {TIMEOUT:?} of `exit`");
                }
                None => thread::sleep(Duration::from_millis(20)),
            }
        }
    }

    /// Read messages until `predicate` matches one, or time out.
    fn wait_for(&mut self, what: &str, predicate: impl Fn(&Value) -> bool) -> Value {
        loop {
            let message = self.incoming.recv_timeout(TIMEOUT).unwrap_or_else(|_| {
                panic!(
                    "timed out waiting for {what}; messages so far:\n{}",
                    serde_json::to_string_pretty(&self.seen).unwrap()
                )
            });
            self.seen.push(message.clone());
            if predicate(&message) {
                return message;
            }
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Decode `Content-Length`-framed JSON-RPC messages off `stdout`.
fn read_messages(stdout: ChildStdout, sender: &mpsc::Sender<Value>) {
    let mut reader = BufReader::new(stdout);
    loop {
        let mut length: Option<usize> = None;
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => return,
                Ok(_) => {}
                Err(_) => return,
            }
            let trimmed = line.trim_end_matches(['\r', '\n']);
            if trimmed.is_empty() {
                break;
            }
            if let Some(value) = trimmed.strip_prefix("Content-Length:") {
                length = value.trim().parse().ok();
            }
        }
        let Some(length) = length else { return };
        let mut body = vec![0u8; length];
        if reader.read_exact(&mut body).is_err() {
            return;
        }
        let Ok(value) = serde_json::from_slice::<Value>(&body) else {
            return;
        };
        if sender.send(value).is_err() {
            return;
        }
    }
}

fn is_response(id: i64) -> impl Fn(&Value) -> bool {
    move |message: &Value| message.get("id").and_then(Value::as_i64) == Some(id)
}

#[test]
fn a_real_session_over_stdio_publishes_diagnostics_for_a_bad_document() {
    let mut session = Session::start();

    // 1. initialize
    session.send(&json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": { "processId": null, "rootUri": null, "capabilities": {} }
    }));
    let initialize = session.wait_for("the initialize response", is_response(1));
    let capabilities = &initialize["result"]["capabilities"];
    assert_eq!(
        initialize["result"]["serverInfo"]["name"],
        json!("mjmx-language-server")
    );
    assert_eq!(capabilities["textDocumentSync"], json!(1), "full sync");
    assert!(capabilities["completionProvider"].is_object());
    assert_eq!(capabilities["hoverProvider"], json!(true));
    assert_eq!(capabilities["documentFormattingProvider"], json!(true));
    assert_eq!(capabilities["documentSymbolProvider"], json!(true));

    // 2. initialized
    session.send(&json!({ "jsonrpc": "2.0", "method": "initialized", "params": {} }));

    // 3. didOpen with a document the linter rejects
    session.send(&json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didOpen",
        "params": { "textDocument": {
            "uri": URI, "languageId": "mjml", "version": 1, "text": BAD_DOCUMENT
        }}
    }));

    // 4. the squiggle comes back on the right line, with the right message
    let published = session.wait_for("publishDiagnostics", |message| {
        message.get("method") == Some(&json!("textDocument/publishDiagnostics"))
    });
    let params = &published["params"];
    assert_eq!(params["uri"], json!(URI));
    let diagnostics = params["diagnostics"]
        .as_array()
        .expect("a diagnostics array");
    assert_eq!(diagnostics.len(), 1, "{params:#}");
    let diagnostic = &diagnostics[0];
    assert_eq!(diagnostic["severity"], json!(1), "error");
    assert_eq!(diagnostic["source"], json!("mjmx"));
    assert!(
        diagnostic["message"].as_str().unwrap().contains("mj-text"),
        "{diagnostic:#}"
    );
    // `    <mj-text>` on the third line (0-based line 2), column 4.
    assert_eq!(
        diagnostic["range"]["start"],
        json!({"line": 2, "character": 4})
    );

    println!(
        "publishDiagnostics: {}",
        serde_json::to_string_pretty(params).unwrap()
    );

    // 5. a follow-up request over the same session
    session.send(&json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "textDocument/documentSymbol",
        "params": { "textDocument": { "uri": URI } }
    }));
    let symbols = session.wait_for("the documentSymbol response", is_response(2));
    assert_eq!(symbols["result"][0]["name"], json!("mjml"));
    assert_eq!(
        symbols["result"][0]["children"][0]["name"],
        json!("mj-body")
    );
    println!(
        "documentSymbol: {}",
        serde_json::to_string_pretty(&symbols["result"]).unwrap()
    );

    // 6. shutdown / exit
    session.send(&json!({ "jsonrpc": "2.0", "id": 3, "method": "shutdown" }));
    let shutdown = session.wait_for("the shutdown response", is_response(3));
    assert!(shutdown.get("error").is_none(), "{shutdown:#}");
    session.send(&json!({ "jsonrpc": "2.0", "method": "exit" }));
    session.close_stdin();

    let status = session.wait_for_exit();
    assert!(status.success(), "server exited with {status}");
}
