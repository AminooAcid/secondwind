#!/bin/sh
# Privileged USB bind/unbind wrapper (v0.4).
#
# The unprivileged agent may run exactly this script via a sudoers rule.
# Bus ids are re-validated here — defense in depth even though the agent
# validates before calling.

set -eu

VERB="${1:-}"
BUS_ID="${2:-}"

case "$BUS_ID" in
    ''|*[!0-9a-zA-Z.-]*)
        echo "secondwind-usb: invalid bus id" >&2
        exit 64
        ;;
esac

case "$VERB" in
    bind)
        modprobe usbip-host 2>/dev/null || true
        exec usbip bind -b "$BUS_ID"
        ;;
    unbind)
        exec usbip unbind -b "$BUS_ID"
        ;;
    *)
        echo "usage: $0 bind|unbind <bus-id>" >&2
        exit 64
        ;;
esac
