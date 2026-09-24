#!/usr/bin/env bash
# Installs a launchd agent that shows `wisp menubar` at login. Choosing Quit in the menu stops it
# until the next login; a crash restarts it. Re-running the script replaces the agent.
set -euo pipefail
# shellcheck source=scripts/launchd.sh
source "$(dirname "$0")/launchd.sh"

label="dev.wisp.menubar"
plist="$HOME/Library/LaunchAgents/$label.plist"
log="$HOME/Library/Logs/wisp-menubar.log"

wisp=$(command -v wisp) || {
  echo "wisp not found on PATH; install it with: cargo install --path ." >&2
  exit 1
}

mkdir -p "$(dirname "$plist")"
cat >"$plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>$label</string>
  <key>ProgramArguments</key>
  <array>
    <string>$wisp</string>
    <string>menubar</string>
  </array>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <dict>
    <key>SuccessfulExit</key>
    <false/>
  </dict>
  <key>ProcessType</key>
  <string>Interactive</string>
  <key>StandardOutPath</key>
  <string>$log</string>
  <key>StandardErrorPath</key>
  <string>$log</string>
</dict>
</plist>
PLIST

load_agent "$label" "$plist"
echo "wisp menu bar item is running (agent $label, log $log)"
