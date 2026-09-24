use std::time::Duration;

use anyhow::Context as _;
use serde::{Deserialize, Serialize};

const MAX_TOKENS: u32 = 48;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Client for llama.cpp's `llama-server` `/completion` endpoint.
#[derive(Debug, Clone)]
pub struct LlamaClient {
    http: reqwest::Client,
    endpoint: String,
}

#[derive(Serialize)]
struct CompletionRequest<'a> {
    prompt: &'a str,
    grammar: &'a str,
    n_predict: u32,
    temperature: f32,
    stop: [&'a str; 1],
    cache_prompt: bool,
}

#[derive(Deserialize)]
struct CompletionResponse {
    content: String,
}

/// A prompt that stops before the word being typed, plus that word, which the model must
/// regenerate first.
///
/// Ending the prompt mid-word (`docker-compose.p`) or on a space splits tokens in ways the model
/// never saw in training, so it continues badly (`.prd`). Cutting back to the last space and
/// forcing the partial word through a grammar lets the model pick its own tokens for it.
#[derive(Debug, PartialEq, Eq)]
pub struct HealedPrompt {
    pub prompt: String,
    pub forced: String,
    pub grammar: String,
}

impl HealedPrompt {
    /// Builds the prompt for completing `buffer` as the next `$ ` line after `transcript`.
    #[must_use]
    pub fn new(transcript: &str, buffer: &str) -> Self {
        let line = format!(" {buffer}");
        let cut = line.rfind(' ').unwrap_or_default();
        let (stem, forced) = line.split_at(cut);
        let literal = forced.replace('\\', r"\\").replace('"', r#"\""#);
        Self {
            prompt: format!("{transcript}${stem}"),
            forced: forced.to_owned(),
            grammar: format!(r#"root ::= "{literal}" [^\n]* "\n""#),
        }
    }
}

impl LlamaClient {
    /// Creates a client for the llama-server at `base_url`, e.g. `http://127.0.0.1:8012`.
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP client cannot be built.
    pub fn new(base_url: &str) -> anyhow::Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .context("building HTTP client")?;
        let endpoint = format!("{}/completion", base_url.trim_end_matches('/'));
        Ok(Self { http, endpoint })
    }

    /// Asks the model how the command line `buffer` continues after `transcript`. Returns the
    /// full suggested command line, or `None` when the model has nothing to add.
    ///
    /// # Errors
    ///
    /// Returns an error if the server is unreachable or answers with an error or bad JSON.
    pub async fn complete_command(
        &self,
        transcript: &str,
        buffer: &str,
    ) -> anyhow::Result<Option<String>> {
        let healed = HealedPrompt::new(transcript, buffer);
        let request = CompletionRequest {
            prompt: &healed.prompt,
            grammar: &healed.grammar,
            n_predict: MAX_TOKENS,
            temperature: 0.0,
            stop: ["\n"],
            cache_prompt: true,
        };
        let response = self
            .http
            .post(&self.endpoint)
            .json(&request)
            .send()
            .await
            .with_context(|| {
                format!("calling {} (is llama-server running? see README)", self.endpoint)
            })?
            .error_for_status()
            .with_context(|| format!("llama-server error from {}", self.endpoint))?;
        let body: CompletionResponse =
            response.json().await.context("decoding llama-server response")?;
        Ok(suggestion(buffer, &healed.forced, &body.content))
    }
}

/// Turns the model output, which starts with the forced partial word, into the full suggested
/// command line.
#[must_use]
pub fn suggestion(buffer: &str, forced: &str, output: &str) -> Option<String> {
    let Some(continuation) = output.strip_prefix(forced) else {
        tracing::warn!("model output {output:?} ignored the forced prefix {forced:?}");
        return None;
    };
    let continuation = continuation.lines().next().unwrap_or_default().trim_end();
    if continuation.trim().is_empty() {
        return None;
    }
    let continuation = if buffer.is_empty() || buffer.ends_with(' ') {
        continuation.trim_start()
    } else {
        continuation
    };
    Some(format!("{buffer}{continuation}").replace('\t', " "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heals_the_partial_last_word() {
        let healed = HealedPrompt::new("# ctx\n", "docker compose -f docker-compose.p");
        assert_eq!(healed.prompt, "# ctx\n$ docker compose -f");
        assert_eq!(healed.forced, " docker-compose.p");
        assert_eq!(healed.grammar, r#"root ::= " docker-compose.p" [^\n]* "\n""#);
    }

    #[test]
    fn heals_a_trailing_space_and_an_empty_line() {
        let after_space = HealedPrompt::new("", "kill ");
        assert_eq!((after_space.prompt.as_str(), after_space.forced.as_str()), ("$ kill", " "));
        let empty = HealedPrompt::new("", "");
        assert_eq!((empty.prompt.as_str(), empty.forced.as_str()), ("$", " "));
        let one_word = HealedPrompt::new("", "gi");
        assert_eq!((one_word.prompt.as_str(), one_word.forced.as_str()), ("$", " gi"));
    }

    #[test]
    fn escapes_quotes_and_backslashes_in_the_grammar() {
        let healed = HealedPrompt::new("", r#"echo "a\b"#);
        assert_eq!(healed.grammar, r#"root ::= " \"a\\b" [^\n]* "\n""#);
    }

    #[test]
    fn appends_continuation_to_buffer() {
        assert_eq!(
            suggestion("git che", " che", " checkout main"),
            Some("git checkout main".into())
        );
    }

    #[test]
    fn keeps_only_the_first_line() {
        assert_eq!(suggestion("ls", " ls", " ls -la\nrm -rf /"), Some("ls -la".into()));
    }

    #[test]
    fn drops_empty_or_blank_continuations() {
        assert_eq!(suggestion("ls", " ls", " ls"), None);
        assert_eq!(suggestion("ls", " ls", " ls   \n"), None);
        assert_eq!(suggestion("ls", " ls", " ls\n-la"), None);
    }

    #[test]
    fn drops_output_that_ignores_the_forced_prefix() {
        assert_eq!(suggestion("git che", " che", "rry-pick"), None);
    }

    #[test]
    fn avoids_double_spaces_after_a_trailing_space() {
        assert_eq!(suggestion("git ", " ", "  status"), Some("git status".into()));
        assert_eq!(suggestion("", " ", " cargo test"), Some("cargo test".into()));
    }

    #[test]
    fn replaces_tabs_so_the_reply_protocol_stays_intact() {
        assert_eq!(suggestion("echo", " echo", " echo\ta\tb"), Some("echo a b".into()));
    }
}
