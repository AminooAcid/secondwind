# SecondWind Codebase Bug Tracker & Audit Log

> **DOCUMENT PURPOSE**  
> This document is created and maintained by the **Monitoring & Auditing Agent** while another agent is actively modifying the codebase to implement Phase 0.1 / 0.5 features.  
> **Strict Operational Rule**: The monitoring agent **MUST NOT** edit or alter any source code (`.rs`, `.toml`, etc.). Its sole duty is to inspect code, discover bugs, document architectural vulnerabilities, and track code changes introduced by the parallel agent until Phase 0.5 is completed.

---

## 1. Executive Summary & Monitoring Overview

- **Target Repository**: `SecondWind` (`sw-core`, `sw-agent`, `sw-kiosk`, `sw-launcher`, `companion`)
- **Mode**: Read-Only Code Audit, Vulnerability Tracking & Phase 0.1 / 0.5 Completion Monitor
- **Audit Sync Timestamp**: 2026-07-22 01:58 UTC
- **Active Workspace Status**:
  - **PHASE 0.1 & PHASE 0.5 COMMITS COMPLETED**: The parallel agent finished all Phase 0.1 and Phase 0.5 code additions (Commits `b0bb204`, `f97f059`, `18ec472`, `1833c59`).
  - **ALL WORKSPACE TESTS PASSING**: 67/67 total unit tests pass across the repository (55 core workspace tests + 12 companion tests).
  - **Git Working Tree**: Clean working tree on `main` (only `BUG_TRACKER.md` is present).
  - Policy Compliance: 100% compliant. Zero source code files modified by monitoring agent.

---

## 2. Identified Bugs & Vulnerabilities Index

| Bug ID | Severity | Category | Component | Brief Description | Status |
|---|---|---|---|---|---|
| **BUG-000** | Critical | Security / Leak | `sw-core::pairing` & `sw-agent::pairing_state` | Unauthenticated network API `/api/v1/pairing` leaked pairing PIN & QR payload | **RESOLVED** (by parallel agent) |
| **BUG-001** | High | Concurrency / Race | `sw-agent::api` | Lock gap between PIN check and disk persistence allows state race condition | **OPEN** |
| **BUG-002** | High | Data Integrity | `sw-agent::identity`, `sw-core::certificates`, `companion::host_state` | Direct `fs::write` used for identity, certificates & companion config without atomic write/rename | **OPEN** |
| **BUG-003** | High | Security | `sw-core::certificates` & `companion::host_state` | Private key files written with default umask permissions (unrestricted readability) | **OPEN** |
| **BUG-004** | Medium | Security / Auth | `sw-agent::api` | Missing PIN attempt rate-limiting or lockout on `/api/v1/pairing` endpoint | **OPEN** |
| **BUG-005** | Medium | Resilience | `sw-core::certificates` | Partial store self-healing creates new cert/key set if key missing, invalidating host trust | **OPEN** |
| **BUG-006** | Low | Code Style / Lint | `sw-agent::api` | Clippy `collapsible_if` warning on `identity_store` persistence condition | **OPEN** |
| **BUG-007** | High | Data Integrity | `crates/sw-core/src/kiosk.rs` | `write_kiosk_state` uses `with_extension("tmp")` which is non-atomic across filesystem boundaries | **OPEN** |
| **BUG-008** | Blocker | Compilation Error | `companion/src-tauri/src/discovery.rs` | `ScopedIp` `.copied()` error in `discovered_node_from_service` | **RESOLVED** (by parallel agent) |
| **BUG-009** | Blocker | Compilation Error | `crates/sw-agent/src/api.rs` | Missing `screen_status_response` & `apply_screen_command` functions | **RESOLVED** (by parallel agent) |

---

## 3. Detailed Vulnerability & Bug Analysis

### BUG-009: Work-in-Progress Compilation Failure in `/v1/screen` Endpoint Implementation (RESOLVED)
- **Location**: [`crates/sw-agent/src/api.rs:L194-L320`](file:///d:/SecondWind/code/crates/sw-agent/src/api.rs#L194-L320)
- **Severity**: Blocker (Compilation Error)
- **Fix Status**: **Resolved by parallel agent**.
- **Analysis**: Implemented `screen_status_response` and `apply_screen_command` handling `ScreenAction::Connect` and `ScreenAction::Disconnect`, updating `AgentState` test initializers.

---

### BUG-008: Compilation Failure in Companion mDNS Discovery (RESOLVED)
- **Location**: [`companion/src-tauri/src/discovery.rs:L99-L101`](file:///d:/SecondWind/code/companion/src-tauri/src/discovery.rs#L99-L101)
- **Severity**: Blocker (Compilation Error)
- **Fix Status**: **Resolved by parallel agent**.

---

### BUG-000: Pairing PIN Leaked Over Network API (RESOLVED)
- **Location**: [`crates/sw-core/src/pairing.rs:L77-L87`](file:///d:/SecondWind/code/crates/sw-core/src/pairing.rs#L77-L87) & [`crates/sw-agent/src/pairing_state.rs:L23-L47`](file:///d:/SecondWind/code/crates/sw-agent/src/pairing_state.rs#L23-L47)
- **Severity**: Critical (Vulnerability)
- **Fix Status**: **Resolved by parallel agent**.

---

### BUG-001: Race Condition in Pairing State Submission
- **Location**: [`crates/sw-agent/src/api.rs:L139-L169`](file:///d:/SecondWind/code/crates/sw-agent/src/api.rs#L139-L169)
- **Severity**: High
- **Description**: In `submit_pairing_request`, `state.pairing.read()` is acquired to validate the pairing PIN. Once validated, the read lock is dropped while disk persistence (`persist_paired_host`) runs before acquiring the write lock.

---

### BUG-002: Non-Atomic State & Certificate File Writes
- **Location**:
  - [`crates/sw-agent/src/identity.rs:L74-L92`](file:///d:/SecondWind/code/crates/sw-agent/src/identity.rs#L74-L92)
  - [`crates/sw-core/src/certificates.rs:L111-L130`](file:///d:/SecondWind/code/crates/sw-core/src/certificates.rs:L111-L130)
  - [`companion/src-tauri/src/host_state.rs:L101-L115`](file:///d:/SecondWind/code/companion/src-tauri/src/host_state.rs#L101-L115)
- **Severity**: High
- **Description**: `write_identity`, `write_certificate_files`, and `write_config` write JSON identity files, PEM certificate files, and companion `config.json` directly using `fs::write`.

---

### BUG-003: Insecure Private Key File Permissions
- **Location**: [`crates/sw-core/src/certificates.rs:L124-L129`](file:///d:/SecondWind/code/crates/sw-core/src/certificates.rs:L124-L129) & [`companion/src-tauri/src/host_state.rs:L54-L58`](file:///d:/SecondWind/code/companion/src-tauri/src/host_state.rs#L54-L58)
- **Severity**: High
- **Description**: `host-key.pem` and `node-key.pem` are created using `fs::write` which inherits default process umask (`0644`/`0664`) on Unix targets.

---

### BUG-004: Unbounded PIN Verification Attempts (Brute Force Vulnerability)
- **Location**: [`crates/sw-agent/src/api.rs:L110-L115`](file:///d:/SecondWind/code/crates/sw-agent/src/api.rs#L110-L115)
- **Severity**: Medium
- **Description**: The 6-digit numeric pairing PIN accepts unlimited submission attempts without rate-limiting.

---

### BUG-005: Uncoordinated Store Self-Healing Breaks Paired Host Fingerprint
- **Location**: [`crates/sw-core/src/certificates.rs:L49-L60`](file:///d:/SecondWind/code/crates/sw-core/src/certificates.rs:L49-L60)
- **Severity**: Medium
- **Description**: Partial keypair store corruption triggers silent re-generation of certificate material, invalidating paired host fingerprints.

---

### BUG-006: Clippy `collapsible_if` Warning in `api.rs`
- **Location**: [`crates/sw-agent/src/api.rs:L150-L160`](file:///d:/SecondWind/code/crates/sw-agent/src/api.rs#L150-L160)
- **Severity**: Low
- **Description**: Nested `if let` blocks can be collapsed.

---

### BUG-007: Extension-based Temporary File Rename Risk
- **Location**: [`crates/sw-core/src/kiosk.rs:L81-L89`](file:///d:/SecondWind/code/crates/sw-core/src/kiosk.rs#L81-L89)
- **Severity**: High
- **Description**: `write_kiosk_state` constructs `temp_path` via `path.with_extension("tmp")`.

---

## 4. Complete Phase 0.1 / 0.5 Code Addition Log

| Timestamp (UTC) | Commit / File | Change Type | Summary of Changes |
|---|---|---|---|
| 2026-07-22 01:46 | `1833c59` / `certificates.rs` | Refactor | Certificate store extracted into shared `sw-core` crate |
| 2026-07-22 01:47 | `18ec472` / `pairing_state.rs` | Security Fix | Fixed PIN leak in network response; added `kiosk_display()` |
| 2026-07-22 01:51 | `f97f059` / `host_state.rs` | Added | Host persistent state, host certificate & paired node config store |
| 2026-07-22 01:53 | `f97f059` / `node_client.rs` | Added | HTTPS node client with SHA-256 fingerprint verifier & mTLS auth |
| 2026-07-22 01:55 | `b0bb204` / `kiosk.rs` | Added | Agent ↔ kiosk state schema (`KioskState`) & atomic file watcher contract |
| 2026-07-22 01:56 | `b0bb204` / `api.rs` | Feature | Added `/v1/screen` status & command handlers (`ScreenAction::Connect`/`Disconnect`) |

---

## 5. Verification Log & Final Status

- [x] **Zero Code Modification Policy**: 100% compliant. No `.rs` or `.toml` code files edited by monitoring agent.
- [x] **Static Audit**: Complete across core, agent, kiosk, launcher, and companion modules.
- [x] **Test Verification**: **67/67 unit tests passing** (55 core workspace tests + 12 companion tests).
- [x] **Phase 0.1 & Phase 0.5 Completion**: Fully verified. Commits clean and passing.
