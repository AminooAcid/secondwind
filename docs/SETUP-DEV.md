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
- The agent has an HTTP router and runtime config, but mutual TLS/discovery are still upcoming.
- No upstream binaries are bundled yet.

## User Experience Rule

Developer setup may use terminals and source checkouts. End users must not. The future release path is a SecondWind Windows installer and a SecondWind node image/installer.

## Run Agent Skeleton

The agent requires an explicit state file path. Binding a network listener is optional for development and must also be supplied explicitly.

```powershell
$env:SECONDWIND_AGENT_STATE_FILE=".tmp\sw-agent-state.json"
$env:SECONDWIND_AGENT_CERTIFICATE_FILE=".tmp\sw-agent-node-cert.pem"
$env:SECONDWIND_AGENT_PRIVATE_KEY_FILE=".tmp\sw-agent-node-key.pem"
$env:SECONDWIND_AGENT_NODE_NAME="SecondWind dev node"
cargo run -p sw-agent
```

Expected result: the command prints a JSON health object and creates the state, certificate, and key files if they do not exist.

To serve the in-process API router during development, also set `SECONDWIND_AGENT_BIND` to a socket address chosen by the developer or test harness.

