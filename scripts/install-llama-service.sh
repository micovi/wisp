#!/usr/bin/env bash
# Installs a launchd agent that keeps llama-server running for wisp, starting it at login and
# restarting it if it exits. Re-running the script replaces the agent.
set -euo pipefail

label="dev.wisp.llama-server"
plist="$HOME/Library/LaunchAgents/$label.plist"
log="$HOME/Library/Logs/wisp-llama-server.log"
preset="--fim-qwen-1.5b-default"
port=8012

server=$(command -v llama-server) || {
  echo "llama-server not found; install it with: brew install llama.cpp" >&2
  exit 1
}

# The agent runs --offline so a missing network at login cannot stop it; the weights must
# already be cached. The first run of the preset downloads them.
if ! compgen -G "$HOME/.cache/huggingface/hub/models--ggml-org--Qwen2.5-Coder-1.5B-Q8_0-GGUF/snapshots/*/*.gguf" >/dev/null; then
  echo "Model not downloaded yet. Run this once, wait for 'server is listening', then Ctrl-C:" >&2
  echo "  llama-server $preset" >&2
  exit 1
fi

mkdir -p "$(dirname "$plist")"
cat >"$plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>$label</string>
  <key>ProgramArguments</key>
  <array>
    <string>$server</string>
    <string>$preset</string>
    <string>--offline</string>
    <string>--host</string>
    <string>127.0.0.1</string>
    <string>--port</string>
    <string>$port</string>
  </array>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <true/>
  <!-- Interactive keeps macOS from throttling it onto efficiency cores; latency matters here. -->
  <key>ProcessType</key>
  <string>Interactive</string>
  <key>StandardOutPath</key>
  <string>$log</string>
  <key>StandardErrorPath</key>
  <string>$log</string>
</dict>
</plist>
EOF
plutil -lint "$plist" >/dev/null

domain="gui/$(id -u)"
launchctl bootout "$domain/$label" 2>/dev/null || true
launchctl bootstrap "$domain" "$plist"

for _ in $(seq 1 60); do
  if curl -sf "http://127.0.0.1:$port/health" >/dev/null; then
    echo "llama-server is running on port $port (agent $label, log $log)"
    exit 0
  fi
  sleep 0.5
done
echo "llama-server did not become healthy within 30s; see $log" >&2
exit 1
