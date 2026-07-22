use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use sw_core::{DecodeApi, VideoCodec, VideoDecoderCapability};

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

#[cfg(test)]
mod tests {
    use std::fs::File;

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

        let probe = parse_vainfo(PathBuf::from("/dev/dri/renderD128"), output);

        assert!(!probe.h264_decode);
        assert_eq!(probe.to_decoder_capability(), None);
    }

    #[test]
    fn render_device_listing_is_sorted_and_filtered() {
        let root =
            std::env::temp_dir().join(format!("secondwind-render-devices-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create temp dir");

        File::create(root.join("card0")).expect("create card file");
        File::create(root.join("renderD_beta")).expect("create render file");
        File::create(root.join("renderD_alpha")).expect("create render file");
        File::create(root.join("not-render")).expect("create other file");

        let devices = render_devices_in(&root);

        assert_eq!(
            devices,
            vec![root.join("renderD_alpha"), root.join("renderD_beta")]
        );

        fs::remove_dir_all(&root).expect("remove temp dir");
    }

    #[test]
    fn driver_name_handles_windows_style_paths_for_tests() {
        let output = r"libva info: Trying to open iHD_drv_video.so";

        let probe = parse_vainfo(PathBuf::from("renderD128"), output);

        assert_eq!(probe.driver_name.as_deref(), Some("iHD_drv_video.so"));
    }
}
