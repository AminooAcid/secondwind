#!/bin/sh
# Builds the SecondWind node ISO.
#
# Run on Debian (or a Debian container) with: live-build, curl, and a Rust
# toolchain able to build x86_64-unknown-linux-gnu.
#
#   cd node-image/live-build
#   sudo ./build.sh
#
# Result: secondwind-node-*.iso in this directory.
#
# Env overrides:
#   SECONDWIND_DEBIAN_DIST   Debian codename for the image (default: stable)
#   SECONDWIND_SKIP_CARGO=1  reuse already-built binaries

set -e

HERE="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$HERE/../.." && pwd)"
DIST="${SECONDWIND_DEBIAN_DIST:-stable}"
TARGET="x86_64-unknown-linux-gnu"

echo "==> 1/5 building SecondWind node binaries ($TARGET, release)"
if [ "${SECONDWIND_SKIP_CARGO:-0}" != "1" ]; then
    (cd "$REPO_ROOT" && cargo build --release --target "$TARGET" -p sw-agent -p sw-kiosk)
fi
AGENT_BIN="$REPO_ROOT/target/$TARGET/release/sw-agent"
KIOSK_BIN="$REPO_ROOT/target/$TARGET/release/sw-kiosk"
for bin in "$AGENT_BIN" "$KIOSK_BIN"; do
    [ -x "$bin" ] || { echo "missing binary: $bin" >&2; exit 1; }
done

echo "==> 2/5 staging binaries and systemd units into the image"
mkdir -p "$HERE/config/includes.chroot/usr/local/bin"
cp "$AGENT_BIN" "$KIOSK_BIN" "$HERE/config/includes.chroot/usr/local/bin/"
chmod 0755 "$HERE/config/includes.chroot/usr/local/bin/"*

mkdir -p "$HERE/config/includes.chroot/etc/systemd/system"
cp "$REPO_ROOT/node-image/systemd/sw-agent.service" \
   "$REPO_ROOT/node-image/systemd/sw-kiosk.service" \
   "$REPO_ROOT/node-image/systemd/secondwind-disk.service" \
   "$REPO_ROOT/node-image/systemd/secondwind-disk-provision.service" \
   "$REPO_ROOT/node-image/systemd/sw-xpra.service" \
   "$REPO_ROOT/node-image/systemd/sw-xpra-provision.service" \
   "$REPO_ROOT/node-image/systemd/secondwind-share.service" \
   "$REPO_ROOT/node-image/systemd/secondwind-usbipd.service" \
   "$HERE/config/includes.chroot/etc/systemd/system/"

mkdir -p "$HERE/config/includes.chroot/usr/local/lib/secondwind"
cp "$REPO_ROOT/scripts/node/secondwind-disk.sh" \
   "$REPO_ROOT/scripts/node/secondwind-disk-provision.sh" \
   "$REPO_ROOT/scripts/node/secondwind-share.sh" \
   "$REPO_ROOT/scripts/node/secondwind-run-synced.sh" \
   "$REPO_ROOT/scripts/node/secondwind-xpra-provision.sh" \
   "$REPO_ROOT/scripts/node/secondwind-usb.sh" \
   "$HERE/config/includes.chroot/usr/local/lib/secondwind/"
chmod 0755 "$HERE/config/includes.chroot/usr/local/lib/secondwind/"*.sh

echo "==> 3/5 preparing the Moonlight apt repository for $DIST"
sed "s/@DIST@/$DIST/" "$HERE/config/archives/moonlight.list.chroot.in" \
    > "$HERE/config/archives/moonlight.list.chroot"
# Signing key; the expected fingerprint is pinned in docs/UPSTREAM.md.
curl -fsSL \
    "https://dl.cloudsmith.io/public/moonlight-game-streaming/moonlight-qt/gpg.key" \
    > "$HERE/config/archives/moonlight.key.chroot"

echo "==> 4/5 live-build config for $DIST"
cd "$HERE"
lb clean
SECONDWIND_DEBIAN_DIST="$DIST" lb config

echo "==> 5/5 building the ISO (this takes a while)"
lb build

echo "Done. ISO:"
ls -1 "$HERE"/*.iso
