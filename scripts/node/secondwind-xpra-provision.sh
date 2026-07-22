#!/bin/sh
# Generates the app-session password on every boot (runtime dir is tmpfs).
# The agent shares it with the paired host over mTLS only.

set -eu

PASSWORD_FILE="${SECONDWIND_XPRA_PASSWORD_FILE:-/run/secondwind/xpra.pass}"

mkdir -p "$(dirname "$PASSWORD_FILE")"
umask 077
head -c 24 /dev/urandom | od -An -tx1 | tr -d ' \n' > "$PASSWORD_FILE"
chown secondwind:secondwind "$PASSWORD_FILE" 2>/dev/null || true
chmod 600 "$PASSWORD_FILE"

echo "secondwind-xpra-provision: session password ready"
