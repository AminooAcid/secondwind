#!/bin/sh
# SecondWind node installer / updater.
#
# Turns a minimal Debian install into a SecondWind node, pulling everything
# from Git and GitHub Releases. Running it again later updates the node.
#
#   wget -qO /tmp/install-node.sh https://raw.githubusercontent.com/AminooAcid/secondwind/main/scripts/node/install-node.sh
#   sudo sh /tmp/install-node.sh
#
# Overridables (all optional):
#   SECONDWIND_REPO_URL     git repository (default: official)
#   SECONDWIND_BRANCH       branch to install from (default: main)
#   SECONDWIND_INSTALL_DIR  checkout location (default: /opt/secondwind)
#   SECONDWIND_RELEASE_TAG  binaries release tag (default: node-rolling)
#   SECONDWIND_BINARIES_TARBALL  use a local sw-node tarball (offline/dev)
#   SECONDWIND_BUILD_FROM_SOURCE=1  compile binaries locally instead
#   SECONDWIND_ASSUME_YES=1 skip the confirmation prompt

set -eu

REPO_URL="${SECONDWIND_REPO_URL:-https://github.com/AminooAcid/secondwind}"
BRANCH="${SECONDWIND_BRANCH:-main}"
INSTALL_DIR="${SECONDWIND_INSTALL_DIR:-/opt/secondwind}"
RELEASE_TAG="${SECONDWIND_RELEASE_TAG:-node-rolling}"
BINARIES_ASSET="sw-node-linux-x86_64.tar.gz"
LIB_DIR="/usr/local/lib/secondwind"
BIN_DIR="/usr/local/bin"

say() { printf '\n==> %s\n' "$1"; }
die() { printf 'secondwind: %s\n' "$1" >&2; exit 1; }

# ---------------------------------------------------------------- checks --
[ "$(id -u)" = "0" ] || die "please run with sudo (sudo sh install-node.sh)"
command -v apt-get >/dev/null 2>&1 || die "this installer needs a Debian-based system"

CODENAME=""
if [ -r /etc/os-release ]; then
    # shellcheck disable=SC1091
    . /etc/os-release
    CODENAME="${VERSION_CODENAME:-}"
fi
[ -n "$CODENAME" ] || die "could not detect the Debian release codename"

if [ "${SECONDWIND_ASSUME_YES:-0}" != "1" ]; then
    printf '\nThis computer will become a SecondWind node:\n'
    printf '  - it boots straight into the SecondWind screen (no desktop)\n'
    printf '  - services for screen/disk/USB/app sharing are installed\n'
    printf '  - no existing files are deleted\n\n'
    printf 'Type "node" and press Enter to continue: '
    read -r answer
    [ "$answer" = "node" ] || die "cancelled"
fi

# ------------------------------------------------------------- get code --
say "1/7 installing base tools"
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y -qq git curl ca-certificates gnupg >/dev/null

say "2/7 getting SecondWind from Git ($BRANCH)"
if [ -d "$INSTALL_DIR/.git" ]; then
    git -C "$INSTALL_DIR" fetch --depth 1 origin "$BRANCH"
    git -C "$INSTALL_DIR" checkout -q "$BRANCH"
    git -C "$INSTALL_DIR" reset -q --hard "origin/$BRANCH"
else
    git clone --depth 1 --branch "$BRANCH" "$REPO_URL" "$INSTALL_DIR"
fi

# ------------------------------------------------------------- packages --
say "3/7 installing system packages (this can take a few minutes)"
# Add the Moonlight repo only if no source for it exists yet — a Phase 0
# manual setup (or an earlier run) may already provide one with its own
# keyring path, and apt refuses two sources with different Signed-By.
if grep -rqs "moonlight-game-streaming/moonlight-qt" \
    /etc/apt/sources.list /etc/apt/sources.list.d/ 2>/dev/null; then
    say "    (Moonlight package source already present — keeping it)"
else
    KEYRING=/usr/share/keyrings/moonlight.gpg
    curl -fsSL "https://dl.cloudsmith.io/public/moonlight-game-streaming/moonlight-qt/gpg.key" \
        | gpg --dearmor -o "$KEYRING"
    printf 'deb [signed-by=%s] https://dl.cloudsmith.io/public/moonlight-game-streaming/moonlight-qt/deb/debian %s main\n' \
        "$KEYRING" "$CODENAME" > /etc/apt/sources.list.d/secondwind-moonlight.list
fi
apt-get update -qq

# Same set the image installs; VA driver picked at runtime, both installed.
apt-get install -y -qq \
    cage foot moonlight-qt avahi-daemon \
    libva-utils i965-va-driver mesa-va-drivers va-driver-all \
    network-manager targetcli-fb xpra cifs-utils rsync usbip sudo docker.io \
    >/dev/null
apt-get install -y -qq intel-media-va-driver-non-free >/dev/null 2>&1 \
    || apt-get install -y -qq intel-media-va-driver >/dev/null 2>&1 \
    || true

# ------------------------------------------------------------- binaries --
say "4/7 installing SecondWind binaries"
if [ -n "${SECONDWIND_BINARIES_TARBALL:-}" ]; then
    [ -f "$SECONDWIND_BINARIES_TARBALL" ] \
        || die "SECONDWIND_BINARIES_TARBALL not found: $SECONDWIND_BINARIES_TARBALL"
    tar -xzf "$SECONDWIND_BINARIES_TARBALL" -C "$BIN_DIR" sw-agent sw-kiosk
    chmod 0755 "$BIN_DIR/sw-agent" "$BIN_DIR/sw-kiosk"
elif [ "${SECONDWIND_BUILD_FROM_SOURCE:-0}" = "1" ]; then
    command -v cargo >/dev/null 2>&1 || die "building from source needs Rust (rustup.rs)"
    (cd "$INSTALL_DIR" && cargo build --release -p sw-agent -p sw-kiosk)
    install -m 0755 "$INSTALL_DIR/target/release/sw-agent" "$BIN_DIR/sw-agent"
    install -m 0755 "$INSTALL_DIR/target/release/sw-kiosk" "$BIN_DIR/sw-kiosk"
else
    TMP_TAR="$(mktemp /tmp/secondwind-binaries.XXXXXX)"
    URL="$REPO_URL/releases/download/$RELEASE_TAG/$BINARIES_ASSET"
    curl -fL -o "$TMP_TAR" "$URL" \
        || die "could not download node binaries from $URL
(set SECONDWIND_BUILD_FROM_SOURCE=1 to compile locally instead)"
    tar -xzf "$TMP_TAR" -C "$BIN_DIR" sw-agent sw-kiosk
    chmod 0755 "$BIN_DIR/sw-agent" "$BIN_DIR/sw-kiosk"
    rm -f "$TMP_TAR"
fi

# --------------------------------------------------- users, files, units --
say "5/7 installing services and configuration"
sh "$INSTALL_DIR/node-image/live-build/config/hooks/normal/0100-secondwind-user.hook.chroot"

mkdir -p "$LIB_DIR"
for helper in secondwind-disk.sh secondwind-disk-provision.sh secondwind-share.sh \
    secondwind-run-synced.sh secondwind-xpra-provision.sh secondwind-usb.sh; do
    install -m 0755 "$INSTALL_DIR/scripts/node/$helper" "$LIB_DIR/$helper"
done

install -m 0644 \
    "$INSTALL_DIR/node-image/systemd/sw-agent.service" \
    "$INSTALL_DIR/node-image/systemd/sw-kiosk.service" \
    "$INSTALL_DIR/node-image/systemd/sw-xpra.service" \
    "$INSTALL_DIR/node-image/systemd/sw-xpra-provision.service" \
    "$INSTALL_DIR/node-image/systemd/secondwind-disk.service" \
    "$INSTALL_DIR/node-image/systemd/secondwind-disk-provision.service" \
    "$INSTALL_DIR/node-image/systemd/secondwind-share.service" \
    "$INSTALL_DIR/node-image/systemd/secondwind-usbipd.service" \
    /etc/systemd/system/

INCLUDES="$INSTALL_DIR/node-image/live-build/config/includes.chroot"
mkdir -p /etc/secondwind /etc/polkit-1/rules.d /etc/sudoers.d
# Config defaults: never overwrite files the node may have customized.
for config in sw-agent.env sw-kiosk.env sw-xpra.env sw-share.env apps.json jobs.json; do
    if [ ! -f "/etc/secondwind/$config" ]; then
        install -m 0644 "$INCLUDES/etc/secondwind/$config" "/etc/secondwind/$config"
    fi
done
install -m 0644 "$INCLUDES/etc/polkit-1/rules.d/50-secondwind-disk.rules" \
    /etc/polkit-1/rules.d/50-secondwind-disk.rules
install -m 0440 "$INCLUDES/etc/sudoers.d/secondwind-usb" /etc/sudoers.d/secondwind-usb

say "6/7 enabling services"
systemctl daemon-reload
sh "$INSTALL_DIR/node-image/live-build/config/hooks/normal/0200-enable-services.hook.chroot"
systemctl restart sw-agent.service 2>/dev/null || true

say "7/7 done"
printf '\nSecondWind node is installed (source: %s, branch: %s).\n' "$REPO_URL" "$BRANCH"
printf 'Reboot now to start the SecondWind screen:  sudo reboot\n'
printf 'To update later, run this installer again.\n'
