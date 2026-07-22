# Architecture

SecondWind is a Windows companion plus a Debian node image. The companion and node agent hide upstream tools behind a SecondWind UX.

## Components

- `sw-core`: shared protocol, config, pairing, and capability types.
- `sw-agent`: node daemon. It owns node state, capability detection, pairing, local service supervision, and the node API.
- `sw-launcher`: host-side launch helper scaffold. It exists now so later phases have the planned crate boundary.
- `companion`: Windows companion UI and Rust core. It owns pairing, discovery, host-side state, feature toggles, and user-visible errors.
- `node-image`: Debian image configuration. It will include the agent, kiosk, Moonlight client path, and required system services.

## Product Boundary

Apollo, Moonlight, Debian, cage, VA-API tooling, and system services are implementation details. Users should see SecondWind screens, SecondWind pairing, and SecondWind errors.

Phase 0 used upstream UIs manually. v0.1 begins replacing that with:

- SecondWind pairing state.
- SecondWind capability checks.
- SecondWind-controlled screen startup.
- SecondWind logs for support.

## Host And Node Experience

The host and node sides must both become easier than the Phase 0 proof:

- Host users interact with the SecondWind companion, not the Apollo browser dashboard. The companion owns Apollo configuration, virtual-display setup, pairing state, screen toggles, recovery actions, and user-facing diagnostics.
- Node users interact with the SecondWind node image, not Debian package setup, Moonlight settings, `cage`, `vainfo`, or driver selection. The node image owns capability detection, kiosk supervision, pairing display, service startup, and clear local status/error screens.
- Developer/debug logs may expose Apollo, Moonlight, Debian, VA-API, or network details for support, but normal product flows should explain problems in SecondWind terms.

## v0.1 Flow

1. The node boots into a minimal Debian system.
2. `sw-agent` detects screen capabilities and advertises the node.
3. The companion discovers the node.
4. The node shows a pairing QR/PIN.
5. The companion completes pairing and stores trust by node UUID.
6. On link availability, the companion enables the screen feature.
7. The agent supervises the kiosk streaming client.
8. Disconnect tears down the virtual display path cleanly.

`sw-core` models node IDs as validated UUIDs. Placeholder display names may be user-friendly strings, but persistent host-side state and trust are keyed by real node UUID values.

## API

The API is versioned under `/v1/`.

Initial routes:

- `GET /v1/health`
- `GET /v1/capabilities`

Pairing and feature-control routes will be added as v0.1 implementation progresses. The locked target remains JSON over HTTPS with mutual TLS.

The first API slice provides these routes as an in-process router so behavior can be tested before choosing bind configuration. It does not hardcode a listening interface or port.

Agent runtime configuration is supplied outside source. The agent reads its state-file path, optional bind address, and display name from environment/config so source does not bake in ports, interfaces, or machine-specific paths.

Pairing routes currently expose explicit `Unavailable`, `WaitingForHost`, and `Paired` states. Certificate generation and mutual TLS are still upcoming v0.1 work; until certificate material exists, the agent must report pairing unavailable rather than fabricate trust.

## Capability Detection

Capability detection must inspect the running machine. It must not assume:

- GPU model
- render device name
- codec support
- panel resolution
- refresh rate
- network interface name
- host path
- drive letter
- IP address

The Phase 0 proof showed why this matters: the working Intel H.264 decoder was not the first render device.

Current first slice implements a VA-API probe that enumerates `/dev/dri/renderD*`, runs `vainfo` for each candidate render node, and treats H.264 support as valid only when `VAEntrypointVLD` is present. This will feed `/v1/capabilities` as the agent API is built out.
