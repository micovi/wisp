# wisp

Grey "ghost text" command suggestions for zsh, written by a small local model that sees what you
see: the terminal output above the prompt, your shell history, the files in the current directory
and your git branches.

```
% git che|ckout feat/login-page      ← grey part is the suggestion; → accepts it
```

- **After a command finishes**, wisp predicts the next one while the prompt is still empty.
- **While you type**, it completes the current line after a 60 ms pause in typing.
- Everything runs locally. Nothing leaves your machine.

## How it works

```
zsh plugin ──unix socket──▶ wisp daemon ──HTTP──▶ llama-server (Qwen2.5-Coder)
 (keystrokes, finished        (context, debounce,
  commands)                    cancellation)
```

- **zsh plugin** (`shell/wisp.zsh`).
  - After each command, it sends the command, its exit code and the cwd.
  - While you edit the line, it sends the current buffer.
  - It shows replies through zsh-autosuggestions, which draws the grey text and provides the
    accept keys.
- **Daemon** (`wisp daemon`). The plugin starts it on demand.
  - After each command it gathers context:
    - the last 60 lines of the WezTerm pane (`wezterm cli get-text`);
    - recent history;
    - the cwd listing;
    - git branches.
  - It turns that context into a shell transcript that ends in `$ <what you typed>`, and a base
    code model continues that line.
  - The transcript's prefix doesn't change while you type, so llama-server reuses its prompt
    cache and processes only the new characters.
  - A new keystroke cancels the request in flight.
  - A suggestion you keep typing along with is reused without asking the model again.

## Setup

Requires zsh with [zsh-autosuggestions](https://github.com/zsh-users/zsh-autosuggestions) loaded
**before** wisp.

1. Download the model and keep llama-server running.

   The first run of the preset downloads Qwen2.5-Coder 1.5B (1.65 GB). Stop it with Ctrl-C once
   it says `server is listening`. The script then installs a launchd agent: it starts
   llama-server at login on port 8012 and restarts it if it exits.

   ```bash
   brew install llama.cpp
   llama-server --fim-qwen-1.5b-default
   ./scripts/install-llama-service.sh
   ```

   To use Qwen2.5-Coder 3B instead, run `llama-server --fim-qwen-3b-default` once to download it
   (3.3 GB), then `./scripts/install-llama-service.sh 3b`. Its suggestions are longer and more
   complete; it answers in ~170–450 ms, versus ~50–120 ms for 1.5B. Run the script with `1.5b`
   to switch back.

   The agent logs to `~/Library/Logs/wisp-llama-server.log`. To remove it:

   ```bash
   launchctl bootout gui/$(id -u)/dev.wisp.llama-server
   trash ~/Library/LaunchAgents/dev.wisp.llama-server.plist
   ```

2. Install wisp:

   ```bash
   cargo install --path .
   ```

3. Add this to the end of `~/.zshrc`, after zsh-autosuggestions is loaded:

   ```zsh
   eval "$(wisp init zsh)"
   ```

4. Optional: show wisp in the macOS menu bar. The script installs a launchd agent that starts it
   at login:

   ```bash
   ./scripts/install-menubar-service.sh
   ```

   The title reads `wisp 3B · 180ms`: the model size and the median latency. It changes to
   `wisp ⏸` while suggestions are paused and to `wisp ⚠` when llama-server is down. The menu
   shows:
   - the loaded model and its memory use;
   - how many shells are connected;
   - the last 10 suggestions with their latency.

   From the menu you can pause suggestions, restart llama-server or open either log.

   macOS hides menu bar items that do not fit, which happens often on notched MacBooks. If the
   item is missing, click `«` in the menu bar or check System Settings → Menu Bar.

Accept a suggestion with → / End / Ctrl-F. These are zsh-autosuggestions' keys.

Screen context works only inside WezTerm. In other terminals wisp still uses history, files, git
and the commands it saw finish.

## Configuration

| Variable       | Default                  | Purpose                                |
| -------------- | ------------------------ | -------------------------------------- |
| `WISP_LLM_URL` | `http://127.0.0.1:8012`  | llama-server the daemon talks to       |
| `WISP_LOG`     | `wisp=info`              | Log filter, e.g. `wisp=debug`          |

The plugin starts the daemon with the environment of your shell. After you change these
variables, restart the daemon: `pkill -f "wisp daemon"`. The next prompt starts it again.

## Status and pausing

```bash
wisp status          # llama-server, daemon, recent suggestions (--json for scripts)
wisp pause           # no suggestions until `wisp resume`; survives restarts
wisp resume
```

## Debugging

```bash
wisp complete "git che"                 # suggestion + latency, using this dir and pane
wisp complete --show-prompt "git che"   # also print what the model sees
tail -f ~/.cache/wisp/wisp.log          # daemon log
```

## Landing page

`site/` is the landing page. It is static HTML and CSS with no build step, so any static host
can serve it. To preview it locally:

```bash
python3 -m http.server -d site 8000
```

Its visual system is recorded in `DESIGN.md`, and the product facts it may claim are in
`PRODUCT.md`.

## Development

```bash
cargo test          # unit tests + daemon tests against a fake llama-server
prek run            # fmt, clippy -D warnings, tests
```
