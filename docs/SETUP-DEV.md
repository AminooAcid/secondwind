# Developer Setup

Current status: v0.1 scaffold.

## Requirements

- Rust toolchain with Cargo.
- Windows for host companion development.
- Debian or a Debian-like environment for node agent and image work.

## Build And Test

From the repository root:

```powershell
cargo test
```

Expected result:

- `sw-core` tests pass.
- `sw-launcher` tests pass.
- `sw-agent` builds.

## Current Limits

- The Tauri companion is scaffolded but not yet implemented.
- The node image folders are scaffolded but do not yet produce an image.
- The agent has shared types and a binary skeleton but no HTTP listener yet.
- No upstream binaries are bundled yet.

## User Experience Rule

Developer setup may use terminals and source checkouts. End users must not. The future release path is a SecondWind Windows installer and a SecondWind node image/installer.
