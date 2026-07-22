# Setting up your node (Git-based install)

This is the current way to turn an old laptop into a SecondWind node:
install plain Debian once, then run one command. Running the same command
again later **updates** the node — no USB re-flashing, ever.

> The flashable all-in-one ISO still exists (`node-image/`) and stays the
> plan for the polished 1.0; this path is easier while SecondWind evolves.

## Step 1 — Install Debian (one time)

Follow `docs/FIRST-SETUP.md`, steps A and B:

- **A** — free up space for Linux safely from inside Windows.
- **B** — install Debian (netinst, minimal: no desktop environment)
  alongside Windows. Dual boot stays; nothing of yours is deleted.

*Expected result: the laptop boots into a black Debian login screen
(text only — that's correct, SecondWind brings its own screen).*

## Step 2 — Run the SecondWind installer

1. Log in with the user you created during the Debian install.
2. Copy-paste these two lines and press Enter after each:

   ```sh
   wget -qO /tmp/install-node.sh https://raw.githubusercontent.com/AminooAcid/secondwind/main/scripts/node/install-node.sh
   sudo sh /tmp/install-node.sh
   ```

3. When asked, type `node` and press Enter.

*Expected result: seven numbered steps run (a few minutes — it downloads
packages and the SecondWind services), ending with "SecondWind node is
installed".*

4. Reboot:

   ```sh
   sudo reboot
   ```

*Expected result: the laptop boots straight into the SecondWind pairing
screen — a QR code and a 6-digit PIN. From here you continue in the
SecondWind app on your PC.*

## Updating the node later

Run the same two lines from Step 2 again. The installer pulls the latest
SecondWind from Git, updates the services, and keeps your settings.

## Options for advanced setups

Environment variables the installer understands (set before `sudo sh …`):

| Variable | Meaning |
|---|---|
| `SECONDWIND_BRANCH` | install from a branch other than `main` |
| `SECONDWIND_REPO_URL` | use a fork |
| `SECONDWIND_RELEASE_TAG` | pin a specific binaries release |
| `SECONDWIND_BUILD_FROM_SOURCE=1` | compile on the node instead of downloading binaries (needs Rust, slow on old hardware) |
| `SECONDWIND_ASSUME_YES=1` | skip the confirmation prompt (automation) |

## The disk feature on this install path

The data-disk feature activates when a partition labeled
`SECONDWIND_DATA` exists (the ISO's installer creates it automatically).
On a manual Debian install, create/label a spare partition with that
label and reboot — or skip it; everything else works without it.
