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

## v0.0 — Install path itself

- [ ] `install-node.sh` completes on a fresh Debian minimal install
      (packages, Moonlight repo, `node-rolling` binaries download).
- [ ] Re-running it updates cleanly and keeps `/etc/secondwind/` edits.

## v0.1 — Screen + pairing + auto-connect

- [ ] After reboot the node reaches the SecondWind pairing screen (QR + PIN), no terminal.
- [ ] Companion discovers the node; PIN pairs; kiosk flips to the idle screen.
- [ ] Apollo layer: managed config block accepted by the installed Apollo version; credentials + PIN arming work against its localhost API (`companion/src-tauri/src/apollo.rs` holds the keys/endpoints to adjust).
- [ ] Screen toggle: virtual display appears matching the node panel; windows reflow on disconnect.
- [ ] Moonlight CLI flags (`pair --pin`, `stream --quit-after`) match the shipped moonlight-qt (adjust `sw-kiosk/src/supervise.rs`).
- [ ] Auto-connect across cable replug and across the USB-hub Ethernet adapter disappearing.
- [ ] Node idle RAM under ~400 MB.

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
