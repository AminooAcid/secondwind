# Developer machine profile — first test deployment

> **Scope warning:** This file describes ONE deployment: the first developer's two machines. It exists so setup guides can have a concrete tested appendix and so testing has known hardware. **Nothing here is a product requirement.** Never hardcode any value from this file into source code. If a value here matters to the product, the product must *detect* it or expose it as configuration. See `SECONDWIND_PLAN.md` §2.
>
> When other machines are tested, do not extend this file — add them to `docs/COMPATIBILITY.md` instead.

---

## Host — main PC

| Item | Value |
|---|---|
| Model | ASUS ZenBook Pro 15 (UX535LI) |
| CPU | Intel Core i7 (10th gen, 6-core, Comet Lake class) |
| GPU | NVIDIA GTX 1650 Ti Max-Q — has a hardware H.264/HEVC encoder (NVENC) |
| RAM | 16 GB |
| OS | Windows 11 Pro |
| Ethernet | No built-in port; uses a USB-C hub (Mcdodo) that provides Ethernet |
| Displays | Built-in panel + one existing external monitor (node becomes the third) |

Implications for testing: hardware encoding should be exercised on this machine; the USB-hub Ethernet path means link-up detection must tolerate an adapter that appears/disappears with the hub.

## Node — old laptop

| Item | Value |
|---|---|
| Brand | MSI |
| CPU | Intel Core i7-4702MQ (Haswell, 4 cores / 8 threads) |
| iGPU | Intel HD 4600 — **hardware-decodes H.264 only; no HEVC, no AV1** |
| dGPU | NVIDIA GT 720M — unused, irrelevant to this project |
| RAM | 8 GB |
| Primary disk | SSD, ~60 GB free, currently a single partition with Windows 11 Pro |
| Other disks | Additional HDD(s) present — **strictly off-limits: never partition, format, or write** |
| Existing OS | Windows 11 Pro — must be preserved (dual-boot) |

Implications for testing: this machine is the reason H.264 must always be a supported fallback — it is a good worst-case decode target. Its 8 GB RAM makes it a realistic test of the ≤400 MB idle-footprint goal.

## Planned partitioning for this node (first deployment only)

- Shrink free space on the SSD to create roughly **35 GB** for SecondWind: about 15 GB for the Linux system, the remainder as the node data partition exposed over iSCSI.
- Leave **≥25 GB free** for the existing Windows installation.
- Existing Windows partition is shrunk only — never reformatted, never removed.
- HDDs are not touched in any way.

## Network for this deployment

- Direct Ethernet cable between the host's USB-C hub and the node's built-in port.
- Convenience static addressing documented for the user: host `192.168.77.1`, node `192.168.77.2`, /24.
- The node retains its own Wi-Fi connection for downloading updates.
- **Product code must not assume any of this** — discovery is mDNS + UUID, and the transport must also work via a switch or Wi-Fi.

## Developer context

- The developer is a beginner with no coding background, working AI-assisted.
- All instructions produced for them must be copy-paste commands or numbered clicks, each with an expected-result line.
- Error messages and photos will be pasted back for diagnosis; never assume a step succeeded.
