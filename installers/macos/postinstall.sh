#!/bin/bash
# pkg postinstall — chạy root.
set -euo pipefail

APP_BUNDLE="/Applications/Sapo Printer Pro Max.app"
AGENT="$APP_BUNDLE/Contents/MacOS/sapo-printer-cert-manager"
PLIST_SRC="$APP_BUNDLE/Contents/Resources/com.sapo.printer.cert-manager.plist"
PLIST_DST="/Library/LaunchDaemons/com.sapo.printer.agent.plist"
DATA_DIR="/Library/Application Support/SapoPrinter"

mkdir -p "$DATA_DIR/tls"
chown -R root:wheel "$DATA_DIR"
chmod 750 "$DATA_DIR"
chmod 700 "$DATA_DIR/tls"

if [ ! -x "$AGENT" ]; then
    echo "Sapo Printer Pro Max agent binary not found: $AGENT" >&2
    exit 1
fi

# 1. Install CA vào System.keychain + sinh cert.
"$AGENT" --data-dir="$DATA_DIR" --install-ca

# 2. Copy plist + load daemon.
cp "$PLIST_SRC" "$PLIST_DST"
chown root:wheel "$PLIST_DST"
chmod 644 "$PLIST_DST"
launchctl load -w "$PLIST_DST"

echo "Sapo Printer Pro Max Agent registered and started"
exit 0
