# Changelog

All notable changes to SecondWind. Format loosely follows Keep a Changelog;
versions are tagged only after their phase acceptance passes on hardware.

## [Unreleased — git-based node delivery]

### Changed
- Primary node install is now Git-based: `scripts/node/install-node.sh`
  turns a stock Debian minimal install into a node (packages, Moonlight
  repo, services, prebuilt binaries from the `node-rolling` GitHub
  release) and updates it on re-run. Guide: `docs/NODE-SETUP.md`. The
  flashable ISO moves to the backlog as the polished 1.0 path.

### Added
- `release-node.yml`: CI builds `sw-agent`/`sw-kiosk` in a Debian
  bookworm container on every push to main and refreshes the rolling
  release the installer downloads.

## [Unreleased — architecture-review response]

### Added
- Per-node operation serialization in the companion: UI toggles,
  auto-connect, app launches, and USB actions on the same node can no
  longer interleave (first slice of a future reconciliation engine).
- Panel-mode detection on the node: DRM connector scan + EDID
  preferred-mode parsing (resolution and refresh), internal panels first —
  fills the v0.1 "match the node panel" spec gap.
- Plain-language explanations for every app-launch decision.
- Structured `tracing` in the agent (`SECONDWIND_LOG`) and per-feature
  health in `GET /v1/health` (screen/disk/apps/share/usb/jobs).
- Job runner: enforced timeouts, background reaper, idempotency keys
  (context menu resubmits no longer start duplicate containers).
- BUG-005 resolved: a regenerated certificate store under an established
  pairing now resets trust explicitly (re-pair) instead of silently
  breaking mTLS.
- Agent routes moved onto `spawn_blocking` so slow systemctl/docker/USB
  commands never occupy request threads.
- CI: `cargo fmt --check`, `clippy -D warnings` on both workspaces,
  RustSec audit, ShellCheck, PSScriptAnalyzer.

### Changed
- Deferred-not-rejected review items recorded in `docs/BACKLOG.md`
  (reconciliation engine, SQLite jobs, DPAPI, api.rs split, TS bindings);
  adaptive-streaming engine and placement scoring rejected with reasons
  in `docs/DECISIONS.md`.

## [Unreleased — audit fixes, round 2]

### Fixed
- Agent job table now evicts finished jobs FIFO (cap 50) instead of
  growing forever (BUG-010).
- Explorer-job path containment rewritten component-based: no more
  potential panic on Unicode case folding or trailing separators, and
  look-alike sibling folders no longer match (BUG-011).
- First-time share setup passes the account password to the elevated
  script via an owner-restricted, self-deleting file — elevated processes
  don't inherit the caller's environment, so the old env-var handoff
  produced an empty password (BUG-012).
- USB detach resolves the local port by the exact remote bus id, so two
  identical devices can no longer detach each other (BUG-014).
- Failed host-side disk attach now rolls the node's export back instead
  of leaving it exposed (BUG-015).
- App-session password file written atomically and owner-only (BUG-016).
- Kiosk clock uses in-process time instead of spawning `date` in the
  render loop (BUG-017).

### Known accepted risk
- Apollo dashboard credentials are set via its `--creds` argv (upstream
  offers no alternative); one-time local exposure documented in
  `docs/DECISIONS.md` (BUG-013).

## [Unreleased — v0.5 code-complete]

### Added
- Job offload: agent `GET`/`POST /v1/jobs` with Docker presets from the
  node's own preset file (convert/compress/download), hard input
  validation (no path traversal, http(s) URLs only), file jobs run with
  the network disabled, and jobs operate directly on the mounted share.
- Explorer context-menu registration ("Convert to MP4 on node",
  "Compress on node") driving the companion's headless `--job` mode,
  which requires inputs to live inside the SecondWind shared folder.
- Ambient idle screen on the kiosk: clock + memory stats, minute-level
  repaints.
- Host installer script (`installer/innosetup/secondwind.iss`) bundling
  the companion, glue scripts, Apollo, usbip-win2, and the xpra client,
  with the third-party manifest in `THIRD-PARTY.md`.
- CI now also runs companion tests and a Linux build of the node
  binaries.

### Changed
- README rewritten: feature matrix, quickstart, credits, known limits.
- `docs/UPSTREAM.md` now lists every upstream with license, invocation,
  and pin rules.

## [Unreleased — v0.4 code-complete]

### Added
- USB feature end-to-end: node `usbipd` + agent `GET`/`POST /v1/usb`
  (device list with bound state; bind/unbind through a sudoers-scoped,
  input-validated root wrapper), companion attach/detach via the bundled
  usbip-win2 client, per-device "Always attach" rules applied by
  auto-connect, and `docs/USB-SETUP.md` for the one-time driver-trust step.

## [Unreleased — v0.3 code-complete]

### Added
- App library with per-app policy (always on node / always local / ask) and
  fallback-to-local, editable in the companion UI; v1 catalog: Firefox,
  Chromium, VLC, LibreOffice, GIMP, PDF reader.
- `sw-launcher` decision engine, Wake-on-LAN magic packets, and the
  seamless-client attach spec (all unit-tested).
- Node app session (xpra, per-boot random password) with whitelisted
  launches via `GET`/`POST /v1/apps` (mTLS only).
- Host file share: dedicated `SecondWindShare` account + SMB share created
  by an elevated script; node mounts it via the polkit-scoped
  `secondwind-share` unit (`GET`/`POST /v1/share`).
- Cache-and-sync wrapper for live-profile apps (tmpfs session copy, atomic
  sync-back, single-instance lock).
- Agent detects interface MACs; the companion records wake targets at
  pairing and wakes powered-off nodes on launch.

### Fixed
- Flaky companion test: parallel tests shared a certificate temp dir.

## [Unreleased — v0.2 code-complete]

### Added
- Disk feature end-to-end: agent `GET`/`POST /v1/disk` (mTLS-gated), LIO
  export unit + first-boot provisioning on the node (per-node IQN, random
  CHAP secret, `SECONDWIND_DATA` partition from the installer recipe),
  polkit rule scoping the agent to the export unit.
- Windows attach/detach PowerShell scripts (first-use NTFS setup of the
  SecondWind disk, drive-letter assignment, flush-before-detach) driven by
  the companion, plus a Disk toggle in the UI.
- Auto-connect now brings the disk up after the screen and tears down in
  reverse order; on link loss the initiator session is flushed and cleaned
  locally.

## [Unreleased — v0.1 code-complete]

### Added
- Shared certificate store in `sw-core` (self-signed generation, SHA-256
  fingerprints, self-healing writes) used by node and host.
- Agent screen control API (`GET`/`POST /v1/screen`) with mTLS, using the
  requesting peer's address as the stream target.
- Agent → kiosk atomic JSON state file carrying pairing PIN/QR to the
  physical screen only.
- Node kiosk: pairing screen with unicode QR + PIN, paired idle screen,
  supervised streaming client (one-shot inner pairing, exponential restart
  backoff), dev escape hatch behind an env flag.
- Companion host identity/certificate store with UUID-keyed node trust.
- Companion pairing flow: fingerprint-pinned HTTPS client + PIN entry UI.
- Companion Apollo layer: detection, managed config block, random
  dashboard credentials, one-shot stream-PIN arming, service control.
- Companion Screen toggle end-to-end and link-up auto-connect with
  debounced mDNS presence and UI notifications.
- Node image automation: live-build tree + `build.sh` (binaries, systemd
  units, Moonlight repo, VA-API drivers, preseeded safe dual-boot
  installer).

### Fixed
- Pairing PIN and QR payload are no longer exposed over the network API.

### Security
- Mutual TLS enforced after pairing on every node connection; host → node
  connections pin the stored certificate fingerprint, never WebPKI.

## Phase 0 — 2026-07-21

- Manual proof documented: `docs/FIRST-SETUP.md` (generic beginner guide +
  device appendix pattern), `docs/DECISIONS.md`, `docs/COMPATIBILITY.md`,
  `docs/PHASE0-TROUBLESHOOTING.md`. No product code.
