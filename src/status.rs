//! `wisp status`: one report on the daemon, llama-server and the pause flag, shared by the CLI
//! and the menu bar.

use std::fmt::Write as _;
use std::path::Path;
use std::sync::OnceLock;
use std::time::Duration;

use anyhow::Context as _;
use serde::Serialize;
use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader};
use tokio::net::UnixStream;
use tokio::process::Command;

use crate::model::{LlamaClient, LlamaStatus};
use crate::paths;
use crate::stats::DaemonStats;

const DAEMON_TIMEOUT: Duration = Duration::from_millis(500);
const RECENT_SHOWN: usize = 10;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Report {
    pub paused: bool,
    /// `None` when no daemon is running; the next shell prompt starts one.
    pub daemon: Option<DaemonStats>,
    pub llama: LlamaStatus,
    /// Resident memory of the launchd-managed llama-server, when it runs under launchd.
    pub llama_memory_mb: Option<u64>,
}

/// Gathers the report; never fails, since reporting what is down is the point.
pub async fn collect(socket: &Path, model: &LlamaClient) -> Report {
    let (paused, daemon, llama, llama_memory_mb) = tokio::join!(
        tokio::fs::try_exists(paths::paused_flag(socket)),
        query_daemon(socket),
        model.status(),
        llama_memory_mb(),
    );
    Report {
        paused: paused.unwrap_or(false),
        daemon: daemon.inspect_err(|err| tracing::debug!("daemon status: {err:#}")).ok(),
        llama,
        llama_memory_mb: llama_memory_mb
            .inspect_err(|err| tracing::debug!("llama-server memory: {err:#}"))
            .ok(),
    }
}

async fn query_daemon(socket: &Path) -> anyhow::Result<DaemonStats> {
    let exchange = async {
        let mut stream = UnixStream::connect(socket)
            .await
            .with_context(|| format!("connecting to {}", socket.display()))?;
        stream.write_all(b"{\"t\":\"status\"}\n").await?;
        let mut line = String::new();
        BufReader::new(stream).read_line(&mut line).await?;
        serde_json::from_str(&line).context("decoding daemon status")
    };
    tokio::time::timeout(DAEMON_TIMEOUT, exchange).await.context("daemon did not answer")?
}

/// `gui/<uid>/<label>`, the launchctl name of one of this user's agents.
///
/// # Errors
///
/// Returns an error if `id -u` cannot be run.
pub fn launchd_target(label: &str) -> anyhow::Result<String> {
    static UID: OnceLock<String> = OnceLock::new();
    let uid = if let Some(uid) = UID.get() {
        uid
    } else {
        let output =
            std::process::Command::new("id").arg("-u").output().context("running id -u")?;
        UID.get_or_init(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
    };
    Ok(format!("gui/{uid}/{label}"))
}

async fn llama_memory_mb() -> anyhow::Result<u64> {
    let target = launchd_target(paths::LLAMA_AGENT)?;
    let agent = Command::new("launchctl").args(["print", &target]).output().await?;
    let agent = String::from_utf8_lossy(&agent.stdout);
    let pid = agent
        .lines()
        .find_map(|line| line.trim().strip_prefix("pid = "))
        .context("llama-server agent is not running")?
        .to_owned();
    let ps = Command::new("ps").args(["-o", "rss=", "-p", &pid]).output().await?;
    let kilobytes: u64 =
        String::from_utf8_lossy(&ps.stdout).trim().parse().context("parsing ps")?;
    Ok(kilobytes / 1024)
}

/// Human-readable lines, used by both `wisp status` and the menu.
#[derive(Debug, PartialEq, Eq)]
pub struct Summary {
    pub title: String,
    pub llama: String,
    pub daemon: String,
    pub suggestions: String,
    /// Newest first: `162ms  git che → git checkout main`.
    pub recent: Vec<String>,
}

impl Report {
    #[must_use]
    pub fn summary(&self) -> Summary {
        Summary {
            title: self.title(),
            llama: self.llama_line(),
            daemon: self.daemon_line(),
            suggestions: if self.paused {
                "suggestions: paused".to_owned()
            } else {
                "suggestions: on".to_owned()
            },
            recent: self.recent_lines(),
        }
    }

    /// Short text for the menu bar: `wisp 3B · 180ms`, `wisp ⏸` or `wisp ⚠`.
    fn title(&self) -> String {
        if self.paused {
            return "wisp ⏸".to_owned();
        }
        if !self.llama.healthy {
            return "wisp ⚠".to_owned();
        }
        let size = self.llama.model.as_deref().and_then(model_size);
        let median = self.daemon.as_ref().and_then(DaemonStats::median_millis);
        match (size, median) {
            (Some(size), Some(ms)) => format!("wisp {size} · {ms}ms"),
            (Some(size), None) => format!("wisp {size}"),
            (None, Some(ms)) => format!("wisp · {ms}ms"),
            (None, None) => "wisp".to_owned(),
        }
    }

    fn llama_line(&self) -> String {
        if !self.llama.healthy {
            let reason = self.llama.error.as_deref().unwrap_or("not reachable");
            return format!("llama-server: down ({reason})");
        }
        let model = self.llama.model.as_deref().unwrap_or("unknown model");
        match self.llama_memory_mb {
            Some(mb) => format!("llama-server: {model} · {:.1} GB", gigabytes(mb)),
            None => format!("llama-server: {model}"),
        }
    }

    fn daemon_line(&self) -> String {
        let Some(daemon) = &self.daemon else {
            return "daemon: not running (starts with the next shell prompt)".to_owned();
        };
        let shells = match daemon.shells {
            1 => "1 shell".to_owned(),
            n => format!("{n} shells"),
        };
        let mut line = match daemon.median_millis() {
            Some(ms) => format!("daemon: {shells} · median {ms}ms"),
            None => format!("daemon: {shells}"),
        };
        if let Some(error) = &daemon.last_error {
            let _ = write!(line, " · last error: {error}");
        }
        line
    }

    fn recent_lines(&self) -> Vec<String> {
        let Some(daemon) = &self.daemon else {
            return Vec::new();
        };
        daemon
            .recent
            .iter()
            .take(RECENT_SHOWN)
            .map(|s| {
                let typed = if s.typed.is_empty() { "(empty prompt)" } else { &s.typed };
                let suggested = s.suggested.as_deref().unwrap_or("(nothing)");
                format!("{:>5}ms  {typed} → {suggested}", s.millis)
            })
            .collect()
    }
}

impl Summary {
    /// The plain-text report printed by `wisp status`.
    #[must_use]
    pub fn to_text(&self) -> String {
        let mut text = [self.llama.as_str(), &self.daemon, &self.suggestions].join("\n");
        if !self.recent.is_empty() {
            text.push_str("\nrecent:\n");
            text.push_str(&self.recent.join("\n"));
        }
        text.push('\n');
        text
    }
}

/// `Qwen2.5-Coder-3B-Q8_0` -> `3B`.
fn model_size(model: &str) -> Option<&str> {
    model.split('-').find(|part| {
        part.len() > 1
            && part.ends_with('B')
            && part[..part.len() - 1].bytes().all(|b| b.is_ascii_digit() || b == b'.')
    })
}

#[expect(clippy::cast_precision_loss, reason = "memory in MB fits f64 exactly for display")]
fn gigabytes(mb: u64) -> f64 {
    mb as f64 / 1024.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats::Suggestion;

    fn healthy_report() -> Report {
        Report {
            paused: false,
            daemon: Some(DaemonStats {
                shells: 2,
                uptime_secs: 60,
                recent: vec![
                    Suggestion {
                        typed: "git che".into(),
                        suggested: Some("git checkout main".into()),
                        millis: 162,
                    },
                    Suggestion { typed: String::new(), suggested: None, millis: 90 },
                    Suggestion {
                        typed: "ls".into(),
                        suggested: Some("ls -la".into()),
                        millis: 200,
                    },
                ],
                last_error: None,
            }),
            llama: LlamaStatus {
                healthy: true,
                model: Some("Qwen2.5-Coder-3B-Q8_0".into()),
                error: None,
            },
            llama_memory_mb: Some(3686),
        }
    }

    #[test]
    fn summarizes_a_healthy_setup() {
        let summary = healthy_report().summary();
        assert_eq!(summary.title, "wisp 3B · 162ms");
        assert_eq!(summary.llama, "llama-server: Qwen2.5-Coder-3B-Q8_0 · 3.6 GB");
        assert_eq!(summary.daemon, "daemon: 2 shells · median 162ms");
        assert_eq!(summary.suggestions, "suggestions: on");
        assert_eq!(
            summary.recent,
            [
                "  162ms  git che → git checkout main",
                "   90ms  (empty prompt) → (nothing)",
                "  200ms  ls → ls -la"
            ]
        );
    }

    #[test]
    fn pause_wins_over_everything_in_the_title() {
        let report = Report { paused: true, ..Report::default() };
        assert_eq!(report.summary().title, "wisp ⏸");
        assert_eq!(report.summary().suggestions, "suggestions: paused");
    }

    #[test]
    fn a_down_server_shows_a_warning_and_the_reason() {
        let report = Report {
            llama: LlamaStatus {
                healthy: false,
                model: None,
                error: Some("no llama-server at x".into()),
            },
            ..Report::default()
        };
        let summary = report.summary();
        assert_eq!(summary.title, "wisp ⚠");
        assert_eq!(summary.llama, "llama-server: down (no llama-server at x)");
        assert_eq!(summary.daemon, "daemon: not running (starts with the next shell prompt)");
        assert!(summary.recent.is_empty());
    }

    #[test]
    fn daemon_line_mentions_the_last_error() {
        let mut report = healthy_report();
        if let Some(daemon) = report.daemon.as_mut() {
            daemon.shells = 1;
            daemon.last_error = Some("timeout".into());
        }
        assert_eq!(report.summary().daemon, "daemon: 1 shell · median 162ms · last error: timeout");
    }

    #[test]
    fn finds_the_model_size() {
        assert_eq!(model_size("Qwen2.5-Coder-3B-Q8_0"), Some("3B"));
        assert_eq!(model_size("Qwen2.5-Coder-1.5B-Q8_0"), Some("1.5B"));
        assert_eq!(model_size("local-model"), None);
        assert_eq!(model_size("B-model"), None);
    }

    #[test]
    fn text_report_lists_recent_suggestions() {
        let text = healthy_report().summary().to_text();
        assert!(text.starts_with("llama-server: Qwen2.5-Coder-3B-Q8_0 · 3.6 GB\n"));
        assert!(text.contains("\nrecent:\n  162ms  git che → git checkout main\n"));
    }
}
