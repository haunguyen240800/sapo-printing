#!/usr/bin/env bash
#
# Manual release helper for Git Bash (Windows).
#
# What it does:
#   1. Reads the product name and version from src-tauri/tauri.conf.json
#   2. Locates the NSIS installer + its .sig produced by `pnpm build`
#   3. Generates latest.json (the updater manifest) with the correct
#      signature content and GitHub download URL
#   4. Creates the GitHub release (if `gh` is installed), otherwise prints
#      manual upload instructions
#
# Prerequisites:
#   - You already ran `pnpm build` with TAURI_SIGNING_PRIVATE_KEY set,
#     so the .exe and .exe.sig exist under target/release/bundle/nsis/
#   - (optional) gh CLI installed + `gh auth login` done
#
# Usage (from anywhere in the repo):
#   bash scripts/release.sh              # dry run: build latest.json, show gh command
#   bash scripts/release.sh --publish    # also create the GitHub release via gh
#
set -euo pipefail

# --- Config -----------------------------------------------------------------
REPO="haunguyen240800/sapo-printing"

# Resolve repo root relative to this script (scripts/ is one level under root).
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

CONF="$ROOT/src-tauri/tauri.conf.json"
NSIS_DIR="$ROOT/src-tauri/target/release/bundle/nsis"

PUBLISH=0
[ "${1:-}" = "--publish" ] && PUBLISH=1

# --- 1. Read product metadata -----------------------------------------------
if [ ! -f "$CONF" ]; then
  echo "ERROR: cannot find $CONF" >&2
  exit 1
fi

read_config_field() {
  node -e '
    const fs = require("fs");
    const config = JSON.parse(fs.readFileSync(process.argv[1], "utf8"));
    const value = config[process.argv[2]];
    if (typeof value !== "string" || value.trim() === "") process.exit(1);
    process.stdout.write(value);
  ' "$CONF" "$1"
}

if ! PRODUCT_NAME="$(read_config_field productName)"; then
  echo "ERROR: could not parse productName from tauri.conf.json" >&2
  exit 1
fi
if ! VERSION="$(read_config_field version)"; then
  echo "ERROR: could not parse version from tauri.conf.json" >&2
  exit 1
fi
if [ -z "$VERSION" ]; then
  echo "ERROR: could not parse version from tauri.conf.json" >&2
  exit 1
fi
TAG="v$VERSION"
echo ">> Product: $PRODUCT_NAME"
echo ">> Version: $VERSION  (tag: $TAG)"

# --- 2. Locate installer + signature ---------------------------------------
EXE="$NSIS_DIR/${PRODUCT_NAME}_${VERSION}_x64-setup.exe"
SIG="$EXE.sig"

if [ ! -f "$EXE" ]; then
  echo "ERROR: installer not found: $EXE" >&2
  echo "       Did you run 'pnpm build' after bumping the version?" >&2
  exit 1
fi
if [ ! -f "$SIG" ]; then
  echo "ERROR: signature not found: $SIG" >&2
  echo "       Did you set TAURI_SIGNING_PRIVATE_KEY before building?" >&2
  exit 1
fi
echo ">> Installer: $EXE"
echo ">> Signature: $SIG"

# --- 3. Generate latest.json ------------------------------------------------
# GitHub replaces spaces in asset filenames with dots in the download URL.
ASSET_NAME="$(basename "$EXE")"
URL_NAME="${ASSET_NAME// /.}"
DOWNLOAD_URL="https://github.com/$REPO/releases/download/$TAG/$URL_NAME"

# The .sig content is a single line of base64 (no chars needing JSON escaping).
# Read it raw; command substitution strips any trailing newline. Do NOT append \n —
# a trailing newline in the signature breaks base64 decoding on the client.
SIGNATURE="$(cat "$SIG")"

PUB_DATE="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

OUT="$NSIS_DIR/latest.json"
cat > "$OUT" <<EOF
{
  "version": "$VERSION",
  "notes": "Release $TAG",
  "pub_date": "$PUB_DATE",
  "platforms": {
    "windows-x86_64": {
      "signature": "$SIGNATURE",
      "url": "$DOWNLOAD_URL"
    }
  }
}
EOF

echo ">> Wrote manifest: $OUT"
echo ">> Download URL:   $DOWNLOAD_URL"
echo "--------------------------------------------------------------------"
cat "$OUT"
echo "--------------------------------------------------------------------"

# --- 4. Publish -------------------------------------------------------------
GH_CMD=(gh release create "$TAG" "$EXE" "$OUT" --repo "$REPO" --title "$TAG" --notes "Release $TAG")

if [ "$PUBLISH" -eq 1 ]; then
  if ! command -v gh >/dev/null 2>&1; then
    echo "ERROR: gh CLI not installed. Install with: winget install --id GitHub.cli" >&2
    echo "       Then run: gh auth login" >&2
    exit 1
  fi
  echo ">> Publishing release..."
  "${GH_CMD[@]}"
  echo ">> Done. Release $TAG created."
else
  echo ">> Dry run complete. To publish, either:"
  echo "   A) Run: bash scripts/release.sh --publish   (needs gh CLI)"
  echo "   B) Manually create release $TAG at:"
  echo "      https://github.com/$REPO/releases/new?tag=$TAG"
  echo "      and upload these two files as assets:"
  echo "        - $EXE"
  echo "        - $OUT"
fi
