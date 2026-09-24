use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio::process::Command;

use crate::history;

const HISTORY_LIMIT: usize = 30;
const SCREEN_LINES: usize = 60;
const SCREEN_LINE_CHARS: usize = 200;
const FILE_LIMIT: usize = 40;
const BRANCH_LIMIT: usize = 10;
const PROBE_TIMEOUT: Duration = Duration::from_millis(500);

/// A command this shell ran, as reported by the zsh plugin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinishedCommand {
    pub command: String,
    pub status: i32,
}

/// Where to look when gathering context for a shell session.
#[derive(Debug, Clone, Default)]
pub struct Sources {
    pub cwd: PathBuf,
    /// `WezTerm` pane id; empty when the shell is not running inside `WezTerm`.
    pub pane: String,
    pub histfile: Option<PathBuf>,
}

/// Everything the model sees besides the partially typed command.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Context {
    pub cwd: String,
    pub git_branch: String,
    pub recent_branches: Vec<String>,
    pub files: Vec<String>,
    pub history: Vec<String>,
    pub screen: String,
    pub last_failure: Option<FinishedCommand>,
}

impl Context {
    /// Collects cwd files, git branches, shell history and the terminal screen concurrently.
    /// Each source is best-effort: a failing probe leaves its field empty and is logged.
    pub async fn gather(sources: &Sources, session: &[FinishedCommand]) -> Self {
        let cwd = sources.cwd.display().to_string();
        let branch_count = format!("--count={BRANCH_LIMIT}");
        let current_branch_args = ["-C", &cwd, "branch", "--show-current"];
        let recent_branch_args = [
            "-C",
            &cwd,
            "for-each-ref",
            "--sort=-committerdate",
            &branch_count,
            "--format=%(refname:short)",
            "refs/heads",
        ];
        let (git_branch, recent_branches, files, file_history, screen) = tokio::join!(
            run_probe("git", &current_branch_args),
            run_probe("git", &recent_branch_args),
            list_files(&sources.cwd),
            read_history(sources.histfile.as_deref()),
            capture_screen(&sources.pane),
        );

        let last_failure = session.last().filter(|cmd| cmd.status != 0).cloned();
        Self {
            cwd,
            git_branch: git_branch.trim().to_owned(),
            recent_branches: recent_branches.lines().map(str::to_owned).collect(),
            files,
            history: merge_history(file_history, session),
            screen: clean_screen(&screen),
            last_failure,
        }
    }

    /// Renders the context as a shell transcript for the model to continue with a `$ ` line. It
    /// does not depend on what the user is typing, so it stays identical between keystrokes and
    /// the model server can reuse its prompt cache.
    #[must_use]
    pub fn transcript(&self) -> String {
        let mut out = String::from("# zsh on macOS. Recent commands, oldest first:\n");
        for command in &self.history {
            let _ = writeln!(out, "$ {command}");
        }
        if !self.screen.is_empty() {
            out.push_str("# Terminal screen, newest output at the bottom:\n");
            out.push_str(&self.screen);
            out.push('\n');
        }
        let _ = writeln!(out, "# cwd: {}", self.cwd);
        if !self.files.is_empty() {
            let _ = writeln!(out, "# files: {}", self.files.join(" "));
        }
        if !self.git_branch.is_empty() {
            let _ = writeln!(out, "# git branch: {}", self.git_branch);
        }
        if !self.recent_branches.is_empty() {
            let _ = writeln!(out, "# recent branches: {}", self.recent_branches.join(" "));
        }
        if let Some(failed) = &self.last_failure {
            let _ = writeln!(out, "# `{}` failed with exit code {}", failed.command, failed.status);
        }
        out
    }
}

/// Session commands come last because they are the freshest; duplicates keep the newest spot.
fn merge_history(file_history: Vec<String>, session: &[FinishedCommand]) -> Vec<String> {
    let mut merged = file_history;
    for finished in session {
        merged.retain(|command| command != &finished.command);
        merged.push(finished.command.clone());
    }
    let excess = merged.len().saturating_sub(HISTORY_LIMIT);
    merged.drain(..excess);
    merged
}

fn clean_screen(raw: &str) -> String {
    let lines: Vec<String> =
        raw.lines().map(|line| line.trim_end().chars().take(SCREEN_LINE_CHARS).collect()).collect();
    let end = lines.iter().rposition(|line| !line.is_empty()).map_or(0, |i| i + 1);
    let start = end.saturating_sub(SCREEN_LINES);
    lines[start..end].join("\n")
}

async fn read_history(histfile: Option<&Path>) -> Vec<String> {
    let Some(path) = histfile else {
        return Vec::new();
    };
    history::read_recent(path, HISTORY_LIMIT).await.unwrap_or_else(|err| {
        tracing::warn!("reading shell history: {err:#}");
        Vec::new()
    })
}

async fn capture_screen(pane: &str) -> String {
    if pane.is_empty() {
        return String::new();
    }
    let start_line = format!("-{SCREEN_LINES}");
    run_probe("wezterm", &["cli", "get-text", "--pane-id", pane, "--start-line", &start_line]).await
}

async fn list_files(cwd: &Path) -> Vec<String> {
    let mut entries = match tokio::fs::read_dir(cwd).await {
        Ok(entries) => entries,
        Err(err) => {
            tracing::warn!("listing {}: {err}", cwd.display());
            return Vec::new();
        }
    };
    let mut names = Vec::new();
    while let Ok(Some(entry)) = entries.next_entry().await {
        let mut name = entry.file_name().to_string_lossy().into_owned();
        if entry.file_type().await.is_ok_and(|kind| kind.is_dir()) {
            name.push('/');
        }
        names.push(name);
    }
    // Visible entries first, then dotfiles, each alphabetical.
    names.sort_by(|a, b| (a.starts_with('.'), a).cmp(&(b.starts_with('.'), b)));
    names.truncate(FILE_LIMIT);
    names
}

/// Runs a short-lived command and returns its stdout, or an empty string if it fails.
async fn run_probe(program: &str, args: &[&str]) -> String {
    let output = Command::new(program).args(args).kill_on_drop(true).output();
    match tokio::time::timeout(PROBE_TIMEOUT, output).await {
        Ok(Ok(output)) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).into_owned()
        }
        Ok(Ok(output)) => {
            tracing::debug!("{program} {args:?} exited with {}", output.status);
            String::new()
        }
        Ok(Err(err)) => {
            tracing::warn!("running {program}: {err}");
            String::new()
        }
        Err(_) => {
            tracing::warn!("{program} {args:?} timed out after {PROBE_TIMEOUT:?}");
            String::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finished(command: &str, status: i32) -> FinishedCommand {
        FinishedCommand { command: command.to_owned(), status }
    }

    #[test]
    fn transcript_includes_every_section() {
        let context = Context {
            cwd: "/work/app".into(),
            git_branch: "main".into(),
            recent_branches: vec!["main".into(), "feat/login".into()],
            files: vec!["Cargo.toml".into(), "src/".into()],
            history: vec!["cargo build".into()],
            screen: "error[E0425]: cannot find value `x`".into(),
            last_failure: Some(finished("cargo build", 101)),
        };
        let prompt = context.transcript();
        for expected in [
            "$ cargo build\n",
            "error[E0425]",
            "# cwd: /work/app\n",
            "# files: Cargo.toml src/\n",
            "# git branch: main\n",
            "# recent branches: main feat/login\n",
            "# `cargo build` failed with exit code 101\n",
        ] {
            assert!(prompt.contains(expected), "missing {expected:?} in:\n{prompt}");
        }
    }

    #[test]
    fn transcript_skips_empty_sections() {
        let context = Context { cwd: "/tmp".into(), ..Context::default() };
        assert_eq!(
            context.transcript(),
            "# zsh on macOS. Recent commands, oldest first:\n# cwd: /tmp\n"
        );
    }

    #[test]
    fn session_commands_override_file_history_order() {
        let merged = merge_history(
            vec!["ls".into(), "make".into(), "git status".into()],
            &[finished("make", 0), finished("cargo run", 1)],
        );
        assert_eq!(merged, ["ls", "git status", "make", "cargo run"]);
    }

    #[test]
    fn merged_history_is_capped() {
        let file: Vec<String> = (0..HISTORY_LIMIT).map(|i| format!("cmd{i}")).collect();
        let merged = merge_history(file, &[finished("new", 0)]);
        assert_eq!(merged.len(), HISTORY_LIMIT);
        assert_eq!(merged.first().map(String::as_str), Some("cmd1"));
        assert_eq!(merged.last().map(String::as_str), Some("new"));
    }

    #[test]
    fn screen_is_trimmed_and_capped() {
        let mut raw: String = (0..100).fold(String::new(), |mut acc, i| {
            let _ = writeln!(acc, "line {i}   ");
            acc
        });
        raw.push_str("\n\n   \n");
        let screen = clean_screen(&raw);
        let lines: Vec<&str> = screen.lines().collect();
        assert_eq!(lines.len(), SCREEN_LINES);
        assert_eq!(lines.last(), Some(&"line 99"));
        assert_eq!(lines.first(), Some(&"line 40"));
    }

    #[test]
    fn long_screen_lines_are_truncated() {
        let screen = clean_screen(&"é".repeat(SCREEN_LINE_CHARS * 2));
        assert_eq!(screen.chars().count(), SCREEN_LINE_CHARS);
    }

    #[test]
    fn blank_screen_is_empty() {
        assert_eq!(clean_screen(""), "");
        assert_eq!(clean_screen("  \n\n"), "");
    }

    #[tokio::test]
    async fn gather_survives_missing_cwd_and_history() {
        let sources = Sources {
            cwd: "/nonexistent/wisp".into(),
            pane: String::new(),
            histfile: Some("/nonexistent/history".into()),
        };
        let context = Context::gather(&sources, &[finished("oops", 2)]).await;
        assert!(context.files.is_empty());
        assert!(context.git_branch.is_empty());
        assert_eq!(context.history, ["oops"]);
        assert_eq!(context.last_failure, Some(finished("oops", 2)));
    }

    #[tokio::test]
    async fn gather_lists_files_with_dirs_marked_and_dotfiles_last() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join(".env"), "").unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "").unwrap();
        let sources = Sources { cwd: dir.path().into(), ..Sources::default() };
        let context = Context::gather(&sources, &[]).await;
        assert_eq!(context.files, ["Cargo.toml", "src/", ".env"]);
    }
}
