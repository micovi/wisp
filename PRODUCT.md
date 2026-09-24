# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Stack

Static HTML/CSS in `site/`, no build step and no dependencies. It must be deployable as-is to GitHub Pages, Cloudflare Pages or Vercel. The deploy target is undecided.

## Users

Developers on macOS who spend their day in zsh. They type long, specific commands: branch names, pod names, file paths, PIDs that just scrolled past in the terminal. Their job is to retype those without copying and pasting, and without leaving the keyboard. They already know zsh-autosuggestions and are skeptical of AI tools that send their terminal to the cloud.

## Product Purpose

wisp shows grey "ghost text" command suggestions in zsh, written by a small local code model. The model sees what the user sees: the terminal output above the prompt, shell history, the files in the current directory and git branches. After a command finishes, wisp predicts the next one; while the user types, it completes the line. Success means the user presses → instead of retyping something that is already on screen.

## Positioning

wisp reads the terminal output above the prompt and completes names that just appeared there: `docker-compose.prod.yml` from an `ls`, a crash-looping pod from `kubectl get pods`, a PID from `lsof`. It does this inline as ghost text, in the user's existing terminal, with a model running on their own Mac. Hosted tools send the terminal to the cloud (Kiro CLI, Warp) or require their own terminal app (Warp, Wave). History-only tools (zsh-autosuggestions, Atuin) cannot complete what they have never seen.

## Operating Context

- **Hardware and model.** Apple Silicon Mac. llama.cpp's `llama-server` runs Qwen2.5-Coder 1.5B or 3B as a launchd agent. The model always stays on the Mac.
- **Shell.** zsh with zsh-autosuggestions, which draws the grey text and provides the accept keys (→, End, Ctrl-F). Works alongside zsh-vi-mode, fzf-tab and starship.
- **Screen context.** Comes from `wezterm cli get-text`, so it requires WezTerm. Other terminals still get history, files, git and session commands.
- **Menu bar and CLI.** A macOS menu bar item and `wisp status` show the model, latency and recent suggestions. `wisp pause` / `wisp resume` switch suggestions off and on.
- **Install.** `brew install llama.cpp`, `cargo install --path .`, the service scripts, then `eval "$(wisp init zsh)"` in `~/.zshrc`.

## Capabilities and Constraints

- Predicts the next command on an empty prompt after each command finishes.
- Completes the current line while typing, after a 60 ms pause.
- Token healing: the partial word being typed is forced through a GBNF grammar instead of ending the prompt mid-token.
- Reuses the prompt cache between keystrokes. A suggestion the user keeps typing along with is reused without another model call.
- macOS only. zsh only; fish has no autosuggestion hook.
- Screen reading works only in WezTerm. Other terminals would need a PTY proxy, which is planned but not built.
- Requires Rust (cargo) to install today; there is no Homebrew formula or binary release yet.
- The GitHub repo is `micovi/wisp`. It is private for now, and the public release date is undecided.

## Brand Commitments

- The name is **wisp**, always lowercase.
- There is no logo, palette or visual identity yet.

## Evidence on Hand

All figures are internal measurements on an Apple M5 with 24 GB. They must be labeled as such.

- **Latency while typing, with a cached prompt.**
  - Qwen2.5-Coder 1.5B: about 50–120 ms.
  - Qwen2.5-Coder 3B: about 150–450 ms.
  - Cold first request after a command: about 180–280 ms.
- **Accuracy.** An 8-scenario eval where the right answer is visible only in terminal output. Scenarios: compose file, git branch, failing test, pod name, stack-trace path, PID, next command after `git add`, container name.
  - Without token healing: 1.5B scored 4/8, 3B scored 3/8.
  - With token healing: 1.5B scored 6/8, 3B scored 7/8.
- **Real completions observed.**
  - `docker compose -f docker-compose.p` → `docker-compose.prod.yml`
  - `git checkout feat/p` → `feat/payments-refactor`
  - `kubectl logs -n billing api` → `api-7d9f8b6c4-x2kqp`
  - `nvim src/b` → `src/billing/invoice_service.py`
  - `kill ` → `48213`
  - `docker exec -it pgl-d` → `pgl-db-1`
  - `cargo cl` → `cargo clippy --all-targets --all-features -- -D warnings` (3B, from history)
- **Memory.** The 3B llama-server uses about 4.3 GB.
- **License.** MIT (open source).
- **Absent: do not fabricate.** No users, testimonials, stars, download counts, press, or company logos. No benchmark against other tools. No screenshots or recordings yet.

## Product Principles

1. **What's on screen is the context.** A suggestion should use names the user can see, not generic guesses.
2. **Local means local.** Nothing typed or displayed leaves the machine. Say so plainly and never qualify it with a cloud option that does not exist.
3. **Never in the way.** Ghost text only, the user's own terminal, the user's own keys. A slow or wrong suggestion costs one keystroke to ignore.
4. **Honest numbers.** Every figure comes from a measurement the user can reproduce with `wisp complete` and `wisp status`.
