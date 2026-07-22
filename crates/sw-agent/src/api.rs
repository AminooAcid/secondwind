use axum::{Json, Router, routing::get};
use sw_core::{
    CapabilitiesResponse, HealthResponse, NodeCapabilities, NodeUuid, ScreenCapabilities,
    ServiceStatus,
};

use crate::capability_detection::probe_vaapi_devices;

#[derive(Debug, Clone)]
pub struct AgentState {
    pub node_uuid: NodeUuid,
    pub capabilities: NodeCapabilities,
}

impl AgentState {
    pub fn detect(node_uuid: NodeUuid, node_name: String) -> Self {
        let decoders = probe_vaapi_devices()
            .into_iter()
            .filter_map(|probe| probe.to_decoder_capability())
            .collect();

        Self {
            node_uuid,
            capabilities: NodeCapabilities {
                node_name,
                screen: ScreenCapabilities {
                    panel_modes: Vec::new(),
                    decoders,
                },
            },
        }
    }

    pub fn service_status(&self) -> ServiceStatus {
        if self.capabilities.screen.supports_h264_decode() {
            ServiceStatus::Ready
        } else {
            ServiceStatus::Degraded
        }
    }
}

pub fn router(state: AgentState) -> Router {
    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/capabilities", get(capabilities))
        .with_state(state)
}

pub async fn health(
    axum::extract::State(state): axum::extract::State<AgentState>,
) -> Json<HealthResponse> {
    Json(health_response(&state))
}

pub async fn capabilities(
    axum::extract::State(state): axum::extract::State<AgentState>,
) -> Json<CapabilitiesResponse> {
    Json(capabilities_response(&state))
}

pub fn health_response(state: &AgentState) -> HealthResponse {
    HealthResponse {
        service: "sw-agent".to_string(),
        status: state.service_status(),
    }
}

pub fn capabilities_response(state: &AgentState) -> CapabilitiesResponse {
    CapabilitiesResponse {
        node_uuid: state.node_uuid.clone(),
        capabilities: state.capabilities.clone(),
    }
}

#[cfg(test)]
mod tests {
    use sw_core::{DecodeApi, PanelMode, VideoCodec, VideoDecoderCapability};

    use super::*;

    fn test_state(decoders: Vec<VideoDecoderCapability>) -> AgentState {
        AgentState {
            node_uuid: NodeUuid::new("node-for-api-tests").expect("valid node uuid"),
            capabilities: NodeCapabilities {
                node_name: "node".to_string(),
                screen: ScreenCapabilities {
                    panel_modes: vec![PanelMode {
                        width_px: 1,
                        height_px: 1,
                        refresh_millihz: 1,
                    }],
                    decoders,
                },
            },
        }
    }

    #[test]
    fn health_is_ready_when_h264_decode_exists() {
        let state = test_state(vec![VideoDecoderCapability {
            codec: VideoCodec::H264,
            api: DecodeApi::VaApi,
            render_device: Some("render-node".to_string()),
            decode: true,
        }]);

        assert_eq!(health_response(&state).status, ServiceStatus::Ready);
    }

    #[test]
    fn health_is_degraded_without_h264_decode() {
        let state = test_state(Vec::new());

        assert_eq!(health_response(&state).status, ServiceStatus::Degraded);
    }

    #[test]
    fn capabilities_response_uses_state_without_redetecting() {
        let state = test_state(Vec::new());

        assert_eq!(
            capabilities_response(&state).node_uuid.as_str(),
            "node-for-api-tests"
        );
        assert_eq!(capabilities_response(&state).capabilities.node_name, "node");
    }

    #[test]
    fn router_builds() {
        let _router = router(test_state(Vec::new()));
    }
}
