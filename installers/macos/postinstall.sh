#!/bin/bash
# pkg postinstall — chạy root.
set -euo pipefail

AGENT="/Applications/Sapo Printer.app/Contents/MacOS/sapo-printer-agent"
PLIST_SRC="/Applications/Sapo Printer.app/Contents/Resources/com.sapo.printer.agent.plist"
PLIST_DST="/Library/LaunchDaemons/com.sapo.printer.agent.plist"
DATA_DIR="/Library/Application Support/SapoPrinter"

mkdir -p "$DATA_DIR/tls"
chown -R root:wheel "$DATA_DIR"
chmod 750 "$DATA_DIR"
chmod 700 "$DATA_DIR/tls"

# 1. Install CA vào System.keychain + sinh cert.
"$AGENT" --data-dir="$DATA_DIR" --install-ca

# 2. Copy plist + load daemon.
cp "$PLIST_SRC" "$PLIST_DST"
chown root:wheel "$PLIST_DST"
chmod 644 "$PLIST_DST"
launchctl load -w "$PLIST_DST"

echo "Sapo Printer Agent registered and started"
exit 0
