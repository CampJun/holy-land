#!/usr/bin/env bash
# Build holyland for Miyoo Mini Plus + assemble the Onion app folder.
#
# Output: target/onion/HolyLand/  — copy this folder to /mnt/SDCARD/App/
# on the Miyoo SD card.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET=armv7-unknown-linux-gnueabihf

cd "$ROOT"

echo "=== cross-building holyland ==="
podman run --rm \
    -e RUSTFLAGS='-L/onion-libs/lib -Clink-arg=-Wl,-rpath-link,/onion-libs/lib -Clink-arg=-Wl,--allow-shlib-undefined -Clink-arg=-Wl,--unresolved-symbols=ignore-in-shared-libs' \
    -v "$ROOT:/src:Z" \
    -v "$ROOT/cross/onion-libs:/onion-libs:Z,ro" \
    -v holyland-cargo-registry:/opt/cargo/registry \
    -v holyland-cargo-git:/opt/cargo/git \
    miyoomini-rust \
    cargo build --release --no-default-features --target "$TARGET"

OUT="$ROOT/target/onion/HolyLand"
echo "=== assembling $OUT ==="
rm -rf "$OUT"
mkdir -p "$OUT"

cp "$ROOT/target/$TARGET/release/holyland" "$OUT/"
cp -r "$ROOT/assets" "$OUT/"
install -m 755 "$ROOT/cross/onion-pkg/launch.sh" "$OUT/"
cp "$ROOT/cross/onion-pkg/config.json" "$OUT/"

echo "=== done ==="
echo "Folder ready: $OUT"
echo "Copy to: /mnt/SDCARD/App/HolyLand/  on the Miyoo SD card."
ls -la "$OUT"
