#!/bin/sh
# First-boot disk provisioning.
#
# If the installer created a data partition labeled SECONDWIND_DATA and no
# disk.env exists yet, generate the node's iSCSI identity + CHAP secret.
# Runs once (the unit is guarded by ConditionPathExists=!.../disk.env).

set -eu

ENV_FILE="/etc/secondwind/disk.env"
DATA_DEVICE="$(readlink -f /dev/disk/by-label/SECONDWIND_DATA 2>/dev/null || true)"

if [ -z "$DATA_DEVICE" ] || [ ! -b "$DATA_DEVICE" ]; then
    echo "secondwind-disk-provision: no SECONDWIND_DATA partition; disk feature stays off"
    exit 0
fi

random_hex() {
    head -c "$1" /dev/urandom | od -An -tx1 | tr -d ' \n'
}

SUFFIX="$(random_hex 4)"
SECRET="$(random_hex 16)"

umask 077
mkdir -p /etc/secondwind
cat > "$ENV_FILE" <<EOF
# Generated on first boot by secondwind-disk-provision. Do not edit.
SECONDWIND_DISK_DEVICE=$DATA_DEVICE
SECONDWIND_DISK_IQN=iqn.2026-07.app.secondwind:node-$SUFFIX
SECONDWIND_DISK_PORT=3260
SECONDWIND_DISK_CHAP_USER=secondwind-$SUFFIX
SECONDWIND_DISK_CHAP_SECRET=$SECRET
EOF
# The agent (secondwind user) must read it to share with the paired host.
chgrp secondwind "$ENV_FILE" 2>/dev/null || true
chmod 640 "$ENV_FILE"

echo "secondwind-disk-provision: provisioned $DATA_DEVICE"
