# Node Image Scaffold

This folder will hold the Debian image configuration for the SecondWind node.

v0.1 target:

- minimal Debian base
- `sw-agent` systemd unit
- kiosk systemd unit
- capability-detection dependencies
- Moonlight/cage startup path
- Avahi/mDNS advertisement

The image must detect hardware at runtime and must not bake in machine-specific values from the first developer profile.
