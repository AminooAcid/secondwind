# Phase 0 Troubleshooting Notes

Date: 2026-07-21

Scope: first manual proof run on the first developer host/node pair. These notes record the real problems encountered while running `docs/FIRST-SETUP.md`, the fixes that worked, and follow-up items for future docs and v0.1 automation.

This file is machine-test evidence, not product requirements. Do not hardcode hardware-specific values from this run into source code.

## Final Result

Phase 0 manual screen proof passed:

- Debian minimal was installed on the node alongside Windows.
- The node booted Debian.
- SSH from the Windows host to the node worked.
- Intel VA-API H.264 hardware decode was confirmed on the node.
- Apollo on the host paired with Moonlight on the node.
- The host desktop streamed successfully to the node laptop.
- Windows eventually showed the node as an extra display and the setup worked as an extended desktop.

## Problems And Fixes

### Windows Shrink Size Was Confusing

Symptom:

- Windows Disk Management reported `154110 MB` available to shrink.
- The planned shrink amount was `35840 MB`.

Fix:

- Shrunk by `35840 MB`, not the maximum.

Reason:

- `35840 MB` is about 35 GB, matching Appendix A.
- The larger number was only the maximum possible shrink amount, not the amount we wanted to take.

Future note:

- Keep emphasizing that the shrink box asks for "amount to remove from Windows", not "final Windows size".

### USB Install Seemed Easier

Symptom:

- User asked whether Debian could be installed onto a flash drive instead of the internal SSD.

Fix:

- Continued with the internal SSD dual-boot plan.

Reason:

- The internal SSD is faster and more reliable.
- USB flash installs add boot-order and wear/failure issues.
- The Windows partition had already been safely shrunk, leaving unallocated space for Debian.

Future note:

- The guide should continue recommending internal SSD dual-boot for Phase 0.

### Debian Partitioning Asked "Primary Or Logical"

Symptom:

- The installer asked whether to create a primary or logical partition.

Fix:

- Backed out of manual partitioning and used:

```text
Guided - use the largest continuous free space
```

Reason:

- The beginner-safe path is the guided free-space option.
- This avoids accidentally touching the existing Windows partition or extra HDDs.

Future note:

- Add a warning in `FIRST-SETUP.md`: if the installer asks "Primary or logical", the user is probably in manual partitioning and should go back.

### Software Selection Included Desktop Options

Symptom:

- Debian offered desktop environments: GNOME, Xfce, KDE Plasma, Cinnamon, MATE, LXDE, LXQt, and others.

Fix:

- Installed only:

```text
SSH server
standard system utilities
```

Reason:

- SecondWind needs a lean no-desktop node.
- Desktop packages can be added later if needed.

Future note:

- The no-desktop instruction was correct.

### Reboot Loaded Windows Instead Of Debian

Symptom:

- After Debian install and USB removal, the node appeared to boot Windows.

Fix:

- Used MSI boot menu with:

```text
F11
```

- Chose the internal SSD:

```text
ADATA SU650
```

Reason:

- Windows Boot Manager was likely first in firmware boot order.
- Debian was installed, but the firmware did not automatically boot it first.

Future note:

- Record this as a boot-order gotcha for this machine.
- v0.1 image/install flow should verify boot target after install.

### Debian Login Was Unclear

Symptom:

- The console showed a Debian login prompt and the user did not know which login/password to use.

Fix:

- Used the normal Debian username and password created during install.

Reason:

- Debian console login is the normal user account, not the host Windows account.
- Password input shows no dots/stars, which is normal.

Future note:

- `FIRST-SETUP.md` should explicitly say "Debian will not show password characters while typing."

### `sudo` Was Missing

Symptom:

```text
sudo: command not found
```

Fix:

Logged in as root:

```bash
su -
```

Then ran:

```bash
apt update
apt install -y sudo
ls /home
usermod -aG sudo monarch
reboot
```

Reason:

- Debian was installed with a separate root account, so `sudo` was not installed/configured for the normal user by default.

Future note:

- The guide should include a branch for "sudo command not found".
- v0.1 image should create/administer privileges without this manual ambiguity.

### GRUB Needed OS Prober

Symptom:

- The guide required ensuring Windows appeared in GRUB.

Fix:

Installed and enabled `os-prober`:

```bash
apt install -y os-prober
echo 'GRUB_DISABLE_OS_PROBER=false' >> /etc/default/grub
update-grub
```

Expected result:

- `update-grub` finds Windows Boot Manager.

Future note:

- This remains a valid Debian 13/Trixie dual-boot step.

### ACPI Errors Appeared During Boot

Symptom:

- ACPI errors appeared during Debian boot.

Fix:

- Waited to see whether Debian reached login.
- Since Debian reached login, no change was made.

Reason:

- ACPI warnings/errors are common on older laptops and are not necessarily fatal.

Future note:

- The guide should say ACPI messages are only a blocker if boot hangs for more than about two minutes.

### NVIDIA Nouveau Errors Appeared

Symptom:

Messages like:

```text
nouveau ... MMIO write ... FAULT ... PRIVRING
msvid ... unable to load firmware data
```

Fix:

- Ignored during Phase 0 because the stream decode path uses the Intel iGPU, not the NVIDIA dGPU.

Reason:

- The NVIDIA GT 720M-class dGPU is not needed for SecondWind.
- The Intel iGPU is the intended decoder path.

Future note:

- If nouveau errors cause freezes later, consider documenting a blacklist workaround for this specific test node.
- Do not make NVIDIA assumptions in product code.

### H.264 Decode Was Not Initially Obvious

Symptom:

- `vainfo` on `/dev/dri/renderD128` opened the Nouveau driver and did not show the desired Intel H.264 decode path.

Discovery:

```bash
lspci | grep -Ei 'vga|3d|display'
```

Showed:

- Intel 4th-gen integrated graphics.
- NVIDIA GT 720M-class 3D controller.

Fix:

- Checked both render devices.
- `/dev/dri/renderD128` was NVIDIA/Nouveau.
- `/dev/dri/renderD129` was Intel.

Working check:

```bash
vainfo --display drm --device /dev/dri/renderD129 | grep -i h264
```

Confirmed:

```text
VAProfileH264ConstrainedBaseline: VAEntrypointVLD
VAProfileH264Main               : VAEntrypointVLD
VAProfileH264High               : VAEntrypointVLD
```

Reason:

- This machine has two GPU render devices.
- H.264 decode must be checked on the Intel render node, not the NVIDIA one.

Future note:

- v0.1 capability detection must enumerate render devices and select the one with H.264 `VAEntrypointVLD`.
- Do not assume `renderD128` is the correct GPU.

### Non-Free Apt Sections Were Missing

Symptom:

```text
Package i965-va-driver-shaders is not available
Error: Package 'i965-va-driver-shaders' has no installation candidate
```

Discovery:

```bash
cat /etc/apt/sources.list
```

Showed active Debian sources with:

```text
main non-free-firmware
```

but missing:

```text
contrib non-free
```

Fix:

```bash
sudo cp /etc/apt/sources.list /etc/apt/sources.list.backup
sudo sed -i 's/ main non-free-firmware/ main contrib non-free non-free-firmware/g' /etc/apt/sources.list
sudo apt update
sudo apt install -y i965-va-driver-shaders
```

Reason:

- The needed older Intel VA-API shader package is outside the default `main non-free-firmware` set.

Future note:

- The setup guide should include enabling `contrib non-free non-free-firmware` before installing older Intel VA-API support.
- v0.1 image build should include the needed repository components and drivers.

### Apollo Showed ViGEmBus Error

Symptom:

Apollo dashboard showed:

```text
Fatal: ViGEmBus is not installed or running. You must install ViGEmBus for gamepad support!
```

Fix:

- Continued without fixing it.

Reason:

- ViGEmBus is for gamepad support.
- Phase 0 screen proof does not require gamepad input.

Future note:

- This warning is not a blocker for screen-only proof.

### `moonlight-qt` Apt Package Was Unavailable

Symptom:

```text
Error: Unable to locate package moonlight-qt
```

Fix:

- Used the documented Flatpak fallback:

```bash
sudo apt install -y cage flatpak
sudo flatpak remote-add --if-not-exists flathub https://flathub.org/repo/flathub.flatpakrepo
sudo flatpak install -y flathub com.moonlight_stream.Moonlight
flatpak list | grep -i moonlight
```

Confirmed:

```text
Moonlight com.moonlight_stream.Moonlight 6.1.0 stable system
```

Reason:

- The Moonlight apt repo/package path did not provide `moonlight-qt` for this setup.

Future note:

- Keep Flatpak fallback in Phase 0 docs.
- v0.1 image build should pin and verify the actual Moonlight packaging path.

### `cage` Could Not Spawn The Flatpak Command

Symptoms:

First:

```text
failed to spawn client: no such file or directory
```

Then after creating a launcher:

```text
failed to spawn client: permission denied
```

Fix:

Verified installed paths:

```bash
which cage
which flatpak
flatpak list | grep -i moonlight
```

Created a launcher:

```bash
mkdir -p ~/.local/bin
nano ~/.local/bin/start-moonlight
chmod 755 /home/monarch/.local/bin/start-moonlight
```

Launcher contents:

```bash
#!/bin/sh
exec /usr/bin/flatpak run com.moonlight_stream.Moonlight
```

Launched through `/bin/sh`:

```bash
/usr/bin/dbus-run-session -- /usr/bin/cage -- /bin/sh /home/monarch/.local/bin/start-moonlight
```

Reason:

- `cage` needed an executable client path.
- Calling the script through `/bin/sh` avoided execute/permission ambiguity.
- `dbus-run-session` provided a cleaner user session for Flatpak/Moonlight.

Future note:

- Phase 0 docs should use the full working launch command for Flatpak Moonlight under cage.
- v0.1 should ship a supervised kiosk script instead of asking the user to type this.

### Running Moonlight Over SSH Failed

Symptom:

Running the Moonlight launcher from SSH produced:

```text
Cannot create window: no screens available
```

Fix:

- Ran Moonlight from the physical node laptop console instead of SSH.

Reason:

- SSH sessions do not own the physical display.
- `cage`/Moonlight must run on the node's local console for this manual proof.

Future note:

- The guide should be explicit: install/configure commands can be run over SSH, but Moonlight kiosk launch must be run on the physical node screen.

### Moonlight Warned No Hardware Decoder Was Detected

Symptom:

Moonlight opened but warned:

```text
No functioning hardware accelerated video decoder was detected by Moonlight.
```

Fix:

Gave the Flatpak GPU access and forced the Intel VA-API driver:

```bash
sudo flatpak override --system --device=dri com.moonlight_stream.Moonlight
sudo flatpak override --system --env=LIBVA_DRIVER_NAME=i965 com.moonlight_stream.Moonlight
```

Reason:

- System `vainfo` had already confirmed Intel H.264 decode.
- The issue was Flatpak/Moonlight access or driver selection, not missing hardware support.

Future note:

- Phase 0 Flatpak fallback should include these overrides on older Intel systems.
- v0.1 should avoid relying on Flatpak-specific runtime quirks if possible.

### Quitting Moonlight Was Not Obvious

Symptom:

- Keyboard shortcut `Ctrl+Alt+Shift+Q` did not exit the stream.

Fix:

Used SSH from the host:

```bash
pkill -f Moonlight
```

Fallbacks:

```bash
pkill -f moonlight
pkill -f flatpak
```

Reason:

- Fullscreen kiosk mode can trap beginner users if the normal overlay shortcut does not work.

Future note:

- Phase 0 docs need an "escape hatch" section for quitting the stream over SSH.
- v0.1 kiosk should provide a reliable local exit/recover path for development builds.

### Stream Worked But Quality Was Initially Low

Symptom:

- The stream worked, but the quality looked low.

Likely causes:

- Moonlight bitrate/resolution/FPS defaults.
- Apollo encoder settings.
- Network path may matter, but router/modem Ethernet is not automatically bad if both devices are wired.

Fix:

- No final tuning was recorded in this session.

Future note:

- Add a quality tuning section after the base proof:
  - Moonlight resolution
  - Moonlight FPS
  - Moonlight bitrate
  - Apollo encoder selection
  - wired direct-link vs router/switch path

## Commands Worth Preserving

Confirm Intel H.264 decode on this node:

```bash
vainfo --display drm --device /dev/dri/renderD129 | grep -i h264
```

Working Flatpak Moonlight kiosk launch:

```bash
/usr/bin/dbus-run-session -- /usr/bin/cage -- /bin/sh /home/monarch/.local/bin/start-moonlight
```

Working Flatpak hardware decode overrides:

```bash
sudo flatpak override --system --device=dri com.moonlight_stream.Moonlight
sudo flatpak override --system --env=LIBVA_DRIVER_NAME=i965 com.moonlight_stream.Moonlight
```

Quit Moonlight remotely:

```bash
pkill -f Moonlight
```

## Documentation Follow-Ups

- Add a branch for `sudo: command not found`.
- Add a warning that "Primary or logical" means the user is probably in manual partitioning.
- Add Debian apt source instructions for `contrib non-free non-free-firmware`.
- Add render-device enumeration guidance; do not assume `renderD128`.
- Keep Flatpak as the Moonlight fallback and include Flatpak GPU overrides.
- Add a "run commands over SSH, launch Moonlight on physical node" note.
- Add an SSH kill command as the fullscreen escape hatch.
- Add quality tuning steps after acceptance.

## v0.1 Automation Notes

- Apollo/Moonlight/Debian UI must not be exposed as the normal user experience; they are upstream internals behind the SecondWind node image and Windows companion.
- End users should receive a node image/installer and Windows companion, not instructions to clone git or install libraries on the node. Git-based setup is for developers and contributors only.
- Node-side failures should be diagnosed by SecondWind checks and logs with clear user-facing messages, while detailed Apollo/Moonlight/Debian logs remain available for support/debugging.
- Capability detection must enumerate VA-API devices and select a render node with H.264 `VAEntrypointVLD`.
- The node image should include the needed Intel VA-API driver packages for older hardware.
- The kiosk launch should be a first-party supervised script/service, not a hand-typed Flatpak/cage command.
- The host companion should make virtual-display use explicit and verifiable.
- The setup flow should record boot-order/GRUB state after install.
