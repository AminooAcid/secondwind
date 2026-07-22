use sw_agent::capability_detection::probe_vaapi_devices;
use sw_core::{
    HealthResponse, NodeCapabilities, ScreenCapabilities, ServiceStatus, VideoDecoderCapability,
};

fn main() {
    let decoders = probe_vaapi_devices()
        .into_iter()
        .filter_map(|probe| probe.to_decoder_capability())
        .collect::<Vec<VideoDecoderCapability>>();

    let capabilities = NodeCapabilities {
        node_name: "unconfigured-node".to_string(),
        screen: ScreenCapabilities {
            panel_modes: Vec::new(),
            decoders,
        },
    };

    let health = HealthResponse {
        service: "sw-agent".to_string(),
        status: if capabilities.screen.supports_h264_decode() {
            ServiceStatus::Ready
        } else {
            ServiceStatus::Degraded
        },
    };

    println!(
        "{}",
        serde_json::to_string(&health).expect("health response should serialize")
    );
}
