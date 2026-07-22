# Changelog

All notable changes to SecondWind. Format loosely follows Keep a Changelog;
versions are tagged only after their phase acceptance passes on hardware.

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
