use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use sw_core::{DecodeApi, NetworkInterface, PanelMode, VideoCodec, VideoDecoderCapability};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaApiProbe {
    pub render_device: PathBuf,
    pub driver_name: Option<String>,
    pub h264_decode: bool,
}

impl VaApiProbe {
    pub fn to_decoder_capability(&self) -> Option<VideoDecoderCapability> {
        self.h264_decode.then(|| VideoDecoderCapability {
            codec: VideoCodec::H264,
            api: DecodeApi::VaApi,
            render_device: Some(self.render_device.display().to_string()),
            decode: true,
        })
    }
}

pub fn render_devices_in(dri_dir: impl AsRef<Path>) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dri_dir) else {
        return Vec::new();
    };

    let mut devices = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("renderD"))
        })
        .collect::<Vec<_>>();
    devices.sort();
    devices
}

pub fn probe_vaapi_devices() -> Vec<VaApiProbe> {
    render_devices_in("/dev/dri")
        .into_iter()
        .filter_map(|render_device| probe_vaapi_device(&render_device).ok())
        .collect()
}

pub fn probe_vaapi_device(render_device: &Path) -> Result<VaApiProbe, VaApiProbeError> {
    let output = Command::new("vainfo")
        .arg("--display")
        .arg("drm")
        .arg("--device")
        .arg(render_device)
        .output()
        .map_err(VaApiProbeError::Spawn)?;

    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));

    Ok(parse_vainfo(render_device.to_path_buf(), &text))
}

pub fn parse_vainfo(render_device: PathBuf, output: &str) -> VaApiProbe {
    VaApiProbe {
        render_device,
        driver_name: parse_driver_name(output),
        h264_decode: output.lines().any(is_h264_decode_line),
    }
}

fn parse_driver_name(output: &str) -> Option<String> {
    output.lines().find_map(|line| {
        let (_, path) = line.split_once("Trying to open ")?;
        path.rsplit(['/', '\\'])
            .next()
            .map(|driver| driver.trim().to_string())
    })
}

fn is_h264_decode_line(line: &str) -> bool {
    line.contains("VAProfileH264") && line.contains("VAEntrypointVLD")
}

#[derive(Debug)]
pub enum VaApiProbeError {
    Spawn(std::io::Error),
}

/// Detects connected panels' preferred modes from DRM connectors' EDID.
/// Internal panels (eDP/LVDS/DSI) sort first — the node's own screen is
/// what the virtual display should match. Nothing is assumed: no EDID, no
/// modes.
pub fn detect_panel_modes() -> Vec<PanelMode> {
    panel_modes_in("/sys/class/drm")
}

pub fn panel_modes_in(drm_dir: impl AsRef<Path>) -> Vec<PanelMode> {
    let Ok(entries) = fs::read_dir(drm_dir) else {
        return Vec::new();
    };

    let mut connectors: Vec<(bool, String, PathBuf)> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_str()?.to_string();
            // Connector dirs look like "card0-eDP-1"; skip bare devices.
            if !name.contains('-') {
                return None;
            }
            let path = entry.path();
            let connected = fs::read_to_string(path.join("status"))
                .map(|status| status.trim() == "connected")
                .unwrap_or(false);
            if !connected {
                return None;
            }
            let internal = ["eDP", "LVDS", "DSI"]
                .iter()
                .any(|kind| name.contains(kind));
            Some((internal, name, path))
        })
        .collect();
    // Internal panels first, then stable by name.
    connectors.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));

    connectors
        .into_iter()
        .filter_map(|(_, _, path)| {
            let edid = fs::read(path.join("edid")).ok()?;
            parse_edid_preferred_mode(&edid)
        })
        .collect()
}

/// Parses the EDID's first detailed timing descriptor (the panel's
/// preferred mode): resolution plus refresh derived from the pixel clock
/// and total raster size.
pub fn parse_edid_preferred_mode(edid: &[u8]) -> Option<PanelMode> {
    const DTD_OFFSET: usize = 54;
    let descriptor = edid.get(DTD_OFFSET..DTD_OFFSET + 18)?;

    let pixel_clock_10khz = u16::from_le_bytes([descriptor[0], descriptor[1]]) as u64;
    if pixel_clock_10khz == 0 {
        return None;
    }

    let h_active = (((descriptor[4] as u32) >> 4) << 8) | descriptor[2] as u32;
    let h_blank = (((descriptor[4] as u32) & 0x0F) << 8) | descriptor[3] as u32;
    let v_active = (((descriptor[7] as u32) >> 4) << 8) | descriptor[5] as u32;
    let v_blank = (((descriptor[7] as u32) & 0x0F) << 8) | descriptor[6] as u32;

    let h_total = (h_active + h_blank) as u64;
    let v_total = (v_active + v_blank) as u64;
    if h_active == 0 || v_active == 0 || h_total == 0 || v_total == 0 {
        return None;
    }

    let refresh_millihz = (pixel_clock_10khz * 10_000 * 1000 / (h_total * v_total)) as u32;
    Some(PanelMode {
        width_px: h_active,
        height_px: v_active,
        refresh_millihz,
    })
}

/// Detects physical network interfaces (name + MAC) for Wake-on-LAN.
/// Reads the running system; nothing is assumed about interface names.
pub fn detect_network_interfaces() -> Vec<NetworkInterface> {
    network_interfaces_in("/sys/class/net")
}

pub fn network_interfaces_in(net_dir: impl AsRef<Path>) -> Vec<NetworkInterface> {
    let Ok(entries) = fs::read_dir(net_dir) else {
        return Vec::new();
    };

    let mut interfaces: Vec<NetworkInterface> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_str()?.to_string();
            if name == "lo" {
                return None;
            }
            let mac_address = fs::read_to_string(entry.path().join("address"))
                .ok()?
                .trim()
                .to_lowercase();
            // Skip virtual interfaces without a real MAC.
            if mac_address.is_empty() || mac_address == "00:00:00:00:00:00" {
                return None;
            }
            Some(NetworkInterface { name, mac_address })
        })
        .collect();
    interfaces.sort_by(|a, b| a.name.cmp(&b.name));
    interfaces
}

#[cfg(test)]
mod tests {
    use std::{
        fs::File,
        path::{Path, PathBuf},
    };

    struct TempDirGuard(PathBuf);

    impl TempDirGuard {
        fn new(name: &str) -> Self {
            let root = std::env::temp_dir().join(format!(
                "secondwind-render-devices-{}-{name}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(&root).expect("create temp dir");
            Self(root)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    use super::*;

    #[test]
    fn parses_h264_decode_from_vainfo_output() {
        let output = r#"
libva info: Trying to open /usr/lib/x86_64-linux-gnu/dri/i965_drv_video.so
      VAProfileH264ConstrainedBaseline: VAEntrypointVLD
      VAProfileH264Main               : VAEntrypointEncSlice
"#;

        let probe = parse_vainfo(PathBuf::from("render-node-with-h264"), output);

        assert_eq!(probe.driver_name.as_deref(), Some("i965_drv_video.so"));
        assert!(probe.h264_decode);
        assert_eq!(
            probe.to_decoder_capability(),
            Some(VideoDecoderCapability {
                codec: VideoCodec::H264,
                api: DecodeApi::VaApi,
                render_device: Some("render-node-with-h264".to_string()),
                decode: true,
            })
        );
    }

    #[test]
    fn ignores_h264_encode_without_decode() {
        let output = r#"
      VAProfileH264Main               : VAEntrypointEncSlice
"#;

        let probe = parse_vainfo(PathBuf::from("render-node-without-h264"), output);

        assert!(!probe.h264_decode);
        assert_eq!(probe.to_decoder_capability(), None);
    }

    #[test]
    fn render_device_listing_is_sorted_and_filtered() {
        let root = TempDirGuard::new("sorted-filtered");

        File::create(root.path().join("card0")).expect("create card file");
        File::create(root.path().join("renderD_beta")).expect("create render file");
        File::create(root.path().join("renderD_alpha")).expect("create render file");
        File::create(root.path().join("not-render")).expect("create other file");

        let devices = render_devices_in(root.path());

        assert_eq!(
            devices,
            vec![
                root.path().join("renderD_alpha"),
                root.path().join("renderD_beta")
            ]
        );
    }

    /// 1920x1080@60: 148.5 MHz pixel clock, 2200x1125 total raster.
    fn edid_with_1080p_dtd() -> Vec<u8> {
        let mut edid = vec![0_u8; 128];
        let dtd = &mut edid[54..72];
        dtd[0..2].copy_from_slice(&14850_u16.to_le_bytes()); // 148.50 MHz
        dtd[2] = (1920 & 0xFF) as u8; // h active low
        dtd[3] = (280 & 0xFF) as u8; // h blank low
        dtd[4] = (((1920 >> 8) as u8) << 4) | ((280 >> 8) as u8);
        dtd[5] = (1080 & 0xFF) as u8; // v active low
        dtd[6] = 45; // v blank low
        dtd[7] = ((1080_u16 >> 8) as u8) << 4;
        edid
    }

    #[test]
    fn edid_preferred_mode_parses_resolution_and_refresh() {
        let mode = parse_edid_preferred_mode(&edid_with_1080p_dtd()).expect("mode");

        assert_eq!(mode.width_px, 1920);
        assert_eq!(mode.height_px, 1080);
        // 148_500_000 / (2200 * 1125) = 60.0 Hz.
        assert_eq!(mode.refresh_millihz, 60_000);
    }

    #[test]
    fn truncated_or_zeroed_edid_yields_no_mode() {
        assert!(parse_edid_preferred_mode(&[0_u8; 20]).is_none());
        assert!(parse_edid_preferred_mode(&[0_u8; 128]).is_none());
    }

    #[test]
    fn panel_scan_prefers_connected_internal_panels() {
        let root = TempDirGuard::new("panel-modes");
        let connector = |name: &str, status: &str, edid: Option<Vec<u8>>| {
            let dir = root.path().join(name);
            fs::create_dir_all(&dir).expect("connector dir");
            fs::write(dir.join("status"), format!("{status}\n")).expect("status");
            if let Some(edid) = edid {
                fs::write(dir.join("edid"), edid).expect("edid");
            }
        };
        connector("card0", "connected", None); // bare device dir: skipped
        connector(
            "card0-HDMI-A-1",
            "disconnected",
            Some(edid_with_1080p_dtd()),
        );
        connector("card0-eDP-1", "connected", Some(edid_with_1080p_dtd()));

        let modes = panel_modes_in(root.path());

        assert_eq!(modes.len(), 1);
        assert_eq!(modes[0].width_px, 1920);
    }

    #[test]
    fn network_interfaces_skip_loopback_and_empty_macs() {
        let root = TempDirGuard::new("net-interfaces");
        for (name, mac) in [
            ("lo", "00:00:00:00:00:00"),
            ("eth0", "AA:BB:CC:DD:EE:01"),
            ("wlan0", "aa:bb:cc:dd:ee:02"),
            ("veth-x", "00:00:00:00:00:00"),
        ] {
            let dir = root.path().join(name);
            fs::create_dir_all(&dir).expect("iface dir");
            fs::write(dir.join("address"), format!("{mac}\n")).expect("mac file");
        }

        let interfaces = network_interfaces_in(root.path());

        assert_eq!(interfaces.len(), 2);
        assert_eq!(interfaces[0].name, "eth0");
        assert_eq!(interfaces[0].mac_address, "aa:bb:cc:dd:ee:01");
        assert_eq!(interfaces[1].name, "wlan0");
    }

    #[test]
    fn driver_name_handles_windows_style_paths_for_tests() {
        let output = r"libva info: Trying to open iHD_drv_video.so";

        let probe = parse_vainfo(PathBuf::from("renderD128"), output);

        assert_eq!(probe.driver_name.as_deref(), Some("iHD_drv_video.so"));
    }
}
