use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

// Exercise the shipped stdin/stdout protocol, including flushes and exit status.
// A deadline and Drop cleanup keep a broken server from hanging the test runner.
struct Server {
    child: Child,
    input: Option<ChildStdin>,
    replies: Receiver<Value>,
    reader: Option<JoinHandle<()>>,
}

impl Server {
    fn new() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_runa"))
            .arg("lsp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let input = child.stdin.take();
        let output = child.stdout.take().unwrap();
        let (sender, replies) = mpsc::channel();
        let reader = thread::spawn(move || {
            let mut output = BufReader::new(output);
            loop {
                let mut header = String::new();
                if output.read_line(&mut header).unwrap() == 0 {
                    return;
                }
                let length: usize = header
                    .strip_prefix("Content-Length: ")
                    .expect("server stdout must contain framed messages only")
                    .trim()
                    .parse()
                    .unwrap();
                header.clear();
                output.read_line(&mut header).unwrap();
                assert_eq!(header, "\r\n");
                let mut body = vec![0; length];
                output.read_exact(&mut body).unwrap();
                let message: Value = serde_json::from_slice(&body).unwrap();
                assert_eq!(message["jsonrpc"], "2.0");
                if sender.send(message).is_err() {
                    return;
                }
            }
        });
        Self {
            child,
            input,
            replies,
            reader: Some(reader),
        }
    }

    fn raw(&mut self, bytes: &[u8]) {
        let input = self.input.as_mut().unwrap();
        input.write_all(bytes).unwrap();
        input.flush().unwrap();
    }

    fn body(&mut self, body: &[u8]) {
        self.raw(format!("Content-Length: {}\r\n\r\n", body.len()).as_bytes());
        self.raw(body);
    }

    fn send(&mut self, value: Value) {
        self.body(&serde_json::to_vec(&value).unwrap());
    }

    fn receive(&self) -> Value {
        self.replies
            .recv_timeout(Duration::from_secs(15))
            .expect("server exited or did not flush a response within 15 seconds")
    }

    fn initialize(&mut self) {
        self.send(json!({"jsonrpc":"2.0", "id":1, "method":"initialize",
            "params":{"processId":null, "rootUri":null, "capabilities":{}}}));
        let response = self.receive();
        assert_eq!(response["id"], 1);
        assert_eq!(
            response["result"]["capabilities"]["textDocumentSync"]["change"],
            1
        );
        assert!(response["result"]["capabilities"]
            .get("diagnosticProvider")
            .is_none());
        self.send(json!({"jsonrpc":"2.0", "method":"initialized", "params":{}}));
    }

    fn error(&self, id: Value, code: i32) {
        let response = self.receive();
        assert_eq!(response["id"], id, "{response}");
        assert_eq!(response["error"]["code"], code, "{response}");
        assert!(response["error"]["message"]
            .as_str()
            .is_some_and(|s| !s.is_empty()));
        assert!(response.get("result").is_none(), "{response}");
    }

    fn shutdown(&mut self) {
        self.send(json!({"jsonrpc":"2.0", "id":"shutdown", "method":"shutdown"}));
        assert_eq!(
            self.receive(),
            json!({"jsonrpc":"2.0", "id":"shutdown", "result":null})
        );
    }

    fn exit(&mut self) {
        self.send(json!({"jsonrpc":"2.0", "method":"exit"}));
    }

    fn finish(mut self, code: i32, transport_error: bool) {
        self.input.take();
        let deadline = Instant::now() + Duration::from_secs(15);
        let status = loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                break status;
            }
            assert!(Instant::now() < deadline, "server failed to exit");
            thread::sleep(Duration::from_millis(10));
        };
        let mut stderr = String::new();
        self.child
            .stderr
            .take()
            .unwrap()
            .read_to_string(&mut stderr)
            .unwrap();
        assert_eq!(status.code(), Some(code), "{stderr}");
        if transport_error {
            assert!(stderr.contains("runa lsp:"), "{stderr}");
        } else {
            assert!(stderr.is_empty(), "{stderr}");
        }
        self.reader.take().unwrap().join().unwrap();
        assert!(
            self.replies.try_iter().collect::<Vec<_>>().is_empty(),
            "unexpected extra response"
        );
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn initialized_server_returns_errors_for_unknown_requests_but_not_notifications() {
    let mut server = Server::new();
    server.initialize();
    server.send(json!({"jsonrpc":"2.0", "method":"unknownNotification"}));
    server.send(json!({"jsonrpc":"2.0", "id":99, "result":null}));
    server.send(json!({"jsonrpc":"2.0", "id":99, "error":{"code":-32601,"message":"unsupported"}}));
    server.send(json!({"jsonrpc":"2.0", "method":"$/cancelRequest", "params":{"id":99}}));
    for (id, method) in [
        (json!(42), "unknownMethod"),
        (json!("ø😀"), "textDocument/diagnostic"),
    ] {
        server.send(json!({"jsonrpc":"2.0", "id":id, "method":method, "params":{}}));
        server.error(id, -32601);
    }
    server.shutdown();
    server.exit();
    server.finish(0, false);
}

#[test]
fn malformed_json_is_reported_and_the_next_frame_is_processed() {
    let mut server = Server::new();
    server.initialize();
    for body in [
        b"{".as_slice(),
        b"",
        b"\xff",
        b"{\"jsonrpc\":\"2.0\",\"id\":2,}",
    ] {
        server.body(body);
        server.error(Value::Null, -32700);
    }
    server.send(json!({"jsonrpc":"2.0", "id":2, "method":"unknown"}));
    server.error(json!(2), -32601);
    server.shutdown();
    server.exit();
    server.finish(0, false);
}

#[test]
fn mixed_case_headers_and_utf8_byte_lengths_are_accepted() {
    let mut server = Server::new();
    let body = serde_json::to_vec(&json!({"jsonrpc":"2.0", "id":"ø😀", "method":"initialize",
        "params":{"processId":null, "rootUri":null, "capabilities":{}}}))
    .unwrap();
    server.raw(
        format!(
            "content-type: application/vscode-jsonrpc; charset=utf-8\r\ncOnTeNt-LeNgTh: {}\r\n\r\n",
            body.len()
        )
        .as_bytes(),
    );
    server.raw(&body);
    let response = server.receive();
    assert_eq!(response["id"], "ø😀");
    assert_eq!(response["result"]["capabilities"]["hoverProvider"], true);
    server.shutdown();
    server.exit();
    server.finish(0, false);
}

#[test]
fn shutdown_rejects_requests_and_stops_document_notifications_until_exit() {
    let mut server = Server::new();
    server.initialize();
    server.shutdown();
    server.send(
        json!({"jsonrpc":"2.0", "method":"textDocument/didOpen", "params":{
        "textDocument":{"uri":"file:///lsp-lifecycle.runa", "version":1, "text":"= x = missing"}}}),
    );
    for method in [
        "textDocument/hover",
        "unknown",
        "shutdown",
        "initialize",
        "exit",
    ] {
        server.send(json!({"jsonrpc":"2.0", "id":method, "method":method, "params":{}}));
        server.error(json!(method), -32600);
    }
    server.exit();
    server.finish(0, false);
}

#[test]
fn exit_without_a_shutdown_request_is_unsuccessful() {
    for initialized in [false, true] {
        let mut server = Server::new();
        if initialized {
            server.initialize();
        }
        // A notification with the request-only method must not shut down.
        server.send(json!({"jsonrpc":"2.0", "method":"shutdown"}));
        server.exit();
        server.finish(1, false);
    }
}

#[test]
fn closing_stdin_without_shutdown_is_unsuccessful() {
    let server = Server::new();
    server.finish(1, false);
    let mut server = Server::new();
    server.initialize();
    server.shutdown();
    server.finish(0, false);
}

#[test]
fn initialize_is_required_once_and_early_notifications_are_ignored() {
    let mut server = Server::new();
    server.send(
        json!({"jsonrpc":"2.0", "method":"textDocument/didOpen", "params":{
        "textDocument":{"uri":"file:///lsp-early.runa", "version":1, "text":"= x = missing"}}}),
    );
    server.send(json!({"jsonrpc":"2.0", "id":2, "method":"textDocument/hover", "params":{}}));
    server.error(json!(2), -32002);
    server.initialize();
    server.send(json!({"jsonrpc":"2.0", "id":3, "method":"initialize", "params":{}}));
    server.error(json!(3), -32600);
    server.shutdown();
    server.exit();
    server.finish(0, false);
}

#[test]
fn invalid_rpc_envelopes_are_not_successful_calls() {
    let mut server = Server::new();
    server.initialize();
    for message in [
        json!(null),
        json!(42),
        json!([]),
        json!({}),
        json!({"method":"initialize"}),
        json!({"jsonrpc":"2.0", "id":true, "method":"shutdown"}),
        json!({"jsonrpc":"2.0", "method":7}),
    ] {
        server.send(message);
        server.error(Value::Null, -32600);
    }
    server.shutdown();
    server.exit();
    server.finish(0, false);
}

#[test]
fn invalid_or_incomplete_framing_exits_with_a_transport_error() {
    for input in [
        "Content-Length: nope\r\n\r\n{}",
        "Content-Length: -1\r\n\r\n",
        "X-Header: 2\r\n\r\n{}",
        "Content-Length: 2\r\nContent-Length: 3\r\n\r\n{}",
        "Content-Length: 20\r\n\r\n{}",
        "Content-Length: 2\r\n",
    ] {
        let mut server = Server::new();
        server.raw(input.as_bytes());
        server.finish(1, true);
    }
}

#[test]
fn full_document_changes_publish_and_clear_diagnostics() {
    let mut server = Server::new();
    server.initialize();
    let uri = "file:///lsp-diagnostics-%C3%B8.runa";
    server.send(
        json!({"jsonrpc":"2.0", "method":"textDocument/didOpen", "params":{
        "textDocument":{"uri":uri, "languageId":"futuruna", "version":1, "text":"= x = missing"}}}),
    );
    let first = server.receive();
    assert_eq!(first["method"], "textDocument/publishDiagnostics");
    assert_eq!(first["params"]["uri"], uri);
    assert!(!first["params"]["diagnostics"]
        .as_array()
        .unwrap()
        .is_empty());
    server.send(
        json!({"jsonrpc":"2.0", "method":"textDocument/didChange", "params":{
        "textDocument":{"uri":uri, "version":2}, "contentChanges":[{"text":"= navn = \"ø😀\""}]}}),
    );
    let changed = server.receive();
    assert_eq!(changed["method"], "textDocument/publishDiagnostics");
    assert_eq!(changed["params"]["diagnostics"], json!([]));
    server.send(json!({"jsonrpc":"2.0", "method":"textDocument/didClose", "params":{"textDocument":{"uri":uri}}}));
    assert_eq!(server.receive()["params"]["diagnostics"], json!([]));
    server.shutdown();
    server.exit();
    server.finish(0, false);
}

#[test]
fn record_completion_and_hover_use_the_open_buffer_without_modifying_it() {
    let mut server = Server::new();
    server.initialize();
    let uri = "file:///lsp-record-fields.runa";
    let source = "# Person(income: Int)\n= p = Person(42)\np.income";
    server.send(
        json!({"jsonrpc":"2.0", "method":"textDocument/didOpen", "params":{
        "textDocument":{"uri":uri, "languageId":"futuruna", "version":1, "text":source}}}),
    );
    assert_eq!(server.receive()["params"]["diagnostics"], json!([]));
    server.send(
        json!({"jsonrpc":"2.0", "id":2, "method":"textDocument/hover", "params":{
        "textDocument":{"uri":uri}, "position":{"line":2,"character":4}}}),
    );
    assert_eq!(
        server.receive()["result"]["contents"]["value"],
        "```runa\nPerson.income: Int\n```"
    );
    server.send(json!({"jsonrpc":"2.0", "method":"textDocument/didChange", "params":{
        "textDocument":{"uri":uri,"version":2}, "contentChanges":[{"text":source.replace("p.income", "p.")}]}}));
    let unfinished = server.receive();
    assert!(!unfinished["params"]["diagnostics"]
        .as_array()
        .unwrap()
        .is_empty());
    server.send(
        json!({"jsonrpc":"2.0", "id":3, "method":"textDocument/completion", "params":{
        "textDocument":{"uri":uri}, "position":{"line":2,"character":2}}}),
    );
    assert_eq!(
        server.receive()["result"],
        json!([
            {"label":"income", "kind":5, "detail":"Person.income: Int"}
        ])
    );
    server.send(json!({"jsonrpc":"2.0", "method":"textDocument/didSave", "params":{"textDocument":{"uri":uri}}}));
    assert_eq!(
        server.receive(),
        unfinished,
        "completion must not edit the open buffer"
    );
    server.shutdown();
    server.exit();
    server.finish(0, false);
}
