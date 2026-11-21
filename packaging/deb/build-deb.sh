#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT_DIR/Cargo.toml" | head -n1)"
ARCH="$(dpkg --print-architecture 2>/dev/null || echo amd64)"
BIN_PATH="${1:-$ROOT_DIR/target/release/supernova-tui}"
OUT_DIR="${2:-$ROOT_DIR/dist}"

if [[ ! -f "$BIN_PATH" ]]; then
  echo "Binary not found at $BIN_PATH" >&2
  exit 1
fi

STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT

mkdir -p "$STAGE/DEBIAN" "$STAGE/usr/bin" "$STAGE/usr/share/doc/supernova-tui"
chmod 755 "$STAGE" "$STAGE/DEBIAN" "$STAGE/usr" "$STAGE/usr/bin" "$STAGE/usr/share" "$STAGE/usr/share/doc" "$STAGE/usr/share/doc/supernova-tui"
install -m 755 "$BIN_PATH" "$STAGE/usr/bin/supernova-tui"
install -m 644 "$ROOT_DIR/README.md" "$STAGE/usr/share/doc/supernova-tui/README.md"

cat > "$STAGE/DEBIAN/control" <<EOF
Package: supernova-tui
Version: $VERSION
Section: sound
Priority: optional
Architecture: $ARCH
Depends: mpv
Maintainer: Derrik Diener <soltros@users.noreply.github.com>
Description: Ratatui TUI client for the Supernova music server
 A terminal user interface client for Supernova supporting library browsing,
 playlists, hearts, podcasts, internet radio, LRCLib synced lyrics, and Last.fm.
EOF
chmod 644 "$STAGE/DEBIAN/control"

mkdir -p "$OUT_DIR"
dpkg-deb --build --root-owner-group "$STAGE" "$OUT_DIR/supernova-tui_${VERSION}_${ARCH}.deb"
