# Backlog — accepted ideas, deliberately not built yet

Items reviewed and judged *right direction, wrong moment*. Each records
why it waits and what unblocks it. The plan's rule stands: build in phase
order, small steps, working state — and right now the gate is the first
hardware validation (`docs/HARDWARE-VALIDATION.md`).

## Apollo control layer — gentler, less destabilising (from first hardware run)

The screen-connect path drove Apollo 0.4.7-alpha into a wedged state on the
first host. The companion is now hardened against Apollo's startup timing
(service name, slow stop, API-bind lag, credential-reload restart), but the
approach is still heavy. Before the next streaming attempt:

- **Do not spawn a second `sunshine.exe` against a live service.** Setting
  credentials via `sunshine.exe --creds` while `ApolloService` runs means
  two sunshine processes touching shared state. Prefer setting creds while
  the service is stopped, or via Apollo's API if it exposes one.
- **Avoid restarts on the hot path.** Configure Apollo once at
  install/first-pair (elevated installer), so `connect_screen` only arms a
  PIN against an already-configured, already-running service — no restart,
  no config write, at screen-on time.
- **Detect and clear a stuck stream session** (Apollo kept trying to encode
  for a client that was gone) instead of restarting blindly.
- Consider requiring the companion to run elevated for the Screen feature,
  or a small elevated helper, rather than assuming Program Files is
  writable.
- Re-test the full streamed screen after a host reboot with a clean Apollo.

## Idle-RAM reduction (from first hardware run)

The MSI Haswell node idles at ~530 MB (Screen-only) to ~640 MB (all
services) on Debian 13 — over the plan's ~400 MB goal. Everything works;
this is footprint polish. Two fixes, in impact order:

- **Lazy-start the xpra Apps session** (~147 MB: xpra + Xvfb). Like Docker
  is now socket-activated, the agent should start `sw-xpra` on the first
  app launch and stop it when idle, instead of it running always-on. Apps
  is not an "always-on" feature — only Screen/Disk/USB are.
- **Trim the `cage` session's inherited daemons**: pipewire/wireplumber
  (no audio passthrough yet), ibus (no input method on a kiosk),
  xdg-desktop-portal, cups. Some respawn with the session; mask at the
  systemd level or start `cage` in a minimal environment.
- If still over after both, revise the ~400 MB target in the plan to match
  the real modern-Debian floor and record why.

## After hardware validation

- **ISO package parity with trixie findings**: the image config still
  assumes apt `moonlight-qt` (Moonlight's repo publishes nothing for
  trixie → Flatpak path needed) and Debian-archived `xpra` (gone in
  trixie → xpra.org repo needed). `install-node.sh` already handles both;
  port the same logic into the live-build hooks before the ISO returns.
- **Flashable all-in-one node ISO as the polished install path.** The
  live-build tree (`node-image/`) is complete and stays maintained; the
  primary path for now is the Git-based `install-node.sh` (owner
  decision, see DECISIONS 2026-07-22). Revisit for 1.0: the ISO removes
  the "install Debian yourself" step entirely.

- **Per-node reconciliation engine.** Grow `companion/src-tauri/src/node_ops.rs`
  (today: per-node serialization) into desired-state → observe → diff →
  apply → verify, with rollback and retry. Unblocked by: validated feature
  flows to reconcile against. First slice exists so all operations already
  funnel through one place.
- **SQLite-backed durable job queue** with progress, cancellation, restart
  recovery. Today: in-memory table with timeouts, background reaping,
  idempotency keys, FIFO eviction. Unblocked by: real job workloads on
  hardware showing which guarantees matter.
- **DPAPI / Windows Credential Manager for host-side secrets** (share
  password, Apollo credentials, session pass files) instead of
  profile-ACL files. Unblocked by: hardware pass (needs testing against a
  real elevated-setup flow).
- **Support bundle + bounded event journal** on top of the structured
  `tracing` output; automated corrective actions for known failures
  (e.g. detect a disabled Windows iSCSI service and offer the fix).
- **Split `sw-agent/src/api.rs` by feature and generate TypeScript
  bindings from the Rust protocol types** (`ts-rs` or similar); per-node/
  per-feature operation state in the companion UI instead of one busy
  flag. Mechanical churn — safe any time, valuable once the protocol
  stops moving.
- **Protocol-level fault-injection tests** (disconnects mid-operation,
  duplicated commands, delayed responses) and an ISO-build smoke job in
  CI. The ISO build needs a beefy cached runner; evaluate after the first
  manual ISO builds settle.

## Rejected (not backlog)

- **Adaptive streaming engine** (RTT/jitter/load measurement driving
  resolution/bitrate/codec switching with hysteresis). Moonlight + Apollo
  already negotiate codecs and adapt bitrate; duplicating that violates
  plan law #2 (integrate, never reimplement). Panel-mode detection *was*
  the real gap and is implemented. Revisit only if hardware validation
  shows upstream adaptation is insufficient — with measurements first.
- **Load/battery/temperature scoring for app placement.** Plan §6.4 locks
  the policy model (Always on node / Always local / Ask, plus fallback).
  Decisions now carry plain-language explanations; the policy semantics
  stay user-set and predictable.
