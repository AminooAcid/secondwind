# SecondWind

One flashable Linux image + one Windows companion that turn any old x86-64
laptop into an **extra monitor, disk, USB hub, and app/compute engine** for
your main PC — with all of your data staying on the main PC.

**Host** = your main Windows PC. **Node** = the old laptop running the
SecondWind image.

## Status

All planned phases through **v0.5 are code-complete** with unit tests
green. Hardware validation on the first physical host/node pair is the
remaining gate before phase tags — see `docs/V0.1-CHECKLIST.md` and the
per-phase notes in `docs/DECISIONS.md`. Phase 0 (manual proof) passed on
real hardware and is documented in `docs/COMPATIBILITY.md`.

## Feature matrix

| Feature | What you see | Status |
|---|---|---|
| Screen | The node becomes an extra monitor; connect/disconnect reflows Windows like a real monitor | code-complete |
| Pairing | QR + 6-digit PIN on the node's screen, typed once into the companion | code-complete |
| Auto-connect | Plug the link in → everything marked "always on" comes up, one toast | code-complete |
| Disk | A node partition appears as a Windows drive letter, flushed and detached with the link | code-complete |
| Apps | One icon per app; runs on the node as a native-looking window, or locally, per your policy — Firefox, Chromium, VLC, LibreOffice, GIMP, PDF reader | code-complete |
| USB | Devices plugged into the node appear in the host's Device Manager | code-complete |
| Jobs | Explorer right-click: convert / compress / download on the node | code-complete |
| Wake | Launching against a powered-off node wakes it first | code-complete |

## Quickstart (developer preview)

1. **Host:** build and run the companion — `cd companion/src-tauri && cargo tauri dev` (needs Rust + the Tauri prerequisites; see `docs/SETUP-DEV.md`).
2. **Node:** build the ISO on a Debian machine — `cd node-image/live-build && sudo ./build.sh` — then boot it on the old laptop (dual-boot install is the guided default and only ever uses the free space you chose).
3. The node shows a SecondWind pairing screen; the companion finds it; enter the PIN.
4. Toggle **Screen** on (or just replug the link — it's automatic after pairing).

The finished product wraps steps 1–2 in a Windows installer and a
flashable image; the user path is install → flash → pair → done, with no
terminal anywhere.

## Repository layout

- `crates/sw-core` — shared protocol, config, pairing, certificates, kiosk contract.
- `crates/sw-agent` — node daemon: capability detection, pairing, mTLS API, feature control.
- `crates/sw-kiosk` — the node's screens (pairing QR/PIN, ambient idle, streaming supervision).
- `crates/sw-launcher` — per-app launch policy, Wake-on-LAN, seamless-client spec.
- `companion` — Windows companion (Tauri): discovery, pairing, toggles, app library, auto-connect.
- `node-image` — Debian live-build tree producing the bootable node ISO.
- `scripts` — PowerShell (host) and shell (node) glue invoked by the products.
- `installer` — Windows installer script + third-party manifest.
- `docs` — architecture, decisions, setup, compatibility, troubleshooting.

## Credits

SecondWind is glue and UX around excellent open-source projects, invoked
as separate processes and never re-implemented: **Apollo** (host streaming
+ virtual display), **Moonlight** (node streaming client), **xpra**
(seamless app windows), **cage** + **foot** (node kiosk shell), **Avahi**
(discovery), **LIO/targetcli** (iSCSI disk), **usbip / usbip-win2** (USB),
**Docker** (job sandbox), **Debian + live-build** (node OS/image),
**Inno Setup** (host installer), **outrun** (documented for advanced CLI
offload). Exact versions and licenses are pinned in `docs/UPSTREAM.md`.

## Known limits (by design)

- No CPU/RAM merging into the host OS — physically impossible over a network; SecondWind offloads work instead.
- The streamed display adds roughly one frame of latency: excellent for productivity, unsuitable for competitive gaming.
- Node performance is bounded by genuinely old hardware: browsing, media, office, moderate compute — not GPU work or heavy video editing.
- Wine compatibility (future tier) is per-app.

## License

MIT (our code). Upstream projects keep their own licenses; GPL tools are
invoked as separate processes, never linked in.
