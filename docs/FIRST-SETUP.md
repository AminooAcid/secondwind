# SecondWind — First Setup (Manual Proof)

> **What this document is.** Phase 0 of SecondWind. There is **no software to install from us yet** — this guide gets one old laptop working as an extra screen for your main PC *by hand*, using only mature, existing tools. When it works, we have proof the whole idea holds together, and later phases automate every step here.
>
> **Important product boundary.** The Apollo dashboard, Moonlight screens, Debian installer, terminal commands, Flatpak setup, driver checks, and manual network steps in this guide are **not** the future SecondWind user experience. They are Phase 0 proof steps only. The real product must wrap these upstream tools behind the SecondWind node image and Windows companion so users pair, connect, and use SecondWind without seeing these internals.
>
> **Future node-side experience.** A normal user should not clone git, install packages, edit Debian config, choose GPU drivers, run `cage`, or diagnose Apollo/Moonlight directly. They should install/flash the SecondWind node image and use the SecondWind Windows companion. The node image and companion must detect capabilities, configure upstream tools, supervise services, and show clear SecondWind-branded errors/logs when something needs attention.
>
> **Who this is for.** A complete beginner. Every step is either a **copy-paste command** or a **numbered click**, and each one ends with an **Expected result** line so you always know whether it worked before moving on. If a result does not match, **stop** and copy the exact error text (or take a photo of the screen) — do not guess your way forward.
>
> **Generic on purpose.** The main steps mention no specific laptop, IP address, or drive size, because they must work on *any* machine. Your exact numbers (how much space to free, which IP to type) live in the **appendix for your machine** at the bottom. The first developer's machines are already filled in as **Appendix A** — use it as the worked example.

---

## Terminology (used everywhere)

- **Host** — your main, everyday Windows PC. This is where all your files stay, always.
- **Node** — the old laptop you are converting. After this guide it dual-boots: its old OS **and** SecondWind's Linux.
- **Link** — the network connection between host and node (a direct Ethernet cable, a switch, or Wi-Fi).

## What you will have achieved at the end

You start Moonlight on the node, pick your host, and the node becomes a **third monitor** — you can drag a window from your main PC onto the old laptop's screen and use it normally. Unplug/stop it and Windows reflows its windows just like unplugging a real monitor.

## The five parts

- **Part A —** Free up space on the node's disk *safely*, using its existing OS.
- **Part B —** Install Debian minimal *alongside* the existing OS (dual-boot), Debian as the default.
- **Part C —** Install the screen software: decode drivers + Moonlight + kiosk on the node, Apollo on the host, then pair them.
- **Part D —** Set up the link between host and node.
- **Part E —** Verify the extra screen works.

---

## Before you begin — safety rules (read once, they apply to every step)

These are hard rules. The guide is written to obey them; you should refuse any step that seems to break one.

1. **Only ONE disk is ever touched** — the node's main system disk, and only its **free/unallocated** space. Any other drive inside the node (extra HDDs, second SSDs) is **strictly off-limits**: never partition, format, or write to it.
2. **The existing OS is preserved.** Its partition is only ever **shrunk**, never reformatted or deleted. You keep your old laptop's Windows exactly as it was, plus a boot menu.
3. **Every destructive action shows you exactly what it will change and asks for confirmation.** If a tool is about to erase or reformat anything and you did *not* expect it, cancel.
4. **Back up anything irreplaceable on the node first.** Shrinking a partition is low-risk, but "low" is not "zero." If the node holds anything you'd miss, copy it to the host before Part A.

## What you need to gather first

- The **node** laptop, charger, and a way to read its screen.
- A **USB flash drive, 4 GB or larger** — its contents will be erased when we make the Debian installer.
- The **host** Windows PC.
- An **Ethernet cable** (for the recommended direct link) and, if either machine lacks an Ethernet port, a USB-C/USB Ethernet adapter or hub.
- Internet access for both machines during install (to download software).
- **60–90 minutes** for the first run.

> **Fill in your machine's appendix before starting.** Scroll to *Appendix pattern* at the bottom, copy it, and fill in the four values it asks for (how much space to free, GPU/decoder, interface names, link IPs). Appendix A shows a completed example. Having these written down keeps the generic steps below unambiguous.

---

# Part A — Free space on the node's disk, safely

**Goal:** create empty (unallocated) space on the node's main disk for Debian, without harming the existing OS. We use the existing OS's *own* built-in tools, so nothing foreign touches the disk yet.

> These instructions assume the node currently runs **Windows** (the common case). If the node runs a different OS, use that OS's built-in partition tool to shrink its system partition and leave unallocated space — the rest of this guide is unchanged.

### A1. Turn off Fast Startup (so the disk is in a clean, safe state)

Windows "Fast Startup" leaves the disk in a half-hibernated state that makes partitioning and later dual-boot access unsafe.

1. Press **Windows key**, type `control panel`, open **Control Panel**.
2. Go to **Hardware and Sound → Power Options → Choose what the power buttons do**.
3. Click **Change settings that are currently unavailable**.
4. **Uncheck** *Turn on fast startup (recommended)*.
5. Click **Save changes**.

**Expected result:** the "Turn on fast startup" checkbox is now empty. (If it wasn't there at all, that's fine — this PC never had it on.)

### A2. Do a full shutdown

- Click **Start → Power → Shut down** (a *shutdown*, not a restart). Wait for the machine to power off completely, then turn it back on.

**Expected result:** the node boots back into its existing OS normally.

### A3. Decide how much space to free

- Read the **"Free for SecondWind"** value from your machine's appendix. As a rule of thumb, plan for **at least 30 GB**: roughly 15 GB for the Linux system and the rest as a data partition we'll expose to the host in a later phase.
- Make sure the existing OS keeps plenty of breathing room too (your appendix lists a minimum to leave free).

### A4. Shrink the existing partition using Disk Management

1. Press **Windows key**, type `diskmgmt.msc`, press **Enter**. The **Disk Management** window opens.
2. Find the disk that holds Windows (usually **Disk 0**) and its large **C:** partition.
   - **Sanity check:** confirm this is the *main system* disk, not an extra data drive. Extra drives are off-limits (safety rule 1).
3. **Right-click the C: partition → Shrink Volume…**
4. In **Enter the amount of space to shrink in MB**, type your target in megabytes (1 GB ≈ 1024 MB — e.g. 35 GB ≈ **35840**). Use the value from your appendix.
5. Click **Shrink**.

**Expected result:** after a moment, a block of **Unallocated** space (shown with a black bar) of roughly the size you asked for appears to the right of C:.

> **If the shrink amount offered is much smaller than you need:** immovable system files are blocking it. Common fixes, then retry the shrink:
> - Temporarily turn off hibernation: open **Command Prompt as Administrator** and run `powercfg /h off`.
> - Temporarily disable the page file and System Protection, reboot, shrink, then re-enable them.
> - If still stuck, note the biggest amount it *will* offer and record the exact number back to whoever is helping you — do not force anything.

**Do not create a partition in the unallocated space.** Leave it empty — the Debian installer will use it in Part B.

---

# Part B — Install Debian minimal alongside the existing OS (dual-boot)

**Goal:** install a lean Debian (no desktop) into the free space, keeping the old OS, with the boot menu **defaulting to Debian**.

### B1. Download the Debian installer

1. On the **host** (or any PC), open a browser to **https://www.debian.org/distrib/** and choose the **64-bit PC netinst** image (a file ending in `-amd64-netinst.iso`).
2. Save the `.iso` file.

**Expected result:** you have a file roughly 700 MB–1.5 GB named like `debian-XX.X.X-amd64-netinst.iso`.

### B2. Make a bootable USB installer

1. Download **Rufus** from **https://rufus.ie** (portable version is fine).
2. Insert your **USB flash drive** (remember: its contents will be erased).
3. Open Rufus:
   - **Device:** select your USB drive (double-check the size so you don't pick the wrong disk).
   - **Boot selection:** click **SELECT** and choose the Debian `.iso`.
   - Leave other settings at their defaults.
4. Click **START**. If asked about *ISO Image* vs *DD Image* mode, choose **ISO Image mode** (the recommended default). Confirm the erase warning.

**Expected result:** Rufus reaches **READY** (green bar). Your USB is now a Debian installer.

### B3. Boot the node from the USB

1. Plug the USB into the **node**. Also connect the node to the internet for the install (Ethernet to your router is simplest; Wi-Fi also works and can be entered during setup).
2. Power the node on and enter its **boot menu** — the key varies by brand (often **F12, F9, F10, Esc, or F2**). Your appendix records the correct key for your machine.
3. Choose the **USB drive** from the boot menu.

**Expected result:** the Debian installer's blue/grey menu appears.

> **If the USB won't boot:** enter the firmware/BIOS setup (usually **F2** or **Del** at power-on) and (a) make sure USB booting is enabled, and (b) if it still fails, temporarily turn **Secure Boot** off. Debian usually works with Secure Boot on, so try with it on first.

### B4. Run the installer (choose the minimal, dual-boot path)

Choose **Graphical install** and follow the prompts. The choices that matter:

1. **Language / location / keyboard:** pick yours.
2. **Network:** let it configure automatically over Ethernet, or select your Wi-Fi and enter the password.
3. **Hostname:** give the node a simple name (e.g. `secondwind-node`). Write it down — you'll see it when pairing.
4. **Users and passwords:** set a root password (or leave blank to use sudo), and create your user account. **Write these down.**
5. **Partitioning — this is the important one:**
   - Choose **Guided – use the largest continuous free space**.
   - This uses *only* the unallocated space you created in Part A. It does **not** touch the existing OS partition.
   - **Before you confirm, read the summary screen.** It must show your existing OS partition being **kept** (not formatted) and only the free space being used. If it proposes erasing the whole disk or reformatting your existing OS partition — **go back and choose the guided free-space option again.** (Safety rules 2 & 3.)
   - Confirm to write the changes.
6. **Software selection (keep it lean):** when you reach *Software selection*, **uncheck all desktop environments** (GNOME, KDE, etc.). Keep:
   - **SSH server** (checked)
   - **standard system utilities** (checked)
   - Everything else unchecked. This gives a minimal, no-desktop system — exactly what a lean node needs.
7. **GRUB boot loader:** when asked, choose to **install GRUB to your primary drive** (it will name the disk). This gives you the boot menu.

**Expected result:** the installer finishes and prompts you to remove the USB and reboot.

### B5. Confirm the dual-boot menu, with Debian as default

1. Remove the USB and let the node reboot.

**Expected result:** a **GRUB** boot menu appears. **Debian is the top entry and is selected by default** — if you wait, it boots Debian. Your existing OS should also be listed lower down.

2. Let it boot into Debian and log in with the user you created.

**Expected result:** a plain text login, then a command prompt. (No desktop — that's correct.)

### B6. Make sure the existing OS is listed in the boot menu

Newer Debian sometimes hides other operating systems from the menu. Turn detection on (this does **not** change the default entry):

```bash
sudo apt update
sudo apt install -y os-prober
echo 'GRUB_DISABLE_OS_PROBER=false' | sudo tee -a /etc/default/grub
sudo update-grub
```

**Expected result:** the last command prints a line mentioning your existing OS being **found** (e.g. `Found Windows Boot Manager`). Reboot once and confirm both entries appear in GRUB, with Debian still the default.

> From here on, the guide uses the terminal — that's normal for this manual proof. The finished SecondWind product will never ask a user to type commands; Phase 1 automates all of this into the image.

---

# Part C — Screen software: node + host, then pair

**Goal:** the node can decode a video stream (using its own hardware), run the Moonlight client fullscreen, and the host can broadcast its screen via Apollo. Then we pair the two.

## C-node: set up the node (run these on the node's terminal)

### C1. Update the system and detect the graphics hardware

```bash
sudo apt update && sudo apt upgrade -y
sudo apt install -y vainfo
```

Find out which GPU the node has, so you install the right decoder driver:

```bash
lspci | grep -Ei 'vga|3d|display'
```

**Expected result:** one or more lines naming the graphics chip (e.g. an Intel or AMD device). Note the vendor.

### C2. Install the hardware video-decode driver that matches the GPU

Video decoding (VA-API) is what lets an old laptop show a smooth stream without melting its CPU. Install the driver for **your** GPU — **detect, don't assume**:

- **Intel, 2014 or newer** (Broadwell and later):
  ```bash
  sudo apt install -y intel-media-va-driver-non-free
  ```
- **Intel, older than 2014** (Haswell, Ivy Bridge, and earlier — e.g. "HD 4600" and below):
  ```bash
  sudo apt install -y i965-va-driver
  ```
- **AMD:**
  ```bash
  sudo apt install -y mesa-va-drivers
  ```

> If you're unsure which Intel generation you have, install `i965-va-driver` first, check with the next step, and if H.264 decode isn't listed, switch to `intel-media-va-driver-non-free`.

### C3. Verify hardware H.264 decode is available

H.264 is the one codec SecondWind guarantees everywhere, because the oldest hardware only decodes H.264. Confirm the node can do it:

```bash
vainfo 2>/dev/null | grep -i h264
```

**Expected result:** at least one line containing `VAProfileH264` together with `VAEntrypointVLD` (the "VLD" entry point means hardware **decode**). If you get **no H.264 line at all**, revisit C2 and try the other Intel driver, then re-run this. Do not continue until H.264 decode shows up.

### C4. Install the Moonlight client and the kiosk compositor

Moonlight is the screen-receiving client; **cage** shows a single app fullscreen with no desktop around it (our kiosk).

```bash
# Moonlight (official package repository)
curl -1sLf 'https://dl.cloudsmith.io/public/moonlight-game-streaming/moonlight-qt/setup.deb.sh' | sudo -E bash
sudo apt update
sudo apt install -y moonlight-qt

# Kiosk compositor
sudo apt install -y cage
```

**Expected result:** both `moonlight-qt` and `cage` install without errors.

> **If the Moonlight repo step fails** (no `curl`, or network policy blocks it), install `curl` with `sudo apt install -y curl` and retry; or as a fallback use Flatpak: `sudo apt install -y flatpak && flatpak install -y flathub com.moonlight_stream.Moonlight` (then later launch it with `flatpak run com.moonlight_stream.Moonlight`).

*(We'll launch Moonlight in Part E, after the host side is ready.)*

## C-host: set up the host (do these on the Windows host)

### C5. Install Apollo

Apollo is a streaming host with a **built-in virtual display** — that's what makes the node appear as a *new* monitor rather than mirroring an existing one.

1. On the host, open a browser to the **Apollo releases page**: **https://github.com/ClassicOldSong/Apollo/releases**
2. Download the latest **Windows installer** (a file ending in `.exe`).
3. Run it, accept the defaults, and finish. Allow it through **Windows Firewall** if prompted (allow on **Private** networks at least).

**Expected result:** Apollo is installed and running (you'll see it in the system tray / Start menu).

### C6. Open Apollo's control panel and set credentials

1. Open **https://localhost:47990** in the host's browser.
2. Your browser will warn about the certificate (it's self-signed and local) — choose **Advanced → Continue to localhost**.
3. On first visit, **create a username and password** for Apollo. Write them down.

**Expected result:** you reach the Apollo dashboard.

### C7. Turn on the virtual display

1. In the Apollo dashboard, open **Configuration**.
2. Find the **virtual display** option (Apollo bundles its own virtual-display driver) and **enable** it. Save/Apply. Approve any Windows driver prompt.

**Expected result:** Apollo confirms the virtual display is enabled. (On some versions the virtual display is created automatically when a client connects — if there's no explicit toggle, that's fine.)

## C-pair: pair the node with the host

### C8. Start a pairing attempt from the node

On the node, launch Moonlight just to pair (a quick window is fine here):

```bash
cage -- moonlight-qt
```

*(If you used the Flatpak fallback: `cage -- flatpak run com.moonlight_stream.Moonlight`.)*

1. Make sure the node and host are on the **same network** for pairing (plug both into your router now if they aren't; we set up the dedicated link in Part D).
2. In Moonlight, your host PC should appear automatically. If it doesn't, click the **+** and type the host's IP address (find it on the host with `ipconfig` in Command Prompt).
3. Click the host. Moonlight displays a **4-digit PIN**.

**Expected result:** Moonlight shows a 4-digit PIN and says it's waiting to be paired.

### C9. Enter the PIN on the host

1. Back in the Apollo dashboard (**https://localhost:47990**), open the **PIN** tab.
2. Type the 4-digit PIN from the node and a device name, then **Send**.

**Expected result:** Moonlight on the node stops waiting and now shows the host with an app list (you'll see at least a **Desktop** entry). The two are paired.

---

# Part D — Set up the link

**Goal:** a reliable network path between host and node. Two options — start with whichever is easier for you.

### Option 1 — Same network (easiest for the first proof)

- Plug **both** host and node into the same home router or switch via Ethernet (or have both on the same Wi-Fi). Each gets an address automatically.

**Expected result:** in Moonlight on the node, the host is listed and reachable (as it was in C8).

### Option 2 — Direct cable with fixed addresses (recommended for daily use)

A direct Ethernet cable between host and node is faster and more private, but each side needs a fixed address on the same little private network. Use the two addresses from your appendix (an example pair: host `A.B.C.1`, node `A.B.C.2`, mask `/24`).

**On the host (Windows):**
1. Open **Settings → Network & Internet → (the Ethernet adapter for the direct cable) → Edit IP assignment**.
2. Switch to **Manual**, turn on **IPv4**, and enter the host address, subnet mask `255.255.255.0`, and leave gateway blank. Save.

**Expected result:** `ipconfig` on the host shows the address you set on that adapter.

**On the node (Debian):** find the wired interface name, then set a fixed address.
```bash
ip link        # note the wired interface name, e.g. enpXsY (NOT 'lo')
```
Edit the network config (replace `IFACE` with the name you found and use your node address):
```bash
sudo tee -a /etc/network/interfaces >/dev/null <<'EOF'

# SecondWind direct link (edit IFACE and address to match your appendix)
auto IFACE
iface IFACE inet static
    address A.B.C.2/24
EOF
sudo systemctl restart networking
```

**Expected result:** `ip addr show IFACE` lists the address you set. From the node, `ping A.B.C.1` (the host) replies. *(Make sure the host's firewall allows ping / the streaming ports on the Private network.)*

> The node can keep its Wi-Fi for internet/updates at the same time as using the wired cable for the link — they don't conflict.

---

# Part E — Verify the extra screen works

**Goal:** the node becomes a real third monitor.

1. Make sure the link from Part D is connected.
2. On the node, launch the client fullscreen in the kiosk:
   ```bash
   cage -- moonlight-qt
   ```
3. Select your **host**, then select **Desktop** to start streaming.

**Expected result:** the node's screen fills with your host's desktop, streamed live.

4. On the **host**, open **Settings → System → Display**. You should see an **additional display**. Set it to **Extend these displays** (not Duplicate) if it isn't already.

**Expected result:** the node now shows an *extension* of your desktop — a blank third desktop, not a copy of your main screen.

5. **Drag a window** from the host's main screen toward the side where the new display sits — it should slide onto the node's screen. Interact with it there.

**Expected result:** the window appears on the node and is usable. Latency is low enough for browsing, office work, and video.

6. **Stop the stream** (on the node, bring up the Moonlight overlay and quit, or close the session).

**Expected result:** the extra display disappears from the host and Windows reflows your windows back — exactly like unplugging a real monitor.

### ✅ Phase 0 acceptance

If you dragged a window onto the old laptop and used it, **Phase 0 is done**: the core idea is proven by hand. Record the outcome for your machine in [COMPATIBILITY.md](COMPATIBILITY.md).

### Quick troubleshooting

- **Host not visible in Moonlight:** confirm both are on the same network/link (Part D), and add the host manually by IP in Moonlight (`+`).
- **Stream is black or stutters badly:** re-check C3 (hardware H.264 decode must be present); make sure Apollo's virtual display is enabled (C7).
- **Node mirrors instead of extends:** set **Extend these displays** in host Display settings (step 4); confirm the virtual display, not an existing monitor, is the one being streamed.
- **Anything else:** copy the exact error text or photograph the screen and bring it back for diagnosis. Never assume a step passed.

---

# Appendix pattern (copy this for each machine)

Each physical deployment gets its own appendix so the generic steps above have concrete numbers. Copy the block below, rename it (Appendix B, C, …), and fill it in **before** you start. Machine-specific values live **only** here — never in the steps above and never in product code.

```
## Appendix X — <friendly name of this deployment>

Host
- Model / OS:
- Has a hardware video encoder? (yes/no, which):
- Ethernet: (built-in / via USB adapter or hub — note if it appears/disappears)

Node
- Model / CPU:
- GPU + which VA-API driver to install (from Part C2):
- Confirmed hardware H.264 decode? (yes/no — from Part C3):
- Boot-menu key (Part B3):
- Firmware setup key:

Part A — space
- Amount to free for SecondWind (GB / MB):
- Minimum free to leave for the existing OS (GB):
- Disks that are OFF-LIMITS (never touch):

Part D — link
- Method: (same network / direct cable)
- Host address:
- Node address + interface name:
- Subnet mask: 255.255.255.0 (/24)
```

---

## Appendix A — First developer deployment (worked example)

> Source: [PROFILE-dev-machine.md](PROFILE-dev-machine.md). These are one developer's real machines — a concrete, tested example. **Nothing here is a product requirement**; the steps above must work on any hardware.

**Host**
- Model / OS: ASUS ZenBook Pro 15 (UX535LI) — Windows 11 Pro.
- Hardware video encoder: **yes** — NVIDIA GTX 1650 Ti Max-Q (NVENC, H.264/HEVC). Exercise hardware encoding on this machine.
- Ethernet: **no built-in port** — provided by a USB-C hub (Mcdodo). Link-up detection must tolerate an adapter that appears/disappears with the hub.
- Displays: built-in panel + one existing external monitor; the node becomes the **third**.

**Node**
- Model / CPU: MSI laptop, Intel Core i7-4702MQ (Haswell, 4c/8t), 8 GB RAM.
- GPU + driver: Intel HD 4600 (Haswell = "older than 2014") → install **`i965-va-driver`** (Part C2, older-Intel branch).
- Hardware H.264 decode: **yes, H.264 only** — no HEVC, no AV1. This machine is exactly why H.264 must always be the fallback; a good worst-case decode target.
- dGPU: NVIDIA GT 720M — **unused, ignore it** (never depend on it or install proprietary drivers for it).
- Existing OS: Windows 11 Pro — preserve via dual-boot (shrink only).

**Part A — space**
- Free for SecondWind: **~35 GB** (≈ **35840 MB** in the Disk Management box) — about 15 GB for the Linux system, the rest as the node data partition (exposed over iSCSI in a later phase).
- Leave **≥ 25 GB free** for the existing Windows install.
- **OFF-LIMITS:** the additional internal **HDD(s)** — never partition, format, or write to them. Only the main SSD (and only its free space) is touched.

**Part D — link**
- Method: **direct Ethernet cable** between the host's USB-C hub and the node's built-in port.
- Host address: **192.168.77.1**
- Node address: **192.168.77.2** (set on the node's wired interface — find the exact name with `ip link`)
- Subnet mask: **255.255.255.0** (`/24`)
- The node keeps its own **Wi-Fi** for downloading updates at the same time.

**Notes for testing on this deployment**
- 8 GB RAM makes the node a realistic test of the ≤ 400 MB idle-footprint goal (a later-phase target).
- The developer is a beginner working AI-assisted: keep every instruction copy-paste or numbered clicks with an expected result, and paste back exact errors/photos rather than assuming success.
