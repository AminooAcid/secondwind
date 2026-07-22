#!/bin/sh
# Mounts/unmounts the host's SecondWind share (CIFS), driven by the
# secondwind-share systemd unit with /run/secondwind/share.env:
#   SECONDWIND_SHARE_UNC       \\host-address\SecondWind
#   SECONDWIND_SHARE_USERNAME  dedicated share account (never a user login)
#   SECONDWIND_SHARE_PASSWORD  its secret
#
# Mountpoint from SECONDWIND_SHARE_MOUNTPOINT (default /mnt/secondwind-host).

set -eu

MOUNTPOINT="${SECONDWIND_SHARE_MOUNTPOINT:-/mnt/secondwind-host}"

up() {
    for name in SECONDWIND_SHARE_UNC SECONDWIND_SHARE_USERNAME SECONDWIND_SHARE_PASSWORD; do
        eval "value=\${$name:-}"
        [ -n "$value" ] || { echo "secondwind-share: $name is not set" >&2; exit 2; }
    done

    mkdir -p "$MOUNTPOINT"
    if mountpoint -q "$MOUNTPOINT"; then
        exit 0
    fi

    # Kernel CIFS with a credentials file kept in the private runtime dir.
    CRED_FILE="$(mktemp /run/secondwind/share-cred.XXXXXX)"
    trap 'rm -f "$CRED_FILE"' EXIT
    printf 'username=%s\npassword=%s\n' \
        "$SECONDWIND_SHARE_USERNAME" "$SECONDWIND_SHARE_PASSWORD" > "$CRED_FILE"

    UNC_SLASHES="$(printf '%s' "$SECONDWIND_SHARE_UNC" | tr '\\' '/')"
    mount -t cifs "$UNC_SLASHES" "$MOUNTPOINT" \
        -o "credentials=$CRED_FILE,uid=secondwind,gid=secondwind,file_mode=0660,dir_mode=0770,iocharset=utf8,soft"
    echo "secondwind-share: mounted $UNC_SLASHES at $MOUNTPOINT"
}

down() {
    if mountpoint -q "$MOUNTPOINT"; then
        umount -l "$MOUNTPOINT"
    fi
    echo "secondwind-share: unmounted"
}

case "${1:-}" in
    up) up ;;
    down) down ;;
    *) echo "usage: $0 up|down" >&2; exit 64 ;;
esac
