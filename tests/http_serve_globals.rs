//! An `http_serve` handler runs on the server's tasks, after the frame that
//! owns the top-level bindings has stopped lending them. The server must still
//! compile when the handler reads those bindings through a function, and every
//! concurrent request must see them.

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

fn request(port: u16, path: &str) -> String {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .unwrap();
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    response
}

#[test]
fn handler_reads_top_level_bindings_through_a_function_under_concurrent_requests() {
    let directory = std::env::temp_dir().join(format!(
        "futuruna-http-serve-globals-{}-{}",
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
        directory.join("notes_server.runa"),
        format!(
            r#"# Info(id: String, text: String)
= notes = [Info(id = "a", text = "first"), Info(id = "b", text = "second")]
= separator = ","
> body_for(path: String) -> String {{ join(map(notes, |n| n.id + path), separator) }}
@ http_serve({port}, |path, method, body| {{
        http_respond(200, "text/plain", body_for(path))
    }})
"#
        ),
    )
    .unwrap();

    let build = Command::new(env!("CARGO_BIN_EXE_runa"))
        .current_dir(&directory)
        .args(["build", "notes_server.runa"])
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );

    let server = Server {
        child: Command::new(directory.join("notes_server"))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
        directory,
    };
    let deadline = Instant::now() + Duration::from_secs(30);
    while TcpStream::connect(("127.0.0.1", port)).is_err() {
        assert!(Instant::now() < deadline, "server did not start listening");
        std::thread::sleep(Duration::from_millis(50));
    }

    let requests = (0..16)
        .map(|index| std::thread::spawn(move || request(port, &format!("/p{index}"))))
        .collect::<Vec<_>>();
    for (index, response) in requests.into_iter().enumerate() {
        let response = response.join().unwrap();
        assert!(response.starts_with("HTTP/1.1 200"), "{response}");
        assert!(
            response.ends_with(&format!("a/p{index},b/p{index}")),
            "{response}"
        );
    }
    drop(server);
}

#[test]
fn handler_bound_to_a_top_level_closure_owns_the_bindings() {
    let directory = std::env::temp_dir().join(format!(
        "futuruna-http-serve-closure-binding-{}-{}",
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
        directory.join("bound_handler.runa"),
        format!(
            r#"= notes = ["a", "b"]
> body_for(path: String) -> String {{ join(map(notes, |n| n + path), ",") }}
= handler = |path: String, method: String, body: String| http_respond(200, "text/plain", body_for(path))
@ http_serve({port}, handler)
"#
        ),
    )
    .unwrap();

    let build = Command::new(env!("CARGO_BIN_EXE_runa"))
        .current_dir(&directory)
        .args(["build", "bound_handler.runa"])
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );

    let server = Server {
        child: Command::new(directory.join("bound_handler"))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
        directory,
    };
    let deadline = Instant::now() + Duration::from_secs(30);
    while TcpStream::connect(("127.0.0.1", port)).is_err() {
        assert!(Instant::now() < deadline, "server did not start listening");
        std::thread::sleep(Duration::from_millis(50));
    }
    let response = request(port, "/x");
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
    assert!(response.ends_with("a/x,b/x"), "{response}");
    drop(server);
}
