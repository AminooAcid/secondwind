# Hardware validation checklist (v0.1 → v0.5)

Everything through v0.5 is code-complete with unit tests green. Each
phase's *acceptance* is hardware-observable, so phases are tagged only as
the corresponding section below passes on the first physical pair
(`docs/PROFILE-dev-machine.md`). Log results (working or not) in
`docs/COMPATIBILITY.md`; fix-ups get normal commits.

Every step assumes: Debian minimal installed on the node (FIRST-SETUP
steps A–B), then `scripts/node/install-node.sh` run per
`docs/NODE-SETUP.md`; companion running on the host. (The ISO path in
`node-image/` is the 1.0 alternative — see BACKLOG.)

> **First hardware run: 2026-07-22, MSI Haswell node + Debian 13 (trixie).
> v0.1 core PASSED; findings folded back into the code. See
> `docs/COMPATIBILITY.md` for the per-fix list.**

## v0.0 — Install path itself

- [x] `install-node.sh` completes on a fresh Debian minimal install
      (packages, Moonlight repo, binaries). ✅ from public GitHub, exit 0.
- [x] Re-running it updates cleanly and keeps `/etc/secondwind/` edits. ✅

## v0.1 — Screen + pairing + auto-connect

- [x] After reboot the node reaches the SecondWind pairing screen (QR + PIN), no terminal. ✅ (GRUB defaulted to the node entry)
- [x] Companion discovers the node; PIN pairs; kiosk flips to the idle screen. ✅ (mDNS over real network; kiosk showed paired-idle ambient screen)
- [x] mTLS enforced after pairing; a no-client-cert connection is rejected. ✅
- [x] H.264 hardware decode detected (HD 4600). ✅ (`screen: ok` in `/v1/health`)
- [~] Apollo layer: managed config block accepted by the installed Apollo version; credentials + PIN arming work against its localhost API. **Validated up to the API: managed config merged (foreign keys preserved), credentials created via `sunshine.exe --creds`, service control works. Then hit Apollo-side instability (see below).**
- [ ] Screen toggle: virtual display appears matching the node panel; windows reflow on disconnect. **Blocked by the Apollo instability below, not by SecondWind code.**

### Apollo 0.4.7-alpha instability (host-side, 2026-07-22)

Driving `--screen-on` repeatedly, the companion correctly: detected Apollo
(`ApolloService`), merged its managed config, created SecondWind
credentials, and controlled the service. The **PIN-arming API call** then
surfaced a cascade of Apollo-startup timing issues that the companion was
hardened against (each committed):

- `sc.exe` matches the service *name* `ApolloService`, not the display name.
- Apollo takes ~30 s to stop (STOP_PENDING) — restart must wait for STOPPED.
- Apollo reaches RUNNING before its HTTPS API binds — must wait for the API.
- New credentials 401 until the service restarts (config/creds read at boot).
- Only restart on a confirmed 401, never on a slow-but-healthy API.

After the churn, Apollo's web UI (47990) stopped binding. **Root-caused
(confirmed against Apollo `src/main.cpp`): startup runs `probe_encoders()`
→ `http::init()` → `confighttp::start()`, so the web UI binds *after* the
encoder self-test. On this Optimus/Max-Q laptop the NVENC probe hangs
(`NV_ENC_ERR_DEVICE_NOT_EXIST` — the dGPU powers down when idle), so the
service reports RUNNING but 47990 never binds.** Two things fixed it,
both now in the code:

1. **Force a reliable host encoder** (`encoder = software`, or quicksync)
   so the probe can't hang — `SECONDWIND_APOLLO_ENCODER` +
   `managed_config_entries`.
2. **`address_family = ipv4`** so the dashboard binds 127.0.0.1 (the
   companion now talks to 127.0.0.1, not `localhost`→::1). Sunshine #2793.

Plus: pin `port = 47989`; set credentials only with the service stopped;
and a one-time reset of `sunshine_state.json` cleared credential state
mangled by earlier `--creds` races. **With these, Apollo served 47990 and
the full companion→Apollo→node→Moonlight chain ran: Apollo configured, PIN
armed, node told to connect, and the node's Moonlight reached Apollo and
requested pairing.**

### The one remaining gap: Moonlight ↔ Apollo pairing handshake

The node's Moonlight connects to Apollo but **pairing does not complete**,
so the stream is refused ("computer SecondWind has not been paired"). Our
design arms a PIN on Apollo (`/api/pin`) and runs
`moonlight pair <host> --pin <armed>`, assuming Moonlight uses the armed
PIN. On moonlight-qt 6.1 (Flatpak) that assumption is wrong — Moonlight
generates *its own* PIN. Manual completion over SSH is not viable
(headless `moonlight pair` prints nothing; the node kiosk has no
interactive GUI). **This is the last integration gap and needs the code
fix in the backlog. Everything up to it is validated on hardware.**
- [ ] Moonlight CLI flags match the shipped client (Flatpak Moonlight 6.1 via the `moonlight` wrapper — verify `pair`/`stream` args).
- [ ] Auto-connect across cable replug and across the USB-hub Ethernet adapter disappearing.
- [~] Node idle RAM under ~400 MB. **Currently ~530 (screen-only) to ~640 MB (all services). Debian 13 session stack + always-on xpra; Docker now socket-activated, bloat masked. Reaching target needs lazy-start xpra + session-daemon trim (backlog).**

## v0.2 — Disk

- [ ] Installer recipe creates the `SECONDWIND_DATA` partition in the freed space ("biggest free space" + expert recipe interaction — verify, adjust `preseed.cfg`).
- [ ] First boot provisions IQN/CHAP; disk attaches, formats NTFS on first use, gets a letter.
- [ ] Drive letter appears/disappears with the link; contents survive reboots; flush-before-detach loses nothing.

## v0.3 — Apps

- [ ] Share setup elevates once; node mounts it; a file saved from node-side Firefox lands in the host folder.
- [ ] Firefox "Always on node" end-to-end: seamless window, profile cache-and-sync round trip, fallback to local when the node is off (policy allows), WoL wake-then-launch.
- [ ] `xpra control start` flags match the shipped xpra (adjust `sw-agent/src/apps.rs`).

## v0.4 — USB

- [ ] usbip-win2 driver install per `docs/USB-SETUP.md`.
- [ ] Flash drive plugged into the node appears in host Explorer; detach is clean; "Always attach" reattaches on reconnect.
- [ ] `usbip port` output parsing matches the bundled client (adjust `companion/src-tauri/src/usb_control.rs`).

## v0.5 — Jobs + polish

- [ ] Right-click → "Compress on node" on a file in the SecondWind folder produces the archive next to it.
- [ ] "Convert to MP4 on node" works on a sample video (first run pulls the ffmpeg image — needs node internet).
- [ ] Download preset saves into the shared Downloads folder.
- [ ] Ambient idle screen shows clock + memory and doesn't flicker.
- [ ] Installer (`secondwind.iss`) builds and installs everything per `THIRD-PARTY.md`; uninstall removes the context menu.

After all sections pass: update `docs/COMPATIBILITY.md`, pin versions in
`docs/UPSTREAM.md`, tag `v0.1`…`v0.5` in order, CI green.
