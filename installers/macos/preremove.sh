#!/bin/bash
# pkg preremove — chạy root khi uninstall.
set +e

AGENT="/Applications/Sapo Printer.app/Contents/MacOS/sapo-printer-agent"
PLIST_DST="/Library/LaunchDaemons/com.sapo.printer.agent.plist"
DATA_DIR="/Library/Application Support/SapoPrinter"

# 1. Unload daemon.
if [ -f "$PLIST_DST" ]; then
    launchctl unload "$PLIST_DST" 2>/dev/null
    rm -f "$PLIST_DST"
fi

# 2. Uninstall CA.
if [ -x "$AGENT" ]; then
    "$AGENT" --data-dir="$DATA_DIR" --uninstall-ca
fi

exit 0
