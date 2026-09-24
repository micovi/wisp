# shellcheck shell=bash
# Shared by the install scripts; source it, do not run it.

# load_agent LABEL PLIST: (re)loads a launchd agent for this user, replacing any loaded copy.
load_agent() {
  local label="$1" plist="$2"
  local domain
  domain="gui/$(id -u)"
  plutil -lint "$plist" >/dev/null
  launchctl bootout "$domain/$label" 2>/dev/null || true
  # bootout returns before the old agent is gone, and bootstrap fails while it is still loaded.
  for _ in $(seq 1 50); do
    launchctl print "$domain/$label" >/dev/null 2>&1 || break
    sleep 0.2
  done
  launchctl bootstrap "$domain" "$plist"
}
