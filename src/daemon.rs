//! Unix-socket daemon that the zsh plugin talks to.
//!
//! Protocol: the shell sends one JSON object per line (see [`ClientMsg`]); the daemon answers
//! with `<seq>\t<suggested command line>\n`. Each connection is one shell session. The shell
//! numbers its messages and ignores replies to anything but its latest one. A `status` message
//! is answered with one line of [`DaemonStats`] JSON instead.

use std::fs::DirBuilder;
use std::os::unix::fs::DirBuilderExt as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use anyhow::{Context as _, bail};
use serde::Deserialize;
use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::context::{Context, FinishedCommand, Sources};
use crate::model::LlamaClient;
use crate::stats::{DaemonStats, Recorder};

const SESSION_COMMAND_LIMIT: usize = 20;

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum ClientMsg {
    /// A command finished (or an empty line was entered) and a new prompt is about to show.
    Done {
        seq: u64,
        cmd: String,
        status: i32,
        cwd: String,
        #[serde(default)]
        pane: String,
        #[serde(default)]
        histfile: String,
    },
    /// The command line changed; `buf` is everything left of the cursor.
    Req { seq: u64, buf: String },
    /// Asks for a [`DaemonStats`] snapshot; sent by `wisp status`, not by shells.
    Status,
}

pub struct Config {
    pub socket: PathBuf,
    pub model: LlamaClient,
    /// How long typing must pause before the model is asked.
    pub debounce: Duration,
    /// No suggestions are made while this file exists.
    pub paused_flag: PathBuf,
}

struct Shared {
    config: Config,
    recorder: Mutex<Recorder>,
}

impl Shared {
    fn recorder(&self) -> std::sync::MutexGuard<'_, Recorder> {
        self.recorder.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Serves shell sessions until the listener fails.
///
/// # Errors
///
/// Returns an error if another daemon owns the socket or the socket cannot be bound.
pub async fn run(config: Config) -> anyhow::Result<()> {
    let listener = bind(&config.socket).await?;
    tracing::info!("listening on {}", config.socket.display());
    let shared = Arc::new(Shared { config, recorder: Mutex::default() });
    loop {
        let (stream, _) = listener.accept().await.context("accepting shell connection")?;
        let shared = Arc::clone(&shared);
        tokio::spawn(async move {
            if let Err(err) = serve(stream, shared).await {
                tracing::warn!("shell session ended with error: {err:#}");
            }
        });
    }
}

async fn bind(socket: &Path) -> anyhow::Result<UnixListener> {
    if let Some(dir) = socket.parent() {
        // Owner-only, so other users cannot talk to this daemon.
        DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)
            .with_context(|| format!("creating socket directory {}", dir.display()))?;
    }
    if UnixStream::connect(socket).await.is_ok() {
        bail!("another wisp daemon is already listening on {}", socket.display());
    }
    match tokio::fs::remove_file(socket).await {
        Ok(()) => tracing::info!("removed stale socket {}", socket.display()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => {
            return Err(err).with_context(|| format!("removing stale socket {}", socket.display()));
        }
    }
    UnixListener::bind(socket).with_context(|| format!("binding {}", socket.display()))
}

async fn serve(stream: UnixStream, shared: Arc<Shared>) -> anyhow::Result<()> {
    let (read, mut write) = stream.into_split();
    let (replies, mut outbox) = mpsc::unbounded_channel::<String>();
    let writer = tokio::spawn(async move {
        while let Some(line) = outbox.recv().await {
            if let Err(err) = write.write_all(line.as_bytes()).await {
                tracing::debug!("shell went away: {err}");
                break;
            }
        }
    });

    let mut session = Session::new(shared, replies);
    let mut lines = BufReader::new(read).lines();
    let result = async {
        while let Some(line) = lines.next_line().await.context("reading from shell")? {
            match serde_json::from_str::<ClientMsg>(&line) {
                Ok(msg) => session.handle(msg).await,
                Err(err) => tracing::warn!("ignoring malformed message {line:?}: {err}"),
            }
        }
        Ok(())
    }
    .await;
    session.close();
    writer.abort();
    result
}

struct Session {
    shared: Arc<Shared>,
    /// Set once the connection sends shell messages, so status queries are not counted.
    is_shell: bool,
    replies: mpsc::UnboundedSender<String>,
    finished: Vec<FinishedCommand>,
    context: Arc<Context>,
    pending: Option<JoinHandle<()>>,
    last_suggestion: Arc<Mutex<Option<String>>>,
}

impl Session {
    fn new(shared: Arc<Shared>, replies: mpsc::UnboundedSender<String>) -> Self {
        Self {
            shared,
            is_shell: false,
            replies,
            finished: Vec::new(),
            context: Arc::default(),
            pending: None,
            last_suggestion: Arc::default(),
        }
    }

    async fn handle(&mut self, msg: ClientMsg) {
        self.cancel();
        match msg {
            ClientMsg::Status => self.send_status(),
            ClientMsg::Done { seq, cmd, status, cwd, pane, histfile } => {
                self.set_last_suggestion(None);
                if !cmd.trim().is_empty() {
                    self.finished.push(FinishedCommand { command: cmd, status });
                    let excess = self.finished.len().saturating_sub(SESSION_COMMAND_LIMIT);
                    self.finished.drain(..excess);
                }
                if !self.accepts_suggestions().await {
                    return;
                }
                let histfile = Some(PathBuf::from(histfile)).filter(|p| !p.as_os_str().is_empty());
                let sources = Sources { cwd: cwd.into(), pane, histfile };
                self.context = Arc::new(Context::gather(&sources, &self.finished).await);
                // Predict the next command while the prompt is still empty.
                self.suggest(seq, String::new(), Duration::ZERO);
            }
            ClientMsg::Req { seq, buf } => {
                if buf.contains('\n') || !self.accepts_suggestions().await {
                    return;
                }
                if let Some(cached) = self.cached_extension(&buf) {
                    send_reply(&self.replies, seq, &cached);
                    return;
                }
                self.suggest(seq, buf, self.shared.config.debounce);
            }
        }
    }

    /// Called for every shell message. While paused it also forgets the last suggestion, so a
    /// stale one is not replayed after resuming.
    async fn accepts_suggestions(&mut self) -> bool {
        if !self.is_shell {
            self.is_shell = true;
            self.shared.recorder().shell_connected();
        }
        let paused = tokio::fs::try_exists(&self.shared.config.paused_flag).await.unwrap_or(false);
        if paused {
            self.set_last_suggestion(None);
        }
        !paused
    }

    fn send_status(&self) {
        let stats: DaemonStats = self.shared.recorder().snapshot();
        match serde_json::to_string(&stats) {
            Ok(json) => {
                let _ = self.replies.send(format!("{json}\n"));
            }
            Err(err) => tracing::error!("encoding status: {err}"),
        }
    }

    fn cancel(&mut self) {
        if let Some(task) = self.pending.take() {
            task.abort();
        }
    }

    fn close(&mut self) {
        self.cancel();
        if self.is_shell {
            self.shared.recorder().shell_disconnected();
        }
    }

    /// The previous suggestion still applies while the user types along with it.
    fn cached_extension(&self, buffer: &str) -> Option<String> {
        let last = self.last_suggestion.lock().unwrap_or_else(PoisonError::into_inner);
        last.as_ref().filter(|s| s.len() > buffer.len() && s.starts_with(buffer)).cloned()
    }

    fn set_last_suggestion(&self, suggestion: Option<String>) {
        *self.last_suggestion.lock().unwrap_or_else(PoisonError::into_inner) = suggestion;
    }

    fn suggest(&mut self, seq: u64, buffer: String, delay: Duration) {
        let model = self.shared.config.model.clone();
        let shared = Arc::clone(&self.shared);
        let context = Arc::clone(&self.context);
        let replies = self.replies.clone();
        let last_suggestion = Arc::clone(&self.last_suggestion);
        self.pending = Some(tokio::spawn(async move {
            tokio::time::sleep(delay).await;
            let started = Instant::now();
            let suggestion = match model.complete_command(&context.transcript(), &buffer).await {
                Ok(suggestion) => suggestion,
                Err(err) => {
                    tracing::warn!("completing {buffer:?}: {err:#}");
                    shared.recorder().record_error(format!("{err:#}"));
                    return;
                }
            };
            let elapsed = started.elapsed();
            tracing::debug!("{buffer:?} -> {suggestion:?} ({elapsed:?})");
            shared.recorder().record(&buffer, suggestion.as_deref(), elapsed);
            let Some(suggestion) = suggestion else {
                return;
            };
            *last_suggestion.lock().unwrap_or_else(PoisonError::into_inner) =
                Some(suggestion.clone());
            send_reply(&replies, seq, &suggestion);
        }));
    }
}

fn send_reply(replies: &mpsc::UnboundedSender<String>, seq: u64, suggestion: &str) {
    // Fails only when the connection is closing, in which case nobody wants the reply.
    let _ = replies.send(format!("{seq}\t{suggestion}\n"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_done_message_with_optional_fields_missing() {
        let msg: ClientMsg =
            serde_json::from_str(r#"{"t":"done","seq":3,"cmd":"ls","status":1,"cwd":"/tmp"}"#)
                .unwrap();
        let expected = ClientMsg::Done {
            seq: 3,
            cmd: "ls".into(),
            status: 1,
            cwd: "/tmp".into(),
            pane: String::new(),
            histfile: String::new(),
        };
        assert_eq!(msg, expected);
    }

    #[test]
    fn parses_request_with_escaped_characters() {
        let msg: ClientMsg =
            serde_json::from_str(r#"{"t":"req","seq":9,"buf":"echo \"a\\b\""}"#).unwrap();
        assert_eq!(msg, ClientMsg::Req { seq: 9, buf: r#"echo "a\b""#.into() });
    }

    #[test]
    fn rejects_unknown_message_types() {
        assert!(serde_json::from_str::<ClientMsg>(r#"{"t":"bogus","seq":1}"#).is_err());
    }
}
