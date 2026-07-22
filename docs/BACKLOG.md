# Backlog — accepted ideas, deliberately not built yet

Items reviewed and judged *right direction, wrong moment*. Each records
why it waits and what unblocks it. The plan's rule stands: build in phase
order, small steps, working state — and right now the gate is the first
hardware validation (`docs/HARDWARE-VALIDATION.md`).

## After hardware validation

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
