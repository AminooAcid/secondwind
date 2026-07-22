# Decision log (ADR)

Every non-trivial decision, with date and reasoning. Architectural decisions locked in `SECONDWIND_PLAN.md` §3 are not repeated here — this log records choices made *while building*, especially anything the plan left open.

Format: newest first. Each entry has a date, the decision, the reasoning, and status.

---

## 2026-07-22 — node delivery switched to Git-based install (product-owner decision)

The plan's §3 "one flashable image" is **amended by the product owner**: the primary node delivery is now `scripts/node/install-node.sh` — install stock Debian minimal once (guide already exists from Phase 0), then one command clones/updates the repo, installs packages, and pulls prebuilt `sw-agent`/`sw-kiosk` from the `node-rolling` GitHub release (built by CI in a Debian bookworm container so glibc always matches Debian stable+). Re-running the script updates the node.

- Reasoning: during hardware validation an ISO per iteration is the slowest possible loop; the script path gives updates for free and is easier for everyone to try. The product law "user never touches a terminal" now applies *after* node setup for this phase; the two-command install is copy-paste with expected-result lines.
- The ISO (`node-image/`) is kept and demoted to `docs/BACKLOG.md` as the polished 1.0 path — the script and the image install the same units/configs from the same sources, so they cannot drift apart structurally.
- Config files under `/etc/secondwind/` are never overwritten on update (`install` only when missing); binaries and units always refresh. Disk feature activates only when a `SECONDWIND_DATA`-labeled partition exists.
*Status: accepted.*

## 2026-07-22 — architecture-review response (apply vs defer)

An external review proposed nine improvements. Triage rule: apply what fixes a defect or fills a spec gap now; defer big rewrites until after the first hardware validation; reject what duplicates upstream or reopens locked decisions. Deferred items live in `docs/BACKLOG.md` with unblock conditions.

- **Applied — per-node operation serialization** (`companion/src-tauri/src/node_ops.rs`): every state-changing operation (UI toggles, auto-connect, launches, USB) runs under that node's lock. This is the deliberate first slice of the proposed reconciliation engine; the full desired-state reconciler is deferred, not rejected. *Status: accepted.*
- **Applied — panel-mode detection** (DRM connectors + EDID preferred-mode parsing, internal panels first). This was a real v0.1 spec gap ("virtual display matched to the node panel"). The proposed adaptive-streaming telemetry engine is **rejected**: Moonlight/Apollo already negotiate and adapt; plan law #2 forbids reimplementing upstream. *Status: accepted / rejected respectively.*
- **Applied — decision explanations** (`sw_launcher::explain`): every launch returns one plain sentence for the user. Load/battery **scoring is rejected**: plan §6.4's policy model is locked and predictable-by-design. *Status: accepted / rejected respectively.*
- **Applied — structured diagnostics**: `tracing` with structured fields in the agent (level via `SECONDWIND_LOG`), per-feature health in `GET /v1/health`. Support bundle + event journal deferred. *Status: accepted.*
- **Applied — job runner hardening**: enforced timeouts (`SECONDWIND_JOB_TIMEOUT_SECS`, default 1 h), a background reaper thread, and idempotency keys. SQLite durability deferred until real workloads justify it. *Status: accepted.*
- **Applied — BUG-005 fix**: the identity stores the node's own certificate fingerprint; a changed certificate under an established pairing triggers an explicit trust reset (back to the pairing screen, logged loudly) instead of silently breaking mTLS. DPAPI for host secrets deferred to the hardware pass. *Status: accepted.*
- **Applied — `spawn_blocking`** around every agent route body (they all run filesystem/subprocess work). *Status: accepted.*
- **Applied — CI hardening**: `cargo fmt --check`, `clippy -D warnings` (both workspaces), RustSec dependency audit, ShellCheck (errors fail), PSScriptAnalyzer (errors fail). ISO smoke build + fault-injection tests deferred. *Status: accepted.*
- **Deferred — `api.rs` split + TypeScript bindings + UI state refactor**: mechanical churn with no behavior change; scheduled after the protocol stops moving (post-validation). *Status: deferred.*

## 2026-07-22 — audit-response choices (monitoring agent findings)

The parallel read-only monitoring agent's `BUG_TRACKER.md` findings BUG-010…017 were triaged; all but one were fixed the same day (job-history eviction, component-based path containment, password-file handoff to the elevated share setup, exact bus-id USB detach matching, disk export rollback, restricted session-pass file, in-process kiosk clock).

- **BUG-013 (Apollo credentials on argv) is accepted for now, not fixed.** Upstream's CLI only takes `--creds <user> <pass>` — there is no stdin/env/config alternative to set dashboard credentials non-interactively. Exposure is a sub-second, one-time, first-setup window on the local machine, and the credentials guard only Apollo's localhost dashboard (which SecondWind owns anyway). Revisit at the hardware pass: if the bundled Apollo version offers a safer path, switch. *Status: accepted risk — tracked in BUG_TRACKER.md.*
- **`BUG_TRACKER.md` is owned by the monitoring agent**; the builder never edits it — statuses there are updated by the monitor on its own audit passes. *Status: accepted.*

## 2026-07-21 — v0.5 jobs + polish choices

- **Job presets live on the node** (`/etc/secondwind/jobs.json`), mirroring the app-whitelist pattern: the host names a `preset_id` + input, never an image or command. Share-path inputs are traversal-checked on the node; file jobs run with `--network none`. *Status: accepted.*
- **Context-menu jobs go through the companion's headless `--job` mode** and only accept files inside the SecondWind shared folder — that is what makes them zero-copy on the node. Registration is per-user (HKCU), no elevation. *Status: accepted.*
- **outrun stays documentation-only** (plan §6.5): advanced CLI users can install it themselves; the v1 UI surfaces Docker presets only. *Status: accepted.*
- **Ambient idle screen keeps to clock + memory line.** More stats mean more wakeups on an 8 GB-class node; the ≤400 MB idle target wins. *Status: accepted.*
- **All phases through v0.5 are code-complete but untagged** until each phase's acceptance passes on the first physical pair; the consolidated hardware checklist lives in `docs/HARDWARE-VALIDATION.md`. *Status: accepted.*

## 2026-07-21 — v0.4 usb choices

- **USB bind/unbind privilege goes through one sudoers-scoped wrapper script**, mirroring the polkit-per-unit pattern used for disk/share; bus ids are validated against a conservative charset in the agent *and* in the wrapper. *Status: accepted.*
- **Auto-attach rules match vendor:product, not bus id.** Users think "my flash drive", not "port 1-1.4"; matching survives replugging into a different port. *Status: accepted.*
- **Host detach resolves the local port from `usbip port` output** by vendor:product; parser unit-tested, exact output format to verify against the bundled usbip-win2 release on hardware. *Status: accepted.*

## 2026-07-21 — v0.3 apps choices

- **One persistent node app session (xpra) instead of per-app sessions.** Single supervised unit, one attach from the host, `xpra control start` per launch; per-boot random password shared over mTLS only. *Status: accepted; `xpra control` flags to verify on hardware.*
- **Launch requests carry an `app_id`, never a command line.** The node resolves it against its own catalog file; a compromised host session cannot execute arbitrary commands via this route. *Status: accepted.*
- **Host→node file transport is SMB (Windows built-in server) behind the agent's `/v1/share` abstraction.** A dedicated `SecondWindShare` local account with a random password is created by one elevated script; the user's own Windows credentials never leave the host. The route abstraction keeps SSHFS possible later (plan §4). *Status: accepted.*
- **Cache-and-sync is a generic wrapper script** (`app-id`, `profile-path`, `command`), config-driven from the catalog's `synced_profile`, with `$HOME` redirection to tmpfs, flock single-instance, and staged atomic sync-back. *Status: accepted.*
- **Wake targets are all detected MACs, learned at pairing over mTLS.** Magic packets go to every stored MAC (harmless duplicates); wake-wait pins stored trust, not fresh discovery data. *Status: accepted.*

## 2026-07-21 — v0.2 disk choices

- **The exported LUN is CHAP-protected with node-generated credentials shared over mTLS.** iSCSI itself has no TLS; a bare LUN on the LAN would bypass the pairing trust model. CHAP secrets are random per node, generated at first boot, and only ever travel inside the paired `exposed` response. *Status: accepted.*
- **The agent controls the export through a dedicated systemd unit + a polkit rule scoped to exactly that unit**, instead of running targetcli as root itself. Smallest privilege surface; the unit's env file (root-owned, group-readable by the agent) is the single description of what may be exported. *Status: accepted.*
- **The data partition is created by the installer recipe with label `SECONDWIND_DATA` and formatted NTFS by the host on first attach.** The Windows initiator is the natural place to lay down the filesystem Windows will use; the first-attach script initializes only the disk belonging to the new iSCSI session. *Status: accepted; recipe behavior with "biggest free space" to verify on hardware.*
- **Teardown order is disk-flush before anything else**, including on link loss (local initiator cleanup with the last-known IQN when the node is unreachable). *Status: accepted.*
- **Windows-side attach/detach lives in two bundled PowerShell scripts** invoked by the companion (`scripts/windows/`), not in Rust P/Invoke — auditable, copy-paste debuggable, and matches the plan's "driven via PowerShell". *Status: accepted.*

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
