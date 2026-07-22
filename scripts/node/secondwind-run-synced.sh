#!/bin/sh
# Cache-and-sync app wrapper (v0.3).
#
#   secondwind-run-synced.sh <app-id> <profile-rel-path> <command...>
#
# Apps holding live databases (browser profiles) must not run them over the
# network share. This wrapper copies the profile from the host share to
# node tmpfs at session start, runs the app against the tmpfs copy via
# $HOME redirection, and syncs it back atomically on exit. Nothing of the
# user's persists on the node: the tmpfs copy dies with the session.
#
# Config via environment (image defaults, overridable):
#   SECONDWIND_SHARE_MOUNTPOINT  where the host share is mounted
#   SECONDWIND_SYNC_ROOT         tmpfs root for session profiles

set -eu

APP_ID="$1"
PROFILE_REL="$2"
shift 2

SHARE_ROOT="${SECONDWIND_SHARE_MOUNTPOINT:-/mnt/secondwind-host}"
SYNC_ROOT="${SECONDWIND_SYNC_ROOT:-/run/secondwind/app-profiles}"

MASTER_DIR="$SHARE_ROOT/app-profiles/$APP_ID"
SESSION_HOME="$SYNC_ROOT/$APP_ID"
LOCK_FILE="$SYNC_ROOT/$APP_ID.lock"

mkdir -p "$SYNC_ROOT"

# One session per app at a time: the profile is a live database.
exec 9> "$LOCK_FILE"
if ! flock -n 9; then
    echo "secondwind-run-synced: $APP_ID is already running" >&2
    exit 75
fi

# 1. Session start: copy master profile (if any) to tmpfs.
rm -rf "$SESSION_HOME"
mkdir -p "$SESSION_HOME/$(dirname "$PROFILE_REL")" 2>/dev/null || mkdir -p "$SESSION_HOME"
if [ -d "$MASTER_DIR/$PROFILE_REL" ]; then
    rsync -a "$MASTER_DIR/$PROFILE_REL/" "$SESSION_HOME/$PROFILE_REL/"
fi

sync_back() {
    # 3. Atomic sync-back: write to a sibling, then swap.
    if [ -d "$SESSION_HOME/$PROFILE_REL" ] && [ -d "$SHARE_ROOT" ]; then
        mkdir -p "$MASTER_DIR"
        STAGING="$MASTER_DIR/.$PROFILE_REL.staging"
        rm -rf "$STAGING"
        mkdir -p "$(dirname "$STAGING")"
        rsync -a "$SESSION_HOME/$PROFILE_REL/" "$STAGING/"
        if [ -d "$MASTER_DIR/$PROFILE_REL" ]; then
            OLD="$MASTER_DIR/.$PROFILE_REL.old"
            rm -rf "$OLD"
            mv "$MASTER_DIR/$PROFILE_REL" "$OLD"
            mv "$STAGING" "$MASTER_DIR/$PROFILE_REL"
            rm -rf "$OLD"
        else
            mv "$STAGING" "$MASTER_DIR/$PROFILE_REL"
        fi
    fi
    rm -rf "$SESSION_HOME"
}
trap sync_back EXIT INT TERM

# 2. Run the app with HOME pointed at the tmpfs session copy.
HOME="$SESSION_HOME" "$@"
