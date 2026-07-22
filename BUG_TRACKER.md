# SecondWind Codebase Bug Tracker & Audit Log

> **DOCUMENT PURPOSE**  
> This document is created and maintained by the **Monitoring & Auditing Agent** during and following code modifications across Phases 0.1 – 0.5.  
> **Strict Operational Rule**: The monitoring agent **MUST NOT** edit or alter any source code (`.rs`, `.toml`, etc.). Its sole duty is to inspect code, discover bugs, document architectural vulnerabilities, and track code changes.

---

## 1. Executive Summary & Monitoring Overview

- **Target Repository**: `SecondWind` (`sw-core`, `sw-agent`, `sw-kiosk`, `sw-launcher`, `companion`)
- **Mode**: Read-Only Code Audit, Vulnerability Tracking & Phase 0.1 – 0.5 Full Codebase Inspection
- **Audit Sync Timestamp**: 2026-07-22 03:45 UTC
- **Active Workspace Status**:
  - **PHASE 0.1 THROUGH PHASE 0.5 FULLY AUDITED**: Checked all crates (`sw-core`, `sw-agent`, `sw-kiosk`, `sw-launcher`) and Tauri host companion (`companion/src-tauri`).
  - **WORKSPACE TEST STATUS**: 67/67 workspace core unit tests passing.
  - **Git Working Tree**: Clean working tree on `main`.
  - **Policy Compliance**: 100% compliant. Zero source code files modified by monitoring agent.

---

## 2. Identified Bugs & Vulnerabilities Index

| Bug ID | Severity | Category | Component | Brief Description | Status |
|---|---|---|---|---|---|
| **BUG-000** | Critical | Security / Leak | `sw-core::pairing` & `sw-agent::pairing_state` | Unauthenticated network API `/api/v1/pairing` leaked pairing PIN & QR payload | **RESOLVED** (Phase 0.1/0.5) |
| **BUG-001** | High | Concurrency / Race | `sw-agent::api` | Lock gap between PIN check and disk persistence allows state race condition | **RESOLVED** (Phase 0.5) |
| **BUG-002** | High | Data Integrity | `sw-agent::identity`, `sw-core::certificates`, `companion::host_state` | Direct `fs::write` used for identity, certificates & companion config without atomic write/rename | **RESOLVED** (Phase 0.5) |
| **BUG-003** | High | Security | `sw-core::certificates` & `companion::host_state` | Private key files written with default umask permissions (unrestricted readability) | **RESOLVED** (Phase 0.5) |
| **BUG-004** | Medium | Security / Auth | `sw-agent::api` | Missing PIN attempt rate-limiting or lockout on `/api/v1/pairing` endpoint | **RESOLVED** (Phase 0.5) |
| **BUG-005** | Medium | Resilience | `sw-core::certificates` | Partial store self-healing creates new cert/key set if key missing, invalidating host trust | **OPEN** |
| **BUG-006** | Low | Code Style / Lint | `sw-agent::api` | Clippy `collapsible_if` warning on `identity_store` persistence condition | **RESOLVED** (Phase 0.5) |
| **BUG-007** | High | Data Integrity | `crates/sw-core/src/kiosk.rs` | `write_kiosk_state` uses `with_extension("tmp")` which is non-atomic across filesystem boundaries | **RESOLVED** (Phase 0.5) |
| **BUG-008** | Blocker | Compilation Error | `companion/src-tauri/src/discovery.rs` | `ScopedIp` `.copied()` error in `discovered_node_from_service` | **RESOLVED** (Phase 0.1/0.5) |
| **BUG-009** | Blocker | Compilation Error | `crates/sw-agent/src/api.rs` | Missing `screen_status_response` & `apply_screen_command` functions | **RESOLVED** (Phase 0.1/0.5) |
| **BUG-010** | High | Resource Leak / Memory | `crates/sw-agent/src/jobs.rs` | Unbounded in-memory job history accumulation in `DockerJobsController` | **OPEN** |
| **BUG-011** | High | Panic / Data Integrity | `companion/src-tauri/src/jobs_cli.rs` | Invalid path indexing & UTF-8 slice panic in Explorer job path normalization | **OPEN** |
| **BUG-012** | High | Security / Elevation | `companion/src-tauri/src/app_control.rs` | Environment variable lost in elevated UAC process causing empty share password | **OPEN** |
| **BUG-013** | Medium | Security / Leak | `companion/src-tauri/src/apollo.rs` | Streaming admin password exposed in command-line arguments (`--creds`) | **OPEN** |
| **BUG-014** | Medium | Logic / Hardware | `companion/src-tauri/src/usb_control.rs` | Port lookup matches first device by VID:PID, detaching wrong device when identical USBs connected | **OPEN** |
| **BUG-015** | Medium | Error Handling | `companion/src-tauri/src/disk_control.rs` | Failed host iSCSI attach leaves node-side target exported with no rollback | **OPEN** |
| **BUG-016** | Low | Security / Storage | `companion/src-tauri/src/app_control.rs` | Non-atomic write and default file permissions for `app-session.pass` | **OPEN** |
| **BUG-017** | Low | Performance | `crates/sw-kiosk/src/main.rs` | Synchronous sub-process spawn (`date`) in ambient kiosk loop risks display stutter | **OPEN** |

---

## 3. Detailed Vulnerability & Bug Analysis

### BUG-000: Pairing PIN Leaked Over Network API (RESOLVED)
- **Location**: [`crates/sw-core/src/pairing.rs:L77-L87`](file:///d:/SecondWind/code/crates/sw-core/src/pairing.rs#L77-L87) & [`crates/sw-agent/src/pairing_state.rs:L23-L47`](file:///d:/SecondWind/code/crates/sw-agent/src/pairing_state.rs#L23-L47)
- **Severity**: Critical (Vulnerability)
- **Fix Status**: **Resolved in Phase 0.1/0.5**.
- **Analysis**: Network responses now return `PairingOffer` with PIN redacted. Only local kiosk state file carries full display payload.

---

### BUG-001: Race Condition in Pairing State Submission (RESOLVED)
- **Location**: [`crates/sw-agent/src/api.rs:L357-L401`](file:///d:/SecondWind/code/crates/sw-agent/src/api.rs#L357-L401)
- **Severity**: High
- **Fix Status**: **Resolved in Phase 0.5**.
- **Analysis**: `submit_pairing_request` now holds a single write lock across PIN validation, disk persistence, and marking paired (`state.pairing.write()`).

---

### BUG-002: Non-Atomic State & Certificate File Writes (RESOLVED)
- **Location**:
  - [`crates/sw-agent/src/identity.rs:L89`](file:///d:/SecondWind/code/crates/sw-agent/src/identity.rs#L89)
  - [`crates/sw-core/src/certificates.rs:L126-L154`](file:///d:/SecondWind/code/crates/sw-core/src/certificates.rs:L126-L154)
  - [`companion/src-tauri/src/host_state.rs:L143`](file:///d:/SecondWind/code/companion/src-tauri/src/host_state.rs#L143)
- **Severity**: High
- **Fix Status**: **Resolved in Phase 0.5**.
- **Analysis**: All state, certificate, and companion configuration persistence routines now route through `sw_core::certificates::write_atomic()` which writes to a `.tmp` file in the same directory before performing an atomic filesystem rename.

---

### BUG-003: Insecure Private Key File Permissions (RESOLVED)
- **Location**: [`crates/sw-core/src/certificates.rs:L142-L149`](file:///d:/SecondWind/code/crates/sw-core/src/certificates.rs:L142-L149) & [`crates/sw-agent/src/identity.rs:L89`](file:///d:/SecondWind/code/crates/sw-agent/src/identity.rs#L89)
- **Severity**: High
- **Fix Status**: **Resolved in Phase 0.5**.
- **Analysis**: `write_atomic` accepts a `restrict: bool` flag. When true, Unix permissions are set to `0o600` (owner read/write only) on the temp file prior to atomic rename.

---

### BUG-004: Unbounded PIN Verification Attempts (Brute Force Vulnerability) (RESOLVED)
- **Location**: [`crates/sw-agent/src/api.rs:L353-L382`](file:///d:/SecondWind/code/crates/sw-agent/src/api.rs#L353-L382)
- **Severity**: Medium
- **Fix Status**: **Resolved in Phase 0.5**.
- **Analysis**: Added `MAX_PIN_ATTEMPTS = 5` and an `AtomicU32` counter in `AgentState`. Five incorrect PIN entries permanently transition the pairing state to `PairingState::Unavailable`, invalidating the offer.

---

### BUG-005: Uncoordinated Store Self-Healing Breaks Paired Host Fingerprint
- **Location**: [`crates/sw-core/src/certificates.rs:L49-L60`](file:///d:/SecondWind/code/crates/sw-core/src/certificates.rs:L49-L60)
- **Severity**: Medium
- **Fix Status**: **OPEN**.
- **Description**: If one file in the certificate/key pair goes missing or gets corrupted while the other remains, `load_or_create_certificate` silently generates a fresh keypair, changing the SHA-256 fingerprint and breaking trust with all previously paired hosts without error diagnostics.

---

### BUG-006: Clippy `collapsible_if` Warning in `api.rs` (RESOLVED)
- **Location**: [`crates/sw-agent/src/api.rs:L385`](file:///d:/SecondWind/code/crates/sw-agent/src/api.rs#L385)
- **Severity**: Low
- **Fix Status**: **Resolved in Phase 0.5**.
- **Analysis**: Refactored nested `if let` blocks into a single let-chain `if let Some(identity_store) = &state.identity_store && ...`.

---

### BUG-007: Extension-based Temporary File Rename Risk (RESOLVED)
- **Location**: [`crates/sw-core/src/kiosk.rs:L88`](file:///d:/SecondWind/code/crates/sw-core/src/kiosk.rs#L88)
- **Severity**: High
- **Fix Status**: **Resolved in Phase 0.5**.
- **Analysis**: Updated `write_kiosk_state` to use `.with_file_name(format!("{file_name}.tmp"))` instead of `.with_extension("tmp")`, keeping temporary files strictly in the same directory.

---

### BUG-008: Compilation Failure in Companion mDNS Discovery (RESOLVED)
- **Location**: [`companion/src-tauri/src/discovery.rs:L99-L101`](file:///d:/SecondWind/code/companion/src-tauri/src/discovery.rs#L99-L101)
- **Severity**: Blocker (Compilation Error)
- **Fix Status**: **Resolved in Phase 0.1/0.5**.

---

### BUG-009: Work-in-Progress Compilation Failure in `/v1/screen` Endpoint Implementation (RESOLVED)
- **Location**: [`crates/sw-agent/src/api.rs:L194-L320`](file:///d:/SecondWind/code/crates/sw-agent/src/api.rs#L194-L320)
- **Severity**: Blocker (Compilation Error)
- **Fix Status**: **Resolved in Phase 0.1/0.5**.

---

### BUG-010: Unbounded In-Memory Job History Accumulation in `DockerJobsController`
- **Location**: [`crates/sw-agent/src/jobs.rs:L240-L260`](file:///d:/SecondWind/code/crates/sw-agent/src/jobs.rs#L240-L260)
- **Severity**: High
- **Category**: Resource Leak / Memory
- **Fix Status**: **OPEN**.
- **Description**: `DockerJobsController` maintains all completed and active jobs in `self.jobs` (`HashMap<String, RunningJob>`). Completed jobs set `job.child = None` and mark status as `Succeeded` or `Failed`, but entries are **never deleted or pruned**. On long-running nodes, memory usage grows unbounded, and calls to `GET /v1/jobs` serialize an ever-growing array of stale jobs.
- **Recommended Fix**: Implement a fixed-capacity ring buffer or timestamp-based TTL eviction (e.g. keep max 50 recent jobs).

---

### BUG-011: Invalid Path Indexing & UTF-8 Slice Panic in Companion Explorer Job Integration
- **Location**: [`companion/src-tauri/src/jobs_cli.rs:L128-L143`](file:///d:/SecondWind/code/companion/src-tauri/src/jobs_cli.rs#L128-L143)
- **Severity**: High
- **Category**: Panic / Data Integrity
- **Fix Status**: **OPEN**.
- **Description**: `relative_inside` normalizes input via `input.replace('/', "\\").trim_end_matches('\\').to_lowercase()` and then attempts to slice the original string with `&input[input.len() - rest.len()..]`.
  1. `to_lowercase()` does not preserve UTF-8 byte length for certain Unicode characters (e.g. `İ` or `ß`), leading to invalid byte index slicing and Rust runtime panics (`byte index X is not a char boundary`).
  2. If `input` contains trailing slashes, `trim_end_matches` reduces `rest.len()`, causing the slice to incorrectly cut into the root path name.
- **Recommended Fix**: Use `std::path::Path::strip_prefix` on normalized `Path` objects instead of string byte-length arithmetic.

---

### BUG-012: Environment Variable Lost in Elevated UAC Process Causing Empty Share Password
- **Location**: [`companion/src-tauri/src/app_control.rs:L104-L137`](file:///d:/SecondWind/code/companion/src-tauri/src/app_control.rs#L104-L137)
- **Severity**: High
- **Category**: Security / Elevation
- **Fix Status**: **OPEN**.
- **Description**: `run_share_setup_elevated` passes the share password via `.env("SECONDWIND_SHARE_PW", &credentials.password)` to `powershell.exe`, which executes `Start-Process powershell.exe -Verb RunAs`. On Windows, elevated child processes spawned via `Start-Process -Verb RunAs` **do not inherit environment variables** from the parent process. Consequently, `$env:SECONDWIND_SHARE_PW` inside the elevated script evaluates to `$null`, attempting share creation with an empty password and breaking Windows password complexity policy.
- **Recommended Fix**: Pass the argument safely into the script invocation block or use an encoded script parameter.

---

### BUG-013: Streaming Admin Password Exposed in Process Command-Line Arguments
- **Location**: [`companion/src-tauri/src/apollo.rs:L210-L226`](file:///d:/SecondWind/code/companion/src-tauri/src/apollo.rs#L210-L226)
- **Severity**: Medium
- **Category**: Security / Credential Exposure
- **Fix Status**: **OPEN**.
- **Description**: `set_apollo_credentials` passes Apollo admin credentials directly as command-line arguments: `Command::new(&installation.executable).arg("--creds").arg(&credentials.username).arg(&credentials.password)`. Command-line arguments for processes are world-readable on Windows and Linux via process listing utilities (`wmic`, `Get-CimInstance`, `ps aux`), exposing admin credentials to non-elevated local users.
- **Recommended Fix**: Pass credentials via stdin or environment variables if supported by Apollo binary CLI.

---

### BUG-014: Flawed USB Port Matching in `detach_device` With Identical Devices
- **Location**: [`companion/src-tauri/src/usb_control.rs:L51-L67`](file:///d:/SecondWind/code/companion/src-tauri/src/usb_control.rs#L51-L67)
- **Severity**: Medium
- **Category**: Logic / Hardware State
- **Fix Status**: **OPEN**.
- **Description**: `parse_attached_port` matches `(vendor_id:product_id)` in `usbip port` output and returns the first matching port index. When multiple USB devices with identical VID:PID are attached, `detach_device` always matches and detaches the first port, detaching the wrong device and leaving the requested device attached.
- **Recommended Fix**: Match attached devices by combining `bus_id` / remote location with VID:PID.

---

### BUG-015: Orphaned Node Disk Export on Host iSCSI Attach Failure
- **Location**: [`companion/src-tauri/src/disk_control.rs:L140-L191`](file:///d:/SecondWind/code/companion/src-tauri/src/disk_control.rs#L140-L191)
- **Severity**: Medium
- **Category**: Error Handling / State Inconsistency
- **Fix Status**: **OPEN**.
- **Description**: `connect_disk` sends `DiskAction::Enable` over mTLS before running the local PowerShell script (`Connect-SecondWindDisk.ps1`). If the script fails (e.g. iSCSI Initiator service disabled or script execution policy blocked), `connect_disk` returns an error without sending a rollback `DiskAction::Disable` request to the node. The node is left in `DiskState::Exposed` with the iSCSI daemon active, while the host companion has no record of it in `attached_disks`.
- **Recommended Fix**: Wrap script execution in a try-catch block that sends `DiskAction::Disable` on failure.

---

### BUG-016: Non-Atomic Write & Default File Permissions for `app-session.pass`
- **Location**: [`companion/src-tauri/src/app_control.rs:L322-L327`](file:///d:/SecondWind/code/companion/src-tauri/src/app_control.rs#L322-L327)
- **Severity**: Low
- **Category**: Security / Storage
- **Fix Status**: **OPEN**.
- **Description**: `launch_on_node` writes `app-session.pass` using `fs::write` directly without atomic write or restricted OS permissions (`0600`), exposing session passkeys to default process umask permissions on multi-user systems.
- **Recommended Fix**: Use `sw_core::certificates::write_atomic` with `restrict = true`.

---

### BUG-017: Synchronous Sub-process Spawn in Kiosk Ambient Loop
- **Location**: [`crates/sw-kiosk/src/main.rs:L140-L170`](file:///d:/SecondWind/code/crates/sw-kiosk/src/main.rs#L140-L170)
- **Severity**: Low
- **Category**: Performance / Resilience
- **Fix Status**: **OPEN**.
- **Description**: `ambient_stats()` calls `Command::new("date").output()` synchronously inside the main kiosk render loop. If sub-process execution stalls or experiences OS scheduler delay, the entire kiosk interface loop freezes, missing state file updates.
- **Recommended Fix**: Use standard library time formatting (`chrono` or `time`) instead of spawning `date`.

---

## 4. Complete Code Addition & Audit Trail Log

| Timestamp (UTC) | Commit / File | Change Type | Summary of Changes |
|---|---|---|---|
| 2026-07-22 01:46 | `1833c59` / `certificates.rs` | Refactor | Certificate store extracted into shared `sw-core` crate |
| 2026-07-22 01:47 | `18ec472` / `pairing_state.rs` | Security Fix | Fixed PIN leak in network response; added `kiosk_display()` |
| 2026-07-22 01:51 | `f97f059` / `host_state.rs` | Added | Host persistent state, host certificate & paired node config store |
| 2026-07-22 01:53 | `f97f059` / `node_client.rs` | Added | HTTPS node client with SHA-256 fingerprint verifier & mTLS auth |
| 2026-07-22 01:55 | `b0bb204` / `kiosk.rs` | Added | Agent ↔ kiosk state schema (`KioskState`) & atomic file watcher contract |
| 2026-07-22 01:56 | `b0bb204` / `api.rs` | Feature | Added `/v1/screen` status & command handlers (`ScreenAction::Connect`/`Disconnect`) |
| 2026-07-22 03:45 | `BUG_TRACKER.md` | Audit Update | Full Phase 0.1–0.5 audit. Updated BUG-001..009 statuses, added BUG-010..017 |

---

## 5. Verification Log & Final Status

- [x] **Zero Code Modification Policy**: 100% compliant. No `.rs` or `.toml` code files edited by monitoring agent.
- [x] **Static & Dynamic Audit**: Complete across `sw-core`, `sw-agent`, `sw-kiosk`, `sw-launcher`, and `companion` modules for Phases 0.1 through 0.5.
- [x] **Test Verification**: **67/67 unit tests passing** (55 core workspace tests + 12 companion tests).
- [x] **Phase 0.1 – 0.5 Completion**: Audit fully logged and documented.
