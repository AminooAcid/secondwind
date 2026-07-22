use std::sync::{Arc, RwLock};

use axum::{Json, Router, routing::get};
use sw_core::{
    CapabilitiesResponse, HealthResponse, NodeCapabilities, NodeUuid, PairingRequest,
    PairingResponse, PairingStatusResponse, ScreenCapabilities, ServiceStatus,
    agent_api::{CAPABILITIES_PATH, HEALTH_PATH, PAIRING_PATH},
};

use crate::{
    capability_detection::probe_vaapi_devices,
    pairing_state::{PairingState, unavailable_pairing},
};

#[derive(Debug, Clone)]
pub struct AgentState {
    pub node_uuid: NodeUuid,
    pub capabilities: NodeCapabilities,
    pub pairing: Arc<RwLock<PairingState>>,
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
            pairing: Arc::new(RwLock::new(unavailable_pairing())),
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
        .route(HEALTH_PATH, get(health))
        .route(CAPABILITIES_PATH, get(capabilities))
        .route(PAIRING_PATH, get(pairing_status).post(submit_pairing))
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

pub async fn pairing_status(
    axum::extract::State(state): axum::extract::State<AgentState>,
) -> Json<PairingStatusResponse> {
    Json(pairing_status_response(&state))
}

pub async fn submit_pairing(
    axum::extract::State(state): axum::extract::State<AgentState>,
    Json(request): Json<PairingRequest>,
) -> Json<PairingResponse> {
    Json(submit_pairing_request(&state, request))
}

pub fn health_response(state: &AgentState) -> HealthResponse {
    HealthResponse {
        service: "sw-agent".to_string(),
        status: state.service_status(),
    }
}

pub fn capabilities_response(state: &AgentState) -> CapabilitiesResponse {
    CapabilitiesResponse {
        node_uuid: state.node_uuid,
        capabilities: state.capabilities.clone(),
    }
}

pub fn pairing_status_response(state: &AgentState) -> PairingStatusResponse {
    state
        .pairing
        .read()
        .expect("pairing state lock should not be poisoned")
        .status_response()
}

pub fn submit_pairing_request(state: &AgentState, request: PairingRequest) -> PairingResponse {
    state
        .pairing
        .write()
        .expect("pairing state lock should not be poisoned")
        .submit_request(request)
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, RwLock};

    use sw_core::{
        DecodeApi, PairingOffer, PairingPin, PairingStatus, PanelMode, VideoCodec,
        VideoDecoderCapability,
    };

    use super::*;
    use crate::pairing_state::PairingState;

    fn test_state(decoders: Vec<VideoDecoderCapability>) -> AgentState {
        AgentState {
            node_uuid: NodeUuid::new("00000000-0000-4000-8000-000000000002")
                .expect("valid node uuid"),
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
            pairing: Arc::new(RwLock::new(PairingState::Unavailable {
                reason: "test".to_string(),
            })),
        }
    }

    fn waiting_pairing_state() -> PairingState {
        PairingState::Waiting {
            offer: PairingOffer {
                node_uuid: NodeUuid::new("00000000-0000-4000-8000-000000000004")
                    .expect("valid uuid"),
                node_name: "node".to_string(),
                certificate_fingerprint: "fingerprint".to_string(),
                pin: PairingPin::new("123456").expect("valid pin"),
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
            capabilities_response(&state).node_uuid.to_string(),
            "00000000-0000-4000-8000-000000000002"
        );
        assert_eq!(capabilities_response(&state).capabilities.node_name, "node");
    }

    #[test]
    fn pairing_status_reports_unavailable_by_default() {
        let state = test_state(Vec::new());

        assert_eq!(
            pairing_status_response(&state).status,
            PairingStatus::Unavailable
        );
    }

    #[test]
    fn pairing_request_can_accept_waiting_offer() {
        let state = test_state(Vec::new());
        *state.pairing.write().expect("lock") = waiting_pairing_state();

        let response = submit_pairing_request(
            &state,
            PairingRequest {
                host_name: "host".to_string(),
                host_certificate_fingerprint: "host-fingerprint".to_string(),
                pin: PairingPin::new("123456").expect("valid pin"),
            },
        );

        assert!(response.accepted);
        assert_eq!(
            pairing_status_response(&state).status,
            PairingStatus::Paired
        );
    }

    #[test]
    fn router_builds() {
        let _router = router(test_state(Vec::new()));
    }
}
