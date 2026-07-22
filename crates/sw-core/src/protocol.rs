use serde::{Deserialize, Serialize};

use crate::{NodeCapabilities, NodeUuid};

pub const API_V1_PREFIX: &str = "/v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthResponse {
    pub service: String,
    pub status: ServiceStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServiceStatus {
    Starting,
    Ready,
    Degraded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilitiesResponse {
    pub node_uuid: NodeUuid,
    pub capabilities: NodeCapabilities,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub code: String,
    pub message: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeatureToggleRequest {
    pub node_uuid: NodeUuid,
    pub feature: FeatureKind,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FeatureKind {
    Screen,
}

/// Host → node screen control.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScreenCommandRequest {
    pub action: ScreenAction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ScreenAction {
    /// Bring the node screen up. The node connects its streaming client back
    /// to the requesting host. `stream_pair_pin` is a one-shot PIN the host
    /// has armed on its streaming server, sent only while that inner pairing
    /// does not exist yet.
    Connect { stream_pair_pin: Option<String> },
    Disconnect,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScreenStatusResponse {
    pub screen: ScreenState,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ScreenState {
    NotPaired,
    Idle,
    Streaming { host_address: String },
}
