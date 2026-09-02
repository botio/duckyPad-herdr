#!/usr/bin/env bash
#
# duckyPad Herdr Bridge — one-shot installer for Linux and macOS.
#
# What it does (idempotent, safe to re-run after `git pull`):
#   1. Builds the daemon:          cargo build --release
#   2. Registers the plugin with herdr (best-effort; only if `herdr` is
#      on PATH)
#   3. Installs the daemon as a USER SERVICE and starts it:
#        Linux  -> systemd user unit  ~/.config/systemd/user/ducky-pad-bridge.service
#        macOS  -> launchd LaunchAgent ~/Library/LaunchAgents/com.botio.ducky-pad-bridge.plist
#      (a service, not a herdr [[startup]] hook: herdr's startup hooks are
#      one-shot and must exit, while this daemon has to stay alive)
#
# The Linux unit file is only written if it does not already exist (so an
# existing customized unit is kept); the macOS plist is regenerated on
# every run (it is a generated file — edit it only if you need to).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN="$SCRIPT_DIR/target/release/ducky-pad-bridge"

SERVICE="ducky-pad-bridge"
LABEL="com.botio.ducky-pad-bridge"

os="$(uname -s)"

if ! command -v cargo >/dev/null 2>&1; then
  echo "error: cargo not found on PATH — install the Rust toolchain first (e.g. https://rustup.rs)." >&2
  exit 1
fi

echo "==> [1/3] building the daemon (cargo build --release)"
(cd "$SCRIPT_DIR" && cargo build --release)

if [ -x "$BIN" ]; then
  echo "==> [2/3] registering the plugin with herdr (best-effort)"
  if command -v herdr >/dev/null 2>&1; then
    herdr plugin link "$SCRIPT_DIR" || \
      echo "   (herdr plugin link failed — the service still works; re-run later if needed)"
  else
    echo "   (herdr not on PATH — skipped; run 'herdr plugin link $SCRIPT_DIR' once herdr is installed)"
  fi
else
  echo "==> [2/3] no binary produced — skipping plugin registration"
fi

echo "==> [3/3] installing the user service"
case "$os" in
  Linux)
    UNIT_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
    UNIT="$UNIT_DIR/$SERVICE.service"
    mkdir -p "$UNIT_DIR"
    if [ -f "$UNIT" ]; then
      echo "   keeping existing $UNIT"
    else
      cat > "$UNIT" <<EOF
[Unit]
Description=duckyPad Herdr Bridge daemon
# Installed by herdr-ducky-pad/install.sh — the daemon polls herdr's Unix
# socket and drives the duckyPad; it must stay alive, so it runs as a user
# service rather than a herdr [[startup]] hook.

[Service]
Type=simple
ExecStart=$BIN
Restart=on-failure
RestartSec=2
Environment=RUST_LOG=info

[Install]
WantedBy=default.target
EOF
      echo "   wrote $UNIT"
    fi
    systemctl --user daemon-reload
    systemctl --user enable --now "$SERVICE"
    echo
    echo "Installed and running (systemd user service '$SERVICE')."
    echo "  status:  systemctl --user status $SERVICE"
    echo "  logs:    journalctl --user -u $SERVICE -f"
    ;;
  Darwin)
    PLIST_DIR="$HOME/Library/LaunchAgents"
    PLIST="$PLIST_DIR/$LABEL.plist"
    mkdir -p "$PLIST_DIR"
    cat > "$PLIST" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>$LABEL</string>
    <key>ProgramArguments</key>
    <array>
        <string>$BIN</string>
    </array>
    <key>EnvironmentVariables</key>
    <dict>
        <key>RUST_LOG</key>
        <string>info</string>
    </dict>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <dict>
        <key>SuccessfulExit</key>
        <false/>
    </dict>
    <key>StandardOutPath</key>
    <string>/tmp/ducky-pad-bridge.out.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/ducky-pad-bridge.log</string>
</dict>
</plist>
EOF
    echo "   wrote $PLIST"
    UID_NUM="$(id -u)"
    launchctl bootout "gui/$UID_NUM/$LABEL" 2>/dev/null || true
    launchctl bootstrap "gui/$UID_NUM" "$PLIST" 2>/dev/null || \
      launchctl load -w "$PLIST"
    echo
    echo "Installed and running (launchd LaunchAgent '$LABEL')."
    echo "  status:  launchctl list | grep ducky-pad-bridge"
    echo "  logs:    tail -f /tmp/ducky-pad-bridge.log"
    ;;
  *)
    echo "error: unsupported platform '$os' (this script supports Linux and macOS)." >&2
    exit 1
    ;;
esac
