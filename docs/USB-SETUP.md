# USB — one-time host driver setup

SecondWind's USB feature makes devices plugged into the node appear in the
host's Device Manager, using usbip-win2. Its kernel driver is the fiddliest
install step on the host, so the installer bundles it and this page
documents exactly what happens (and how to fix it when Windows objects).

## What the installer does

1. Copies the usbip-win2 client (`usbip.exe`) and its driver package next
   to the SecondWind companion.
2. Installs the `usbip2_filter`/`usbip2_ude` drivers. These are signed by
   the usbip-win2 project's certificate, **not** by Microsoft WHQL.

## If the driver refuses to install

Windows requires the signer's certificate to be trusted for kernel
drivers. Steps (numbered clicks, one-time, administrator needed):

1. Press **Windows key**, type `powershell`, right-click **Windows
   PowerShell** → **Run as administrator**.
   *Expected: a blue window titled Administrator: Windows PowerShell.*
2. Paste the line below and press **Enter** (adjust the path if SecondWind
   is installed elsewhere):

   ```powershell
   & "$env:ProgramFiles\SecondWind\usbip\classic_setup.ps1"
   ```

   *Expected: the script reports the certificate was added to the trusted
   stores and the drivers installed.*
3. Plug a USB stick into the node, open SecondWind → your node → **USB
   devices on this node** → **Attach**.
   *Expected: the device appears in File Explorer within a few seconds.*

If step 2 reports a signature error instead, take a photo of the exact
message and send it to your support contact — do **not** disable Secure
Boot or test signing unless support asks.

## Product boundary

Normal use never mentions usbip: users click **Attach**/**Detach** in the
companion, and "Always attach" makes a device follow the node connection
automatically. This page exists only because kernel-driver trust is a
Windows-level, one-time step.

## Pinning

The exact bundled usbip-win2 release and its driver signing details are
recorded in `docs/UPSTREAM.md` and `installer/innosetup/`.
