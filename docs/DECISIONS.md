# Decision log (ADR)

Every non-trivial decision, with date and reasoning. Architectural decisions locked in `SECONDWIND_PLAN.md` §3 are not repeated here — this log records choices made *while building*, especially anything the plan left open.

Format: newest first. Each entry has a date, the decision, the reasoning, and status.

---

## 2026-07-21 — v0.1 implementation choices

Decisions made while completing the v0.1 feature set (companion pairing/screen, kiosk, auto-connect, image automation).

- **Certificate store moved to `sw-core`.** Both peers persist self-signed certs and exchange fingerprints; shared crypto belongs in the shared crate (plan §5). *Status: accepted.*
- **Pairing PIN/QR are never served over the network API.** `GET /v1/pairing` originally returned the full offer; any LAN peer could have read the PIN and defeated the proximity proof. The PIN/QR now travel only agent → kiosk state file → physical screen. *Status: accepted (security fix).*
- **Agent ↔ kiosk contract is an atomically-written JSON state file** (`/run/secondwind/kiosk.json`, path from systemd), not a local API. Keeps the network surface mTLS-only, gives the kiosk the PIN/QR without network exposure, and survives either process restarting. *Status: accepted.*
- **v0.1 kiosk UI is a fullscreen terminal UI** (cage → foot → `sw-kiosk`, unicode QR). Minimal dependencies on a no-desktop node; a richer renderer can replace it later without changing the state-file contract. *Status: accepted for v0.1.*
- **Companion → node HTTP uses fingerprint-pinned TLS, never WebPKI.** Pre-pairing the pin comes from mDNS TXT/QR; post-pairing always from persisted per-UUID trust. `ureq` with a custom rustls verifier. *Status: accepted.*
- **Inner stream pairing (Moonlight ↔ Apollo) is automated with a one-shot PIN**: companion generates it, arms it on Apollo's localhost API (random SecondWind-owned dashboard credentials), and forwards it inside the mTLS screen-connect; the kiosk runs the client's pair step once, then streams. Consumed PINs are never retried. *Status: accepted; exact Apollo endpoint/config keys to verify on first hardware test.*
- **The agent uses the requesting peer's address as the stream target** (`ConnectInfo` on `POST /v1/screen`) instead of any configured host address — zero config, correct across cable/switch/Wi-Fi. *Status: accepted.*
- **Screen defaults to always-on when a node is paired**, so link-up auto-connect needs zero clicks (plan law #4). Per-node opt-out exists in config for a later UI. *Status: accepted.*
- **Auto-connect presence is debounced (3 missed scans)** to tolerate the USB-hub Ethernet adapter disappearing briefly (a known dev-host edge case, generalized). *Status: accepted.*
- **v0.1 agent binds all interfaces on an ephemeral port** (image env), relying on mTLS + pairing for access control. Binding to the link interface only (plan §9) is deferred until interface selection exists. *Status: accepted compromise — revisit.*
- **v0.1 is not tagged until the first physical host/node test passes.** The plan's acceptance is hardware-observable; tagging on code-complete alone would misrepresent it. *Status: accepted.*

## 2026-07-22 - v0.1 scaffold choices

v0.1 implementation has started after Phase 0 manual proof passed. The first slice creates the locked monorepo shape and shared Rust crate boundaries without implementing future-phase features.

- **License: MIT.** Reasoning: the product plan requires MIT or Apache-2.0; MIT is short, permissive, and compatible with keeping upstream GPL tools as separate invoked processes rather than linked libraries. *Status: accepted.*
- **Workspace members start with `sw-core`, `sw-agent`, and `sw-launcher`.** Reasoning: this gives shared protocol/config types, a node daemon boundary, and the planned host launch-helper boundary while keeping v0.1 focused on screen/pairing/auto-connect. *Status: accepted.*
- **Companion folder is scaffolded, not a full Tauri app yet.** Reasoning: the companion must eventually be Tauri, but adding full Tauri dependencies before the shared API settles would create churn. The folder documents the boundary while the first slice focuses on buildable Rust core crates. *Status: accepted for first v0.1 slice.*
- **No hardcoded hardware/network/display defaults in source.** Reasoning: Phase 0 proved render-device order is machine-dependent; source types accept detected/configured values rather than baking in model names, IPs, paths, codecs, resolutions, or drive letters. *Status: accepted.*

## 2026-07-21 — Phase 0 documentation choices

Phase 0 is docs-only (a manual proof). No product code was written. The following choices were made while writing `FIRST-SETUP.md`; each is a *manual-proof convenience*, not a product commitment — Phase 1 will re-decide how the image automates them.

- **Moonlight installed from the official Cloudsmith apt repo** (Flatpak as fallback). Reasoning: leaner than Flatpak and closer to what the eventual Debian image will bundle; Flatpak kept as a fallback for restricted networks. *Status: accepted for Phase 0.*
- **VA-API decode driver chosen by detected GPU generation**, not hardcoded: `intel-media-va-driver-non-free` for Intel Gen8+ (2014+), `i965-va-driver` for older Intel (Haswell and earlier), `mesa-va-drivers` for AMD. Reasoning: honours plan §2 "detect, never assume"; the dev node's HD 4600 (Haswell) specifically needs the older `i965` driver, which a single hardcoded choice would get wrong. *Status: accepted.*
- **`cage` used as the kiosk compositor even in the manual proof.** Reasoning: a no-desktop Debian has no way to display a GUI app otherwise, and it matches the locked kiosk decision (plan §3). *Status: accepted.*
- **Partitioning via the installer's "Guided – use the largest continuous free space"** rather than manual partitioning. Reasoning: safest guided path that provably uses only unallocated space and preserves the existing OS; lowest risk for a beginner. *Status: accepted.*
- **`GRUB_DISABLE_OS_PROBER=false` set post-install** so the existing OS appears in GRUB without changing the Debian-default entry. Reasoning: recent Debian hides other OSes by default; this restores the dual-boot menu the plan requires while keeping the node entry as default. *Status: accepted.*
- **First proof link defaults to "same network (DHCP)"**, with direct-cable + static IPs documented as the recommended daily setup. Reasoning: fastest path to the Phase 0 acceptance; direct cable is a docs recommendation, never a code assumption (plan §2). *Status: accepted.*
- **Repo layout:** treated the working directory `code/` as the monorepo root and created only `docs/` for Phase 0 (no Cargo workspace, crates, installer, or image config yet). Reasoning: plan §7 says build strictly in phase order and Phase 0 is docs-only. *Status: accepted; revisit at v0.1 start.*
- **`PROFILE-dev-machine.md` copied into `docs/`** to match the §5 layout and make the repo self-contained. The parent-level source copy the developer supplied is left untouched. *Status: accepted.*
