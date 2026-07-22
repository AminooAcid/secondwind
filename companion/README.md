# Companion Scaffold

The Windows companion will be the user-facing host app for SecondWind.

v0.1 responsibilities:

- Discover nodes.
- Pair with a node using the SecondWind pairing flow.
- Show paired node state.
- Expose a Screen toggle.
- Configure and supervise the host-side screen path without exposing Apollo's browser UI.

This folder is scaffolded only. The Tauri manifest and Rust entrypoint exist so the companion has a real app boundary; the full UI will be implemented as the shared core and node-agent API settle.

## Agent API Contract

The companion must use `sw-core::agent_api` route constants and shared protocol types for node communication. It should not duplicate `/v1/` paths or parse Apollo/Moonlight state directly.
