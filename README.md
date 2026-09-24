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

1. Start the model server. The first run downloads Qwen2.5-Coder 1.5B (1.65 GB); it then serves
   on port 8012.

   ```bash
   brew install llama.cpp
   llama-server --fim-qwen-1.5b-default
   ```

2. Install wisp:

   ```bash
   cargo install --path .
   ```

3. Add this to the end of `~/.zshrc`, after zsh-autosuggestions is loaded:

   ```zsh
   eval "$(wisp init zsh)"
   ```

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

## Debugging

```bash
wisp complete "git che"                 # suggestion + latency, using this dir and pane
wisp complete --show-prompt "git che"   # also print what the model sees
tail -f "$TMPDIR/wisp.log"              # daemon log
```

## Development

```bash
cargo test          # unit tests + daemon tests against a fake llama-server
prek run            # fmt, clippy -D warnings, tests
```
