//! Live cross-language round trip: this Rust CLIENT against the real
//! Go SERVER (`github.com/zap-proto/go@v1.8.2`), over a real socket.
//!
//! `tests/crosswire.rs` pins Go's bytes; this pins Go's BEHAVIOUR — the
//! framing, the direction tag, the PromiseID correlation and the
//! connection lifecycle, none of which a byte fixture can prove. The
//! peer is `../interop/main.go`, built and launched by this test.
//!
//! Skipped (loudly) when the Go toolchain is absent, so the suite still
//! runs on a Rust-only machine.

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

use zapwire::rpc::{STATUS_NOT_FOUND, STATUS_OK};
use zapwire::transport::Conn;

/// The repo root — `rust/`'s parent, where `interop/` and `go.mod` live.
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("rust/ has a parent")
        .to_path_buf()
}

fn have_go() -> bool {
    Command::new("go")
        .arg("version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Build the Go peer, then run it on `addr`. Returns the child and
/// blocks until it prints `ready`, so the test never races the bind.
fn start_go_peer(addr: &str) -> Child {
    let root = repo_root();
    let bin = std::env::temp_dir().join(format!("zap-interop-peer-{}", std::process::id()));

    let built = Command::new("go")
        .args(["build", "-o"])
        .arg(&bin)
        .arg("./interop")
        .current_dir(&root)
        .output()
        .expect("go build");
    assert!(
        built.status.success(),
        "go build failed: {}",
        String::from_utf8_lossy(&built.stderr)
    );

    let mut child = Command::new(&bin)
        .arg(addr)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("spawn go peer");

    let out = child.stdout.take().expect("peer stdout");
    let mut line = String::new();
    BufReader::new(out)
        .read_line(&mut line)
        .expect("peer readiness line");
    assert!(
        line.starts_with("ready"),
        "unexpected peer output: {line:?}"
    );
    child
}

/// A Rust client calls a Go server over a Unix socket — the LOCAL
/// deployment ZAP is actually used in — and gets the Go handler's
/// answer back, correlated to its own PromiseID.
#[tokio::test]
async fn rust_client_calls_a_go_server_over_a_unix_socket() {
    if !have_go() {
        eprintln!("SKIP interop: no Go toolchain on PATH");
        return;
    }
    let dir = std::env::temp_dir().join(format!("zap-interop-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let sock = dir.join("go.sock");
    let addr = sock.to_str().unwrap().to_string();

    let mut peer = start_go_peer(&addr);
    let conn = Conn::connect(&addr).await.expect("dial the Go peer");

    // Method 1 is the Go peer's "upper".
    let r = conn.invoke(1, &[], b"hello from rust").await.unwrap();
    assert_eq!(r.status, STATUS_OK);
    assert_eq!(r.body, b"HELLO FROM RUST");
    assert_eq!(r.promise_id, 1, "the answer echoes our PromiseID");

    // A second call on the same connection gets the NEXT PromiseID and
    // its own answer — correlation, not ordering.
    let r2 = conn.invoke(1, &[], b"again").await.unwrap();
    assert_eq!(r2.body, b"AGAIN");
    assert_eq!(r2.promise_id, 2);

    // An unknown ordinal comes back as the Go peer's status, not as a
    // hang or a decode error.
    let miss = conn.invoke(99, &[], b"nope").await.unwrap();
    assert_eq!(miss.status, STATUS_NOT_FOUND);

    let _ = peer.kill();
    let _ = peer.wait();
    let _ = std::fs::remove_dir_all(&dir);
}

/// Concurrent in-flight calls against the Go server: every answer must
/// find its own caller. This is the property the PromiseID exists for,
/// and it can only be proved against a peer that really interleaves.
#[tokio::test]
async fn concurrent_calls_to_a_go_server_stay_correlated() {
    if !have_go() {
        eprintln!("SKIP interop: no Go toolchain on PATH");
        return;
    }
    let dir = std::env::temp_dir().join(format!("zap-interop-conc-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let sock = dir.join("go.sock");
    let addr = sock.to_str().unwrap().to_string();
    let mut peer = start_go_peer(&addr);

    let conn = Conn::connect(&addr).await.expect("dial the Go peer");
    let mut tasks = Vec::new();
    for i in 0..64u32 {
        let conn = conn.clone();
        tasks.push(tokio::spawn(async move {
            let body = format!("msg-{i}");
            let r = conn.invoke(1, &[], body.as_bytes()).await.unwrap();
            assert_eq!(r.body, body.to_uppercase().as_bytes());
        }));
    }
    for t in tasks {
        t.await.unwrap();
    }

    let _ = peer.kill();
    let _ = peer.wait();
    let _ = std::fs::remove_dir_all(&dir);
}
