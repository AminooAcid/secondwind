# Upstream Projects

This file tracks upstream projects that SecondWind integrates. Versions are not pinned yet because v0.1 has only started scaffolding.

| Feature | Project | Status |
|---|---|---|
| Screen host | Apollo | Proven manually in Phase 0; not bundled yet |
| Screen node | Moonlight | Proven manually in Phase 0 via Flatpak fallback; not bundled yet |
| Kiosk shell | cage | Proven manually in Phase 0; not bundled yet |
| Discovery | Avahi / mDNS | Planned for v0.1 |
| Node OS image | Debian live-build | Planned for v0.1 |
| Host installer | Inno Setup | Planned |
| Disk | LIO iSCSI target | Later phase |
| USB | usbip / usbip-win2 | Later phase |
| Apps | xpra | Later phase |
| Jobs | Docker / outrun | Later phase |
| Windows app support | Wine | Later phase |

Before bundling any upstream binary or package, record:

- version
- license
- source URL
- verification method
- how SecondWind invokes it
