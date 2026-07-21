//! End-to-end smoke test for `mjmx lsp`.
//!
//! The language server is also reachable through the standalone
//! `mjmx-language-server` binary (covered by that crate's own `lsp_smoke` test).
//! This test proves the *CLI* entry point is wired to the same server: it spawns
//! the real `mjmx` binary with the `lsp` subcommand and drives a genuine JSON-RPC
//! session over stdio (`Content-Length` framing), exactly as an editor would.

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
    /// Spawn `mjmx lsp` and start decoding its stdout.
    fn start() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_mjmx"))
            .arg("lsp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("failed to spawn `mjmx lsp`");

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
fn mjmx_lsp_serves_a_real_session_over_stdio() {
    let mut session = Session::start();

    // 1. initialize
    session.send(&json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": { "processId": null, "rootUri": null, "capabilities": {} }
    }));
    let initialize = session.wait_for("the initialize response", is_response(1));
    println!(
        "initialize result: {}",
        serde_json::to_string_pretty(&initialize["result"]).unwrap()
    );
    let capabilities = &initialize["result"]["capabilities"];
    // The CLI must reach the *same* server as the standalone binary.
    assert_eq!(
        initialize["result"]["serverInfo"]["name"],
        json!("mjmx-language-server")
    );
    assert_eq!(
        capabilities["textDocumentSync"],
        json!(2),
        "incremental sync"
    );
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

    // 5. an incremental edit that fixes the document clears the squiggles:
    //    wrap the `<mj-text>` line in an `<mj-column>` inside an `<mj-section>`.
    session.send(&json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didChange",
        "params": {
            "textDocument": { "uri": URI, "version": 2 },
            "contentChanges": [{
                "range": {
                    "start": { "line": 2, "character": 0 },
                    "end": { "line": 2, "character": 4 }
                },
                "text": "    <mj-section><mj-column>\n      "
            }, {
                "range": {
                    "start": { "line": 3, "character": 29 },
                    "end": { "line": 3, "character": 29 }
                },
                "text": "</mj-column></mj-section>"
            }]
        }
    }));
    let cleared = session.wait_for("the cleared publishDiagnostics", |message| {
        message.get("method") == Some(&json!("textDocument/publishDiagnostics"))
            && message["params"]["version"] == json!(2)
    });
    println!(
        "publishDiagnostics after incremental edit: {}",
        serde_json::to_string_pretty(&cleared["params"]).unwrap()
    );
    assert_eq!(
        cleared["params"]["diagnostics"],
        json!([]),
        "the incremental edit should have fixed the document"
    );

    // 6. a follow-up request confirms the edited document is the one being served
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
    assert_eq!(
        symbols["result"][0]["children"][0]["children"][0]["name"],
        json!("mj-section")
    );
    println!(
        "documentSymbol: {}",
        serde_json::to_string_pretty(&symbols["result"]).unwrap()
    );

    // 7. shutdown / exit
    session.send(&json!({ "jsonrpc": "2.0", "id": 3, "method": "shutdown" }));
    let shutdown = session.wait_for("the shutdown response", is_response(3));
    assert!(shutdown.get("error").is_none(), "{shutdown:#}");
    println!("shutdown: {}", serde_json::to_string(&shutdown).unwrap());
    session.send(&json!({ "jsonrpc": "2.0", "method": "exit" }));
    session.close_stdin();

    let status = session.wait_for_exit();
    assert!(status.success(), "`mjmx lsp` exited with {status}");
    println!("`mjmx lsp` exited with {status}");
}
