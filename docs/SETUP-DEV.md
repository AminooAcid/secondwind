# Developer Setup

## Requirements

- Rust toolchain with Cargo (stable).
- **Host work** (companion): Windows + the Tauri v2 prerequisites
  (WebView2, MSVC build tools).
- **Node work** (agent/kiosk/image): Debian or a Debian container/WSL;
  the ISO build additionally needs `live-build` and `curl`.

## Build and test

Workspace (sw-core, sw-agent, sw-kiosk, sw-launcher) from the repo root:

```powershell
cargo test --workspace
```

Companion (its own workspace):

```powershell
cd companion/src-tauri
cargo test --lib     # unit tests
cargo tauri dev      # run the app against companion/ui
```

Expected result: every suite reports `test result: ok`.

## Node ISO

```bash
cd node-image/live-build
sudo ./build.sh                    # SECONDWIND_DEBIAN_DIST=<codename> to override
```

See `node-image/README.md` for what the image contains.

## Run agent + kiosk locally (Linux)

```bash
export SECONDWIND_AGENT_STATE_FILE=/tmp/sw/state.json
export SECONDWIND_AGENT_CERTIFICATE_FILE=/tmp/sw/cert.pem
export SECONDWIND_AGENT_PRIVATE_KEY_FILE=/tmp/sw/key.pem
export SECONDWIND_AGENT_BIND=0.0.0.0:0
export SECONDWIND_KIOSK_STATE_FILE=/tmp/sw/kiosk.json
cargo run -p sw-agent &
SECONDWIND_KIOSK_ALLOW_EXIT=1 cargo run -p sw-kiosk   # press q to quit
```

On Windows, the agent also runs without `SECONDWIND_AGENT_BIND` and just
prints its health JSON (useful for a quick smoke check).

## Useful development environment variables

| Variable | Effect |
|---|---|
| `SECONDWIND_COMPANION_STATE_DIR` | companion state dir override (default: app-data) |
| `SECONDWIND_SCRIPTS_DIR` | where the companion finds the PowerShell scripts |
| `SECONDWIND_APOLLO_DIR` / `SECONDWIND_APOLLO_API` / `SECONDWIND_APOLLO_SERVICE` | Apollo detection overrides |
| `SECONDWIND_XPRA_CLIENT` / `SECONDWIND_USBIP_CLIENT` | host client binary overrides |
| `SECONDWIND_AGENT_*` | agent state/cert/bind/name (see `sw-agent/src/main.rs`) |
| `SECONDWIND_KIOSK_*` | kiosk state file, client, poll, dev escape (`SECONDWIND_KIOSK_ALLOW_EXIT=1`) |

## User experience rule

Developer setup may use terminals and source checkouts. End users must
not: the release path is the SecondWind Windows installer and the
SecondWind node image, nothing else.

## Conventions

- Conventional commits; never commit secrets or generated certificates.
- Every non-trivial decision goes to `docs/DECISIONS.md` with date + reason.
- No hardcoded hardware/network/display values in source — detection or
  config only (plan §2; the kiosk and Apollo modules carry guard tests).
