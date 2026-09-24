//! What the daemon has been doing, for `wisp status` and the menu bar.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

const RECENT_LIMIT: usize = 20;

/// One model call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Suggestion {
    pub typed: String,
    /// `None` when the model had nothing to add.
    pub suggested: Option<String>,
    pub millis: u64,
}

/// A snapshot of the daemon, as sent over the socket.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DaemonStats {
    pub shells: usize,
    pub uptime_secs: u64,
    /// Newest first.
    pub recent: Vec<Suggestion>,
    /// The most recent model error, cleared by the next successful call.
    pub last_error: Option<String>,
}

impl DaemonStats {
    #[must_use]
    pub fn median_millis(&self) -> Option<u64> {
        let mut millis: Vec<u64> = self.recent.iter().map(|s| s.millis).collect();
        millis.sort_unstable();
        millis.get(millis.len() / 2).copied()
    }
}

/// Live counters owned by the daemon.
#[derive(Debug)]
pub struct Recorder {
    started: Instant,
    shells: usize,
    recent: VecDeque<Suggestion>,
    last_error: Option<String>,
}

impl Default for Recorder {
    fn default() -> Self {
        Self { started: Instant::now(), shells: 0, recent: VecDeque::new(), last_error: None }
    }
}

impl Recorder {
    pub fn shell_connected(&mut self) {
        self.shells += 1;
    }

    pub fn shell_disconnected(&mut self) {
        self.shells = self.shells.saturating_sub(1);
    }

    pub fn record(&mut self, typed: &str, suggested: Option<&str>, elapsed: Duration) {
        self.last_error = None;
        self.recent.push_front(Suggestion {
            typed: typed.to_owned(),
            suggested: suggested.map(str::to_owned),
            millis: u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX),
        });
        self.recent.truncate(RECENT_LIMIT);
    }

    pub fn record_error(&mut self, error: String) {
        self.last_error = Some(error);
    }

    #[must_use]
    pub fn snapshot(&self) -> DaemonStats {
        DaemonStats {
            shells: self.shells,
            uptime_secs: self.started.elapsed().as_secs(),
            recent: self.recent.iter().cloned().collect(),
            last_error: self.last_error.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_newest_suggestions_first_and_capped() {
        let mut recorder = Recorder::default();
        for i in 0..RECENT_LIMIT + 5 {
            recorder.record(&format!("cmd{i}"), None, Duration::from_millis(10));
        }
        let stats = recorder.snapshot();
        assert_eq!(stats.recent.len(), RECENT_LIMIT);
        assert_eq!(stats.recent[0].typed, format!("cmd{}", RECENT_LIMIT + 4));
    }

    #[test]
    fn a_successful_call_clears_the_last_error() {
        let mut recorder = Recorder::default();
        recorder.record_error("llama-server down".into());
        assert_eq!(recorder.snapshot().last_error.as_deref(), Some("llama-server down"));
        recorder.record("ls", Some("ls -la"), Duration::from_millis(5));
        assert_eq!(recorder.snapshot().last_error, None);
    }

    #[test]
    fn counts_shells_without_underflowing() {
        let mut recorder = Recorder::default();
        recorder.shell_connected();
        recorder.shell_connected();
        recorder.shell_disconnected();
        assert_eq!(recorder.snapshot().shells, 1);
        recorder.shell_disconnected();
        recorder.shell_disconnected();
        assert_eq!(recorder.snapshot().shells, 0);
    }

    #[test]
    fn median_of_recent_latencies() {
        let mut recorder = Recorder::default();
        assert_eq!(recorder.snapshot().median_millis(), None);
        for millis in [300, 100, 200] {
            recorder.record("x", None, Duration::from_millis(millis));
        }
        assert_eq!(recorder.snapshot().median_millis(), Some(200));
    }
}
