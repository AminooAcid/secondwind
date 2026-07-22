use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeCapabilities {
    pub node_name: String,
    pub screen: ScreenCapabilities,
    /// Detected network interfaces (for Wake-on-LAN); defaulted so older
    /// peers still parse.
    #[serde(default)]
    pub network_interfaces: Vec<NetworkInterface>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkInterface {
    pub name: String,
    /// Colon-separated lowercase MAC, as read from the running system.
    pub mac_address: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScreenCapabilities {
    pub panel_modes: Vec<PanelMode>,
    pub decoders: Vec<VideoDecoderCapability>,
}

impl ScreenCapabilities {
    pub fn supports_h264_decode(&self) -> bool {
        self.decoders
            .iter()
            .any(|decoder| decoder.codec == VideoCodec::H264 && decoder.decode)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PanelMode {
    pub width_px: u32,
    pub height_px: u32,
    pub refresh_millihz: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoDecoderCapability {
    pub codec: VideoCodec,
    pub api: DecodeApi,
    pub render_device: Option<String>,
    pub decode: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VideoCodec {
    H264,
    Hevc,
    Av1,
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DecodeApi {
    VaApi,
    Vdpau,
    Software,
    Other(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_h264_decode_support() {
        let capabilities = ScreenCapabilities {
            panel_modes: Vec::new(),
            decoders: vec![VideoDecoderCapability {
                codec: VideoCodec::H264,
                api: DecodeApi::VaApi,
                render_device: Some("render-node".to_string()),
                decode: true,
            }],
        };

        assert!(capabilities.supports_h264_decode());
    }

    #[test]
    fn encode_without_decode_does_not_satisfy_h264_requirement() {
        let capabilities = ScreenCapabilities {
            panel_modes: Vec::new(),
            decoders: vec![VideoDecoderCapability {
                codec: VideoCodec::H264,
                api: DecodeApi::VaApi,
                render_device: None,
                decode: false,
            }],
        };

        assert!(!capabilities.supports_h264_decode());
    }
}
