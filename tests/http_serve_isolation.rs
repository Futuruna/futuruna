//! One bad request must never take an `http_serve` server down: a handler
//! panic answers 500 for that request only, an oversized body answers 413 and
//! a body that is not UTF-8 answers 400, without calling the handler.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

struct Server {
    child: Child,
    directory: PathBuf,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

fn send(port: u16, head: &str, body: &[u8]) -> String {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .unwrap();
    stream.write_all(head.as_bytes()).unwrap();
    stream.write_all(body).unwrap();
    let mut response = Vec::new();
    stream.read_to_end(&mut response).unwrap();
    String::from_utf8_lossy(&response).into_owned()
}

fn post(port: u16, path: &str, body: &[u8]) -> String {
    send(
        port,
        &format!(
            "POST {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Length: {}\r\n\r\n",
            body.len()
        ),
        body,
    )
}

fn assert_json_error(response: &str, status: &str, message: &str) {
    assert!(response.starts_with(status), "{response}");
    assert!(
        response
            .to_ascii_lowercase()
            .contains("content-type: application/json"),
        "{response}"
    );
    assert!(
        response.ends_with(&format!("{{\"error\":\"{message}\"}}")),
        "{response}"
    );
}

#[test]
fn a_panicking_or_malformed_request_fails_alone_and_the_server_keeps_serving() {
    let directory = std::env::temp_dir().join(format!(
        "futuruna-http-serve-isolation-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    std::fs::write(
        directory.join("overflow_server.runa"),
        format!(
            r#"> amount_for(path: String) -> Int {{
    if path == "/overflow" {{ 9223372036854775807 + 1 }} else {{ 7 }}
}}
@ http_serve({port}, |path, method, body| {{
        http_respond(200, "text/plain", show(amount_for(path)) + ":" + body)
    }})
"#
        ),
    )
    .unwrap();

    let build = Command::new(env!("CARGO_BIN_EXE_runa"))
        .current_dir(&directory)
        .args(["build", "overflow_server.runa"])
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );

    let stderr_path = directory.join("server.stderr");
    let mut server = Server {
        child: Command::new(directory.join("overflow_server"))
            .stdout(Stdio::null())
            .stderr(std::fs::File::create(&stderr_path).unwrap())
            .spawn()
            .unwrap(),
        directory,
    };
    let deadline = Instant::now() + Duration::from_secs(30);
    while TcpStream::connect(("127.0.0.1", port)).is_err() {
        assert!(Instant::now() < deadline, "server did not start listening");
        std::thread::sleep(Duration::from_millis(50));
    }

    assert_json_error(
        &post(port, "/overflow", b"x"),
        "HTTP/1.1 500",
        "internal error",
    );

    let requests = (0..8)
        .map(|index| std::thread::spawn(move || post(port, &format!("/p{index}"), b"ok")))
        .collect::<Vec<_>>();
    for response in requests {
        let response = response.join().unwrap();
        assert!(response.starts_with("HTTP/1.1 200"), "{response}");
        assert!(response.ends_with("7:ok"), "{response}");
    }

    // The declared length alone exceeds the 1 MiB limit: rejected unread.
    assert_json_error(
        &send(
            port,
            "POST /big HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Length: 1048577\r\n\r\n",
            b"",
        ),
        "HTTP/1.1 413",
        "request body too large",
    );
    assert_json_error(
        &post(port, "/bytes", &[0xff, 0xfe, b'a']),
        "HTTP/1.1 400",
        "request body is not valid UTF-8",
    );

    assert!(post(port, "/after", b"alive").ends_with("7:alive"));
    assert!(
        server.child.try_wait().unwrap().is_none(),
        "server process exited"
    );
    let stderr = std::fs::read_to_string(&stderr_path).unwrap();
    assert!(
        stderr.contains("error: request POST /overflow: integer addition overflow"),
        "{stderr}"
    );
}
