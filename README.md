# SecondWind

SecondWind turns an old laptop into an extra monitor and, in later phases, a node for disk, USB, app, and job offload features.

Current status: v0.1 has started. Phase 0 manual proof passed on the first developer machines and is documented in `docs/COMPATIBILITY.md` and `docs/PHASE0-TROUBLESHOOTING.md`.

## Product Boundary

SecondWind is the user-facing product. Apollo, Moonlight, Debian, cage, VA-API tools, and related services are upstream internals. Normal users should not open the Apollo dashboard, run Moonlight setup, choose drivers, edit Debian configuration, or type terminal commands on the node.

The intended user path is:

1. Install the SecondWind Windows companion.
2. Install or flash the SecondWind node image.
3. Pair the host and node.
4. Connect the link.
5. Use the node as an extra screen.

## Repository Layout

- `crates/sw-core`: shared protocol, config, pairing, and capability types.
- `crates/sw-agent`: node daemon.
- `crates/sw-launcher`: host launch helper crate, scaffolded for later phases.
- `companion`: Windows companion app scaffold.
- `node-image`: Debian image configuration scaffold.
- `scripts`: host and node helper script scaffold.
- `installer`: Windows installer scaffold.
- `docs`: architecture, setup, compatibility, decisions, and proof notes.

## Development

```powershell
cargo test
```

See `docs/SETUP-DEV.md` for developer setup notes.
