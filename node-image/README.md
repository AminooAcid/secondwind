# Node Image

Debian image configuration for the SecondWind node.

## Layout

- `live-build/` — full `live-build` tree. `build.sh` builds the ISO: it
  compiles `sw-agent` + `sw-kiosk` for Linux, stages them with the systemd
  units, instantiates the Moonlight apt repo for the chosen Debian release,
  and runs `lb build`. The ISO boots a live SecondWind node and carries the
  Debian installer preseeded for the safe dual-boot path (guided install
  into the largest continuous free space only).
- `systemd/` — single source of truth for the units and env examples;
  `build.sh` copies the units into the image.
- `kiosk/` — kiosk session notes.

## What the booted node runs

- `sw-agent` (as the unprivileged `secondwind` user): capability detection,
  pairing, mDNS advertisement, HTTPS+mTLS API, and the kiosk state file in
  `/run/secondwind/kiosk.json`.
- `sw-kiosk` on tty1 inside `cage` + `foot`: pairing screen (QR + PIN),
  paired idle screen, and the supervised streaming client.

Everything hardware-specific is detected at runtime; the image never bakes
in models, IPs, resolutions, codecs, or disk sizes. Building requires a
Debian environment (native, container, or WSL) — see `docs/SETUP-DEV.md`.
