# Compatibility log

Every machine tested with SecondWind — working or not — goes here. Record what worked, what didn't, and any machine-specific gotchas. This is how we learn the real hardware range; the product must still *detect* capabilities, never assume them from this list.

Columns: role (host/node), machine, key specs, what was tested, result, notes.

---

## Nodes

| Machine | CPU / iGPU | Tested | Result | Notes |
|---|---|---|---|---|
| MSI laptop (first dev node) | i7-4702MQ (Haswell) / Intel HD 4600 | Phase 0 manual proof | passed | HD 4600 H.264 decode confirmed on `/dev/dri/renderD129` with `i965` (`VAEntrypointVLD`). Debian 13/Trixie minimal dual-boot installed alongside Windows 11 Pro. Moonlight apt package unavailable; used Flatpak fallback with `--device=dri` and `LIBVA_DRIVER_NAME=i965` overrides. See Appendix A in `FIRST-SETUP.md` and `PHASE0-TROUBLESHOOTING.md`. |
| MSI laptop (same node) | i7-4702MQ (Haswell) / Intel HD 4600 | v0.1 product install + pair (2026-07-22) | ✅ core passed | `install-node.sh` from public GitHub installed the full node; booted straight into the SecondWind kiosk (paired-idle ambient screen with clock + memory). mDNS discovery, PIN pairing, mTLS enforcement (no-cert connection rejected), and H.264 capability all confirmed from the host over the real network. **Fixes this surfaced** (all committed): Moonlight not packaged for trixie → Flatpak + `moonlight` wrapper; `xpra` gone from Debian archive → xpra.org repo; `libva-utils`→`vainfo`; eDP panel exposes a 0-byte EDID → DRM `modes` fallback; Debian `xpra-server.socket` collides on port 14500 → masked; GRUB defaulted to the node entry correctly. **Open:** idle RAM ~530–640 MB (target ~400) — Debian 13 `cage` session stack (pipewire/wireplumber/portals/ibus) + always-on xpra; fix is lazy-start xpra + trim session daemons (backlog). Screen streaming (extra monitor) not yet driven end-to-end. |

## Hosts

| Machine | CPU / GPU | Tested | Result | Notes |
|---|---|---|---|---|
| ASUS ZenBook Pro 15 UX535LI (first dev host) | i7 10th-gen / GTX 1650 Ti Max-Q (NVENC) | Phase 0 manual proof | passed | Apollo 0.4.7-alpha.1 installed and paired with Moonlight. Apollo showed a ViGEmBus/gamepad warning, which did not block screen proof. Extra display/extended desktop worked. Ethernet via a USB-C hub remains a link-detection edge case for later phases. See `PHASE0-TROUBLESHOOTING.md`. |

---

### How to update this log

After running `FIRST-SETUP.md` on a machine, change its **Result** to ✅ (extra screen worked) or ❌ (didn't), and add:
- which VA-API driver actually gave hardware H.264 decode (from Part C3),
- the negotiated codec if known,
- anything that needed a workaround (boot key, Secure Boot, shrink limits, firewall, etc.).

New machines are added here — do **not** extend `PROFILE-dev-machine.md` (that file is frozen to the first deployment).
