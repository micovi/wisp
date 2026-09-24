use std::collections::HashSet;
use std::io::SeekFrom;
use std::path::Path;

use anyhow::Context as _;
use tokio::io::{AsyncReadExt as _, AsyncSeekExt as _};

/// zsh escapes bytes >= 0x83 in the history file as `META, byte ^ 32`.
const META: u8 = 0x83;

/// How much of the end of the history file is read; plenty for a few dozen commands.
const TAIL_BYTES: u64 = 256 * 1024;

/// Reads the most recent unique commands from a zsh history file, oldest first.
///
/// # Errors
///
/// Returns an error if the file cannot be opened or read.
pub async fn read_recent(path: &Path, limit: usize) -> anyhow::Result<Vec<String>> {
    let mut file = tokio::fs::File::open(path)
        .await
        .with_context(|| format!("opening history file {}", path.display()))?;
    let len = file.metadata().await?.len();
    let start = len.saturating_sub(TAIL_BYTES);
    file.seek(SeekFrom::Start(start)).await?;
    let mut raw = Vec::new();
    file.read_to_end(&mut raw).await?;
    if start > 0 {
        // The first line is probably cut in half.
        let first_newline = raw.iter().position(|&b| b == b'\n').map_or(raw.len(), |i| i + 1);
        raw.drain(..first_newline);
    }
    Ok(recent_commands(&raw, limit))
}

/// Parses zsh history bytes (plain or `EXTENDED_HISTORY` format) and returns the last `limit`
/// unique commands, oldest first.
#[must_use]
pub fn recent_commands(raw: &[u8], limit: usize) -> Vec<String> {
    let text = String::from_utf8_lossy(&unmetafy(raw)).into_owned();
    let mut commands = Vec::new();
    let mut pending = String::new();
    for line in text.lines() {
        if let Some(continued) = line.strip_suffix('\\') {
            pending.push_str(continued);
            pending.push('\n');
            continue;
        }
        pending.push_str(line);
        let entry = strip_extended_prefix(&pending).trim();
        if !entry.is_empty() {
            commands.push(entry.to_owned());
        }
        pending.clear();
    }

    let mut seen = HashSet::new();
    let mut recent: Vec<String> =
        commands.into_iter().rev().filter(|cmd| seen.insert(cmd.clone())).take(limit).collect();
    recent.reverse();
    recent
}

fn unmetafy(raw: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(raw.len());
    let mut bytes = raw.iter();
    while let Some(&b) = bytes.next() {
        if b == META {
            if let Some(&next) = bytes.next() {
                out.push(next ^ 32);
            }
        } else {
            out.push(b);
        }
    }
    out
}

/// Strips the `: <start>:<elapsed>;` prefix written when `EXTENDED_HISTORY` is set.
fn strip_extended_prefix(entry: &str) -> &str {
    let Some(rest) = entry.strip_prefix(": ") else {
        return entry;
    };
    let Some((stamp, command)) = rest.split_once(';') else {
        return entry;
    };
    let is_stamp = stamp.split(':').all(|part| {
        let part = part.trim();
        !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit())
    });
    if is_stamp { command } else { entry }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_history() {
        let raw = b"ls\ngit status\ncargo build\n";
        assert_eq!(recent_commands(raw, 10), ["ls", "git status", "cargo build"]);
    }

    #[test]
    fn strips_extended_timestamps() {
        let raw = b": 1700000000:0;ls -la\n: 1700000005:12;cargo test\n";
        assert_eq!(recent_commands(raw, 10), ["ls -la", "cargo test"]);
    }

    #[test]
    fn keeps_commands_that_only_look_like_timestamps() {
        let raw = b": not a stamp;echo hi\n";
        assert_eq!(recent_commands(raw, 10), [": not a stamp;echo hi"]);
    }

    #[test]
    fn joins_multiline_commands() {
        let raw = b": 1700000000:0;for f in *; do\\\n  echo $f\\\ndone\nls\n";
        assert_eq!(recent_commands(raw, 10), ["for f in *; do\n  echo $f\ndone", "ls"]);
    }

    #[test]
    fn unmetafies_non_ascii_bytes() {
        // "é" is 0xC3 0xA9; zsh stores 0xA9 as META, 0xA9 ^ 32.
        let raw = [b'e', b'c', b'h', b'o', b' ', 0xC3, META, 0xA9 ^ 32, b'\n'];
        assert_eq!(recent_commands(&raw, 10), ["echo é"]);
    }

    #[test]
    fn dedupes_keeping_latest_position_and_limit() {
        let raw = b"a\nb\na\nc\nd\n";
        assert_eq!(recent_commands(raw, 3), ["a", "c", "d"]);
    }

    #[test]
    fn empty_input_gives_no_commands() {
        assert!(recent_commands(b"", 10).is_empty());
        assert!(recent_commands(b"\n\n", 10).is_empty());
    }

    #[tokio::test]
    async fn read_recent_drops_partial_first_line_of_large_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history");
        let filler = "x".repeat(usize::try_from(TAIL_BYTES).unwrap());
        std::fs::write(&path, format!("{filler}\nfirst\nsecond\n")).unwrap();
        assert_eq!(read_recent(&path, 10).await.unwrap(), ["first", "second"]);
    }

    #[tokio::test]
    async fn read_recent_reports_missing_file() {
        let err = read_recent(Path::new("/nonexistent/wisp-history"), 10).await;
        assert!(err.is_err_and(|e| e.to_string().contains("/nonexistent/wisp-history")));
    }
}
