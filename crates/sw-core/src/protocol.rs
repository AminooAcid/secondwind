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

/// Host → node disk control (v0.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiskCommandRequest {
    pub action: DiskAction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiskAction {
    Enable,
    Disable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiskStatusResponse {
    pub disk: DiskState,
    pub message: Option<String>,
}

/// Connection details for the node's exposed data disk. Only ever sent over
/// the paired mTLS channel; the CHAP secret is generated on the node and is
/// how the block layer authenticates the host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiskTarget {
    pub target_iqn: String,
    pub port: u16,
    pub chap_username: String,
    pub chap_secret: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum DiskState {
    NotPaired,
    /// The node has no designated data partition (or disk support is not
    /// configured on this image).
    Unavailable { reason: String },
    /// Configured but not currently exported.
    Ready,
    /// Exported and reachable at the returned target.
    Exposed { target: DiskTarget },
}

/// Seamless-apps status (v0.3). The session endpoint's password only ever
/// travels over the paired mTLS channel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppsStatusResponse {
    pub session: AppSessionState,
    pub apps: Vec<NodeAppInfo>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum AppSessionState {
    NotPaired,
    Unavailable { reason: String },
    Ready { endpoint: AppSessionEndpoint },
}

/// Where the host's seamless-window client attaches.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppSessionEndpoint {
    pub port: u16,
    pub password: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeAppInfo {
    pub app_id: String,
    pub installed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppLaunchRequest {
    /// Catalog identifier; the agent resolves it against its own whitelist
    /// and never executes host-supplied command lines.
    pub app_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppLaunchResponse {
    pub launched: bool,
    pub message: Option<String>,
}

/// Host → node file-share configuration (v0.3). Credentials are for the
/// dedicated share account the host created — never a user's own login.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShareConfigRequest {
    /// UNC path of the host share, e.g. `\\\\host-address\\SecondWind`.
    pub unc_path: String,
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShareStatusResponse {
    pub share: ShareState,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ShareState {
    NotPaired,
    Unavailable { reason: String },
    NotConfigured,
    Mounted,
}
