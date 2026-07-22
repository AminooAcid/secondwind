#!/bin/sh
# SecondWind node disk export (LIO iSCSI via targetcli).
#
# Driven by the secondwind-disk systemd unit with /etc/secondwind/disk.env:
#   SECONDWIND_DISK_DEVICE       block device of the designated data partition
#   SECONDWIND_DISK_IQN          target IQN for this node
#   SECONDWIND_DISK_PORT         portal port (default 3260)
#   SECONDWIND_DISK_CHAP_USER    CHAP username the host must present
#   SECONDWIND_DISK_CHAP_SECRET  CHAP secret the host must present
#
# Only the designated partition is ever exported. Nothing here names any
# other disk or partition.

set -eu

BACKSTORE_NAME="secondwind-data"

require_env() {
    for name in SECONDWIND_DISK_DEVICE SECONDWIND_DISK_IQN \
                SECONDWIND_DISK_CHAP_USER SECONDWIND_DISK_CHAP_SECRET; do
        eval "value=\${$name:-}"
        if [ -z "$value" ]; then
            echo "secondwind-disk: $name is not set (is the data disk provisioned?)" >&2
            exit 2
        fi
    done
}

up() {
    require_env
    [ -b "$SECONDWIND_DISK_DEVICE" ] || {
        echo "secondwind-disk: $SECONDWIND_DISK_DEVICE is not a block device" >&2
        exit 3
    }

    targetcli "/backstores/block create name=$BACKSTORE_NAME dev=$SECONDWIND_DISK_DEVICE" || true
    targetcli "/iscsi create $SECONDWIND_DISK_IQN" || true
    targetcli "/iscsi/$SECONDWIND_DISK_IQN/tpg1/luns create /backstores/block/$BACKSTORE_NAME" || true
    # CHAP-authenticated dynamic ACLs: any initiator that knows the secret
    # (only the paired host — it travels over mTLS) can attach.
    targetcli "/iscsi/$SECONDWIND_DISK_IQN/tpg1 set attribute authentication=1 generate_node_acls=1 cache_dynamic_acls=1 demo_mode_write_protect=0"
    targetcli "/iscsi/$SECONDWIND_DISK_IQN/tpg1 set auth userid=$SECONDWIND_DISK_CHAP_USER password=$SECONDWIND_DISK_CHAP_SECRET"
    echo "secondwind-disk: exported $SECONDWIND_DISK_DEVICE as $SECONDWIND_DISK_IQN"
}

down() {
    require_env
    targetcli "/iscsi delete $SECONDWIND_DISK_IQN" || true
    targetcli "/backstores/block delete $BACKSTORE_NAME" || true
    echo "secondwind-disk: unexported $SECONDWIND_DISK_IQN"
}

case "${1:-}" in
    up) up ;;
    down) down ;;
    *) echo "usage: $0 up|down" >&2; exit 64 ;;
esac
