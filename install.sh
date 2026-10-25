#!/bin/sh
# Install parcel: curl -fsSL https://raw.githubusercontent.com/lubabs770/parcel/main/install.sh | sh
set -eu
BIN_DIR=${BIN_DIR:-$HOME/.local/bin}
VERSION=${VERSION:-latest}
case "$(uname -s)-$(uname -m)" in
  Darwin-arm64) target=aarch64-apple-darwin ;;
  Darwin-x86_64) target=x86_64-apple-darwin ;;
  Linux-x86_64) target=x86_64-unknown-linux-musl ;;
  Linux-aarch64 | Linux-arm64) target=aarch64-unknown-linux-musl ;;
  *) echo "parcel: no prebuilt binary for $(uname -s)-$(uname -m); try: cargo install --git https://github.com/lubabs770/parcel" >&2; exit 1 ;;
esac
if [ "$VERSION" = latest ]; then
  base="https://github.com/lubabs770/parcel/releases/latest/download"
else
  base="https://github.com/lubabs770/parcel/releases/download/$VERSION"
fi
asset="parcel-$target.tar.gz"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
curl -fsSL "$base/$asset" -o "$tmp/$asset"
curl -fsSL "$base/checksums.txt" -o "$tmp/checksums.txt"
# Verify before touching anything installed.
want=$(grep " $asset\$" "$tmp/checksums.txt" | cut -d' ' -f1)
if command -v sha256sum >/dev/null 2>&1; then got=$(sha256sum "$tmp/$asset" | cut -d' ' -f1)
else got=$(shasum -a 256 "$tmp/$asset" | cut -d' ' -f1); fi
[ -n "$want" ] && [ "$want" = "$got" ] || { echo "parcel: checksum mismatch for $asset" >&2; exit 1; }
tar -xzf "$tmp/$asset" -C "$tmp"
"$tmp/parcel" --version >/dev/null || { echo "parcel: downloaded binary doesn't run here" >&2; exit 1; }
mkdir -p "$BIN_DIR"
# Install via a temp name + rename, so a running parcel is never half-overwritten.
install -m 755 "$tmp/parcel" "$BIN_DIR/.parcel.new" && mv -f "$BIN_DIR/.parcel.new" "$BIN_DIR/parcel"
echo "installed $("$BIN_DIR/parcel" --version) to $BIN_DIR/parcel"
command -v himalaya >/dev/null 2>&1 || echo "note: parcel drives an email CLI; the default is himalaya (https://github.com/pimalaya/himalaya)"
