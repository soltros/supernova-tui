#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT_DIR/Cargo.toml" | head -n1)"
ARCH="$(uname -m)"
OUT_DIR="$ROOT_DIR/dist"

cd "$ROOT_DIR"
cargo build --release --offline

mkdir -p "$OUT_DIR"
BIN="$ROOT_DIR/target/release/supernova-tui"

# Generic binary tarball
TAR_STAGE="$(mktemp -d)"
cp "$BIN" "$TAR_STAGE/supernova-tui"
cp "$ROOT_DIR/README.md" "$TAR_STAGE/README.md"
tar -czf "$OUT_DIR/supernova-tui-linux-${ARCH}.tar.gz" -C "$TAR_STAGE" supernova-tui README.md
rm -rf "$TAR_STAGE"

# Raw generic binary
cp "$BIN" "$OUT_DIR/supernova-tui-linux-${ARCH}"

# .deb
if command -v dpkg-deb >/dev/null 2>&1; then
  bash "$ROOT_DIR/packaging/deb/build-deb.sh" "$BIN" "$OUT_DIR"
fi

# .rpm
if command -v rpmbuild >/dev/null 2>&1; then
  RPM_TOP="$(mktemp -d)"
  mkdir -p "$RPM_TOP"/{BUILD,RPMS,SOURCES,SPECS,SRPMS}
  cp "$BIN" "$RPM_TOP/SOURCES/supernova-tui"
  cp "$ROOT_DIR/README.md" "$RPM_TOP/SOURCES/README.md"
  rpmbuild --define "_topdir $RPM_TOP" -bb "$ROOT_DIR/packaging/rpm/supernova-tui.spec"
  find "$RPM_TOP/RPMS" -name '*.rpm' -exec cp {} "$OUT_DIR/" \;
  rm -rf "$RPM_TOP"
fi

(cd "$OUT_DIR" && sha256sum supernova-tui* > SHA256SUMS.txt)
ls -lh "$OUT_DIR"
