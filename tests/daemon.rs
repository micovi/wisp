//! Drives the daemon over its unix socket with a fake llama-server behind it.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test helpers outside #[test] fns should fail loudly"
)]

use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use tokio::io::{AsyncBufReadExt as _, AsyncReadExt as _, AsyncWriteExt as _, BufReader, Lines};
use tokio::net::unix::OwnedReadHalf;
use tokio::net::{TcpListener, UnixStream};
use wisp::daemon::{self, Config};
use wisp::model::LlamaClient;
use wisp::stats::DaemonStats;

const DEBOUNCE: Duration = Duration::from_millis(50);
const REPLY_TIMEOUT: Duration = Duration::from_secs(2);

/// A fake llama-server that answers every `/completion` with the grammar's forced prefix
/// followed by `content`, after `delay`.
struct FakeModel {
    url: String,
    calls: Arc<AtomicUsize>,
}

async fn fake_model(content: &'static str, delay: Duration) -> FakeModel {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&calls);
    tokio::spawn(async move {
        loop {
            let (mut stream, _) = listener.accept().await.unwrap();
            let counter = Arc::clone(&counter);
            tokio::spawn(async move {
                let request = read_http_request(&mut stream).await;
                counter.fetch_add(1, Ordering::SeqCst);
                tokio::time::sleep(delay).await;
                let output = format!("{}{content}", forced_prefix(&request));
                let body = serde_json::json!({ "content": output }).to_string();
                let response = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes()).await;
            });
        }
    });
    FakeModel { url, calls }
}

/// Extracts the literal from a `root ::= "<literal>" ...` grammar.
fn forced_prefix(request_body: &str) -> String {
    let request: serde_json::Value = serde_json::from_str(request_body).unwrap();
    let grammar = request["grammar"].as_str().unwrap();
    let literal = grammar.strip_prefix("root ::= \"").unwrap();
    let literal = &literal[..literal.find("\" [^").unwrap()];
    literal.replace("\\\"", "\"").replace("\\\\", "\\")
}

/// Reads one HTTP request and returns its body.
async fn read_http_request(stream: &mut tokio::net::TcpStream) -> String {
    let mut request = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        let n = stream.read(&mut chunk).await.unwrap();
        request.extend_from_slice(&chunk[..n]);
        let text = String::from_utf8_lossy(&request);
        if let Some((head, body)) = text.split_once("\r\n\r\n") {
            let length = head
                .lines()
                .find_map(|l| {
                    l.to_ascii_lowercase().strip_prefix("content-length: ").map(str::to_owned)
                })
                .and_then(|v| v.trim().parse::<usize>().ok())
                .unwrap_or(0);
            if body.len() >= length {
                return body.to_owned();
            }
        }
        assert!(n > 0, "client closed the connection mid-request");
    }
}

struct Shell {
    lines: Lines<BufReader<OwnedReadHalf>>,
    write: tokio::net::unix::OwnedWriteHalf,
    dir: tempfile::TempDir,
}

impl Shell {
    async fn send(&mut self, json: serde_json::Value) {
        self.write.write_all(format!("{json}\n").as_bytes()).await.unwrap();
    }

    async fn reply(&mut self) -> Option<String> {
        tokio::time::timeout(REPLY_TIMEOUT, self.lines.next_line()).await.ok()?.unwrap()
    }

    async fn no_reply_within(&mut self, wait: Duration) -> bool {
        tokio::time::timeout(wait, self.lines.next_line()).await.is_err()
    }
}

async fn start_daemon(model_url: &str) -> Shell {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("wisp.sock");
    let config = Config {
        socket: socket.clone(),
        model: LlamaClient::new(model_url).unwrap(),
        debounce: DEBOUNCE,
        paused_flag: wisp::paths::paused_flag(&socket),
    };
    tokio::spawn(daemon::run(config));
    let stream = connect(&socket).await;
    let (read, write) = stream.into_split();
    Shell { lines: BufReader::new(read).lines(), write, dir }
}

async fn connect(socket: &Path) -> UnixStream {
    for _ in 0..100 {
        if let Ok(stream) = UnixStream::connect(socket).await {
            return stream;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("daemon never listened on {}", socket.display());
}

fn done(seq: u64, cmd: &str, cwd: &Path) -> serde_json::Value {
    serde_json::json!({ "t": "done", "seq": seq, "cmd": cmd, "status": 0, "cwd": cwd })
}

fn req(seq: u64, buf: &str) -> serde_json::Value {
    serde_json::json!({ "t": "req", "seq": seq, "buf": buf })
}

#[tokio::test]
async fn predicts_next_command_after_a_command_finishes() {
    let model = fake_model("cargo test", Duration::ZERO).await;
    let mut shell = start_daemon(&model.url).await;
    shell.send(done(1, "cargo build", &PathBuf::from("/tmp"))).await;
    assert_eq!(shell.reply().await.as_deref(), Some("1\tcargo test"));
}

#[tokio::test]
async fn completes_the_typed_buffer() {
    let model = fake_model("ckout main", Duration::ZERO).await;
    let mut shell = start_daemon(&model.url).await;
    shell.send(req(4, "git che")).await;
    assert_eq!(shell.reply().await.as_deref(), Some("4\tgit checkout main"));
}

#[tokio::test]
async fn typing_along_with_a_suggestion_reuses_it_without_calling_the_model() {
    let model = fake_model("ckout main", Duration::ZERO).await;
    let mut shell = start_daemon(&model.url).await;
    shell.send(req(1, "git che")).await;
    assert_eq!(shell.reply().await.as_deref(), Some("1\tgit checkout main"));
    shell.send(req(2, "git chec")).await;
    assert_eq!(shell.reply().await.as_deref(), Some("2\tgit checkout main"));
    assert_eq!(model.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn fast_typing_only_asks_the_model_once() {
    let model = fake_model("status", Duration::ZERO).await;
    let mut shell = start_daemon(&model.url).await;
    for (seq, buf) in [(1, "g"), (2, "gi"), (3, "git"), (4, "git ")] {
        shell.send(req(seq, buf)).await;
    }
    assert_eq!(shell.reply().await.as_deref(), Some("4\tgit status"));
    assert!(shell.no_reply_within(DEBOUNCE * 4).await);
    assert_eq!(model.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn a_slow_answer_is_dropped_when_the_user_keeps_typing() {
    let model = fake_model("x", Duration::from_millis(300)).await;
    let mut shell = start_daemon(&model.url).await;
    shell.send(req(1, "ls")).await;
    tokio::time::sleep(DEBOUNCE * 2).await;
    shell.send(req(2, "ls -")).await;
    assert_eq!(shell.reply().await.as_deref(), Some("2\tls -x"));
    assert!(shell.no_reply_within(Duration::from_millis(400)).await);
}

#[tokio::test]
async fn empty_model_output_gives_no_reply() {
    let model = fake_model("\n", Duration::ZERO).await;
    let mut shell = start_daemon(&model.url).await;
    shell.send(req(1, "ls")).await;
    assert!(shell.no_reply_within(DEBOUNCE * 6).await);
}

#[tokio::test]
async fn unreachable_model_and_bad_messages_keep_the_session_alive() {
    let mut shell = start_daemon("http://127.0.0.1:9").await;
    shell.write.write_all(b"not json\n").await.unwrap();
    shell.send(req(1, "ls")).await;
    assert!(shell.no_reply_within(DEBOUNCE * 6).await);

    let socket = shell.dir.path().join("wisp.sock");
    assert!(UnixStream::connect(&socket).await.is_ok(), "daemon should still accept shells");
}

#[tokio::test]
async fn a_second_daemon_refuses_to_steal_the_socket() {
    let model = fake_model("", Duration::ZERO).await;
    let shell = start_daemon(&model.url).await;
    let socket = shell.dir.path().join("wisp.sock");
    let second = Config {
        socket,
        model: LlamaClient::new(&model.url).unwrap(),
        debounce: DEBOUNCE,
        paused_flag: PathBuf::from("/nonexistent/wisp-paused"),
    };
    let err = daemon::run(second).await.unwrap_err();
    assert!(err.to_string().contains("already listening"), "{err:#}");
}

#[tokio::test]
async fn a_stale_socket_file_is_replaced() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("wisp.sock");
    drop(std::os::unix::net::UnixListener::bind(&socket).unwrap());
    let model = fake_model("", Duration::ZERO).await;
    let config = Config {
        socket: socket.clone(),
        model: LlamaClient::new(&model.url).unwrap(),
        debounce: DEBOUNCE,
        paused_flag: PathBuf::from("/nonexistent/wisp-paused"),
    };
    tokio::spawn(daemon::run(config));
    connect(&socket).await;
}

#[tokio::test]
async fn creates_a_missing_socket_directory() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("cache/wisp/wisp.sock");
    let model = fake_model("", Duration::ZERO).await;
    let config = Config {
        socket: socket.clone(),
        model: LlamaClient::new(&model.url).unwrap(),
        debounce: DEBOUNCE,
        paused_flag: PathBuf::from("/nonexistent/wisp-paused"),
    };
    tokio::spawn(daemon::run(config));
    connect(&socket).await;
    let mode = std::fs::metadata(socket.parent().unwrap()).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o700);
}

async fn status_of(socket: &Path) -> DaemonStats {
    let stream = connect(socket).await;
    let (read, mut write) = stream.into_split();
    write.write_all(b"{\"t\":\"status\"}\n").await.unwrap();
    let line = BufReader::new(read).lines().next_line().await.unwrap().unwrap();
    serde_json::from_str(&line).unwrap()
}

#[tokio::test]
async fn status_reports_shells_and_recent_suggestions() {
    let model = fake_model("ckout main", Duration::ZERO).await;
    let mut shell = start_daemon(&model.url).await;
    let socket = shell.dir.path().join("wisp.sock");
    assert_eq!(status_of(&socket).await.shells, 0, "a status query is not a shell");

    shell.send(req(1, "git che")).await;
    assert!(shell.reply().await.is_some());
    let stats = status_of(&socket).await;
    assert_eq!(stats.shells, 1);
    assert_eq!(stats.recent.len(), 1);
    assert_eq!(stats.recent[0].typed, "git che");
    assert_eq!(stats.recent[0].suggested.as_deref(), Some("git checkout main"));
    assert_eq!(stats.last_error, None);

    drop(shell.write);
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(status_of(&socket).await.shells, 0);
}

#[tokio::test]
async fn status_reports_model_errors() {
    let mut shell = start_daemon("http://127.0.0.1:9").await;
    let socket = shell.dir.path().join("wisp.sock");
    shell.send(req(1, "ls")).await;
    tokio::time::sleep(DEBOUNCE * 4).await;
    let error = status_of(&socket).await.last_error.unwrap_or_default();
    assert!(error.contains("127.0.0.1:9"), "{error}");
}

#[tokio::test]
async fn pausing_stops_suggestions_until_resumed() {
    let model = fake_model("ckout main", Duration::ZERO).await;
    let mut shell = start_daemon(&model.url).await;
    let socket = shell.dir.path().join("wisp.sock");

    wisp::paths::set_paused(&socket, true).unwrap();
    shell.send(req(1, "git che")).await;
    assert!(shell.no_reply_within(DEBOUNCE * 4).await);
    assert_eq!(model.calls.load(Ordering::SeqCst), 0);

    wisp::paths::set_paused(&socket, false).unwrap();
    shell.send(req(2, "git che")).await;
    assert_eq!(shell.reply().await.as_deref(), Some("2\tgit checkout main"));
}
