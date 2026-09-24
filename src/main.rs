use std::io::Write as _;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::Context as _;
use clap::{Parser, Subcommand, ValueEnum};
use tracing_subscriber::EnvFilter;
use wisp::context::{Context, Sources};
use wisp::daemon::{self, Config};
use wisp::model::{HealedPrompt, LlamaClient};

const ZSH_PLUGIN: &str = include_str!("../shell/wisp.zsh");
const DEBOUNCE: Duration = Duration::from_millis(60);

/// AI ghost-text command suggestions for zsh.
#[derive(Parser)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Print the shell plugin. Add `eval "$(wisp init zsh)"` to the end of ~/.zshrc.
    Init {
        shell: Shell,
        /// Defaults to ~/.cache/wisp/wisp.sock.
        #[arg(long)]
        socket: Option<PathBuf>,
    },
    /// Run the daemon the shell plugin talks to. The plugin starts it on demand.
    Daemon {
        /// Defaults to ~/.cache/wisp/wisp.sock.
        #[arg(long)]
        socket: Option<PathBuf>,
        /// Base URL of the llama-server that generates suggestions.
        #[arg(long, env = "WISP_LLM_URL", default_value = "http://127.0.0.1:8012")]
        llm_url: String,
    },
    /// Suggest a completion for BUFFER from this directory and terminal pane, for debugging.
    Complete {
        buffer: String,
        /// Print the prompt sent to the model.
        #[arg(long)]
        show_prompt: bool,
        #[arg(long, env = "WISP_LLM_URL", default_value = "http://127.0.0.1:8012")]
        llm_url: String,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum Shell {
    Zsh,
}

/// A fixed per-user path, so every shell finds the same daemon even when tools such as
/// nix-shell or direnv change `TMPDIR`.
fn socket_or_default(socket: Option<PathBuf>) -> anyhow::Result<PathBuf> {
    if let Some(socket) = socket {
        return Ok(socket);
    }
    let home = std::env::home_dir().context("HOME is not set; pass --socket explicitly")?;
    Ok(home.join(".cache/wisp/wisp.sock"))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_env("WISP_LOG").unwrap_or_else(|_| EnvFilter::new("wisp=info")),
        )
        .with_writer(std::io::stderr)
        .init();

    match Cli::parse().command {
        Command::Init { shell: Shell::Zsh, socket } => {
            print_zsh_plugin(&socket_or_default(socket)?)
        }
        Command::Daemon { socket, llm_url } => {
            let socket = socket_or_default(socket)?;
            let model = LlamaClient::new(&llm_url)?;
            daemon::run(Config { socket, model, debounce: DEBOUNCE }).await
        }
        Command::Complete { buffer, show_prompt, llm_url } => {
            complete_once(&buffer, show_prompt, &llm_url).await
        }
    }
}

fn print_zsh_plugin(socket: &std::path::Path) -> anyhow::Result<()> {
    let exe = std::env::current_exe().context("locating the wisp binary")?;
    let plugin = ZSH_PLUGIN
        .replace("__WISP_BIN__", &shell_quote(&exe.display().to_string()))
        .replace("__WISP_SOCKET__", &shell_quote(&socket.display().to_string()));
    std::io::stdout().write_all(plugin.as_bytes()).context("writing plugin to stdout")
}

async fn complete_once(buffer: &str, show_prompt: bool, llm_url: &str) -> anyhow::Result<()> {
    let sources = Sources {
        cwd: std::env::current_dir().context("reading current directory")?,
        pane: std::env::var("WEZTERM_PANE").unwrap_or_default(),
        histfile: std::env::var_os("HISTFILE")
            .map(PathBuf::from)
            .or_else(|| std::env::home_dir().map(|home| home.join(".zsh_history"))),
    };
    let gather_started = Instant::now();
    let context = Context::gather(&sources, &[]).await;
    let gather_time = gather_started.elapsed();
    let transcript = context.transcript();

    let mut out = std::io::stdout().lock();
    if show_prompt {
        let healed = HealedPrompt::new(&transcript, buffer);
        writeln!(out, "{}\n--- grammar: {}", healed.prompt, healed.grammar)?;
    }
    let model = LlamaClient::new(llm_url)?;
    for attempt in ["cold", "cached"] {
        let started = Instant::now();
        let suggestion = model.complete_command(&transcript, buffer).await?;
        writeln!(
            out,
            "{attempt}: {suggestion:?} in {:?} (context gathered in {gather_time:?})",
            started.elapsed()
        )?;
    }
    Ok(())
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}
