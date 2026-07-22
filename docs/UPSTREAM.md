# Upstream Projects

Every upstream SecondWind integrates, how it is invoked, and where its
version gets pinned. **Pin rule:** a concrete version + checksum is
recorded here the moment a binary/package is first bundled into a shipped
artifact (installer or ISO); until the first hardware-validated release,
Debian-packaged upstreams float on the image's Debian release.

| Feature | Project | License | How SecondWind invokes it | Version source |
|---|---|---|---|---|
| Screen (host) | Apollo (Sunshine fork, built-in virtual display) | GPL-3.0 | separate process; managed config block, localhost API for PIN arming, Windows service | bundled by installer — pin on first release |
| Screen (node) | Moonlight (moonlight-qt) | GPL-3.0 | `moonlight pair` / `moonlight stream` under kiosk supervision | Moonlight apt repo (Cloudsmith), suite = image Debian codename |
| Kiosk shell | cage + foot | MIT | systemd unit runs `cage -- foot --fullscreen sw-kiosk` | Debian |
| Discovery | Avahi (mDNS) | LGPL-2.1 | avahi-daemon on the node; mdns-sd crate speaks the protocol | Debian |
| Disk | LIO / targetcli-fb | Apache-2.0 | `targetcli` via the polkit-scoped `secondwind-disk` unit | Debian |
| Disk (host) | Windows iSCSI initiator | Windows built-in | PowerShell `*-Iscsi*` cmdlets | Windows |
| Apps | xpra | GPL-2.0+ | node session unit + `xpra control start`; host `xpra attach` | Debian (node); bundled client (host) — pin on first release |
| Files | Windows SMB server + Linux CIFS | built-in / kernel | dedicated share account; `mount -t cifs` via the polkit-scoped unit | Windows / Debian |
| USB | usbip (node), usbip-win2 (host) | GPL-2.0 | `usbipd` unit + sudoers-scoped bind wrapper; host `usbip.exe attach/detach` | Debian / bundled — pin on first release |
| Jobs | Docker Engine | Apache-2.0 | `docker run --rm` per preset, share bind-mounted at `/data`, network off for file jobs | Debian (`docker.io`) |
| Job images | linuxserver/ffmpeg, alpine, curlimages/curl | various | preset file `/etc/secondwind/jobs.json` | pin digests on first release |
| Node OS | Debian (current stable) + live-build | DFSG | `node-image/live-build/build.sh` | `SECONDWIND_DEBIAN_DIST` |
| Host installer | Inno Setup | modified BSD | `installer/innosetup/secondwind.iss` | pin on first release |
| CLI offload | outrun | MIT | documented for advanced users only, not in the UI | not bundled |
| Windows apps on node | Wine | LGPL-2.1 | future tier behind a feature flag | not bundled |

## Bundle checklist (fill per bundled binary at release time)

- exact version
- license file included in the artifact
- source URL
- SHA-256 of the downloaded artifact
- how SecondWind invokes it (must remain a separate process)

## Keys

- Moonlight apt repository signing key: fetched at image build from
  Cloudsmith (`build.sh` step 3). Record its fingerprint here on the first
  release build.
