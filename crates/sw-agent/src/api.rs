use std::{
    net::SocketAddr,
    path::PathBuf,
    sync::{Arc, RwLock},
};

use axum::{Json, Router, extract::ConnectInfo, routing::get};
use sw_core::{
    AppLaunchRequest, AppLaunchResponse, AppSessionState, AppsStatusResponse,
    CapabilitiesResponse, DiskAction, DiskCommandRequest, DiskState, DiskStatusResponse,
    HealthResponse, KioskState, NodeCapabilities, NodeUuid, PairingRequest, PairingResponse,
    PairingStatusResponse, ScreenAction, ScreenCapabilities, ScreenCommandRequest, ScreenState,
    ScreenStatusResponse, ServiceStatus, ShareConfigRequest, ShareState, ShareStatusResponse,
    UsbAction, UsbCommandRequest, UsbDevicesResponse, UsbState,
    agent_api::{
        APPS_PATH, CAPABILITIES_PATH, DISK_PATH, HEALTH_PATH, PAIRING_PATH, SCREEN_PATH,
        SHARE_PATH, USB_PATH,
    },
    kiosk::write_kiosk_state,
};

use crate::{
    apps::{SharedAppsController, app_infos},
    capability_detection::probe_vaapi_devices,
    disk::{SharedDiskController, disk_state},
    identity::{PairedHostTrust, persist_paired_host},
    pairing_state::{PairingState, unavailable_pairing},
    share::{SharedShareController, share_state},
    usb::SharedUsbController,
};

#[derive(Debug, Clone)]
pub struct AgentState {
    pub node_uuid: NodeUuid,
    pub capabilities: NodeCapabilities,
    pub pairing: Arc<RwLock<PairingState>>,
    pub identity_store: Option<AgentIdentityStore>,
    pub screen: Arc<RwLock<ScreenState>>,
    /// One-shot inner streaming pairing PIN, held only while a connect
    /// request carries one; consumed by the kiosk supervisor.
    pub stream_pair_pin: Arc<RwLock<Option<String>>>,
    pub kiosk_state_file: Option<PathBuf>,
    /// v0.2 disk exposure; None when the image has no disk support wired.
    pub disk: Option<SharedDiskController>,
    /// v0.3 seamless apps; None when the image has no app session wired.
    pub apps: Option<SharedAppsController>,
    /// v0.3 host share mount; None when the image has no share support.
    pub share: Option<SharedShareController>,
    /// v0.4 USB export; None when the image has no usbip support.
    pub usb: Option<SharedUsbController>,
}

impl AgentState {
    pub fn detect(node_uuid: NodeUuid, node_name: String) -> Self {
        Self::detect_with_pairing(node_uuid, node_name, unavailable_pairing())
    }

    pub fn detect_with_pairing(
        node_uuid: NodeUuid,
        node_name: String,
        pairing: PairingState,
    ) -> Self {
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
                network_interfaces: crate::capability_detection::detect_network_interfaces(),
            },
            pairing: Arc::new(RwLock::new(pairing)),
            identity_store: None,
            screen: Arc::new(RwLock::new(ScreenState::Idle)),
            stream_pair_pin: Arc::new(RwLock::new(None)),
            kiosk_state_file: None,
            disk: None,
            apps: None,
            share: None,
            usb: None,
        }
    }

    pub fn with_disk_controller(mut self, disk: Option<SharedDiskController>) -> Self {
        self.disk = disk;
        self
    }

    pub fn with_apps_controller(mut self, apps: Option<SharedAppsController>) -> Self {
        self.apps = apps;
        self
    }

    pub fn with_share_controller(mut self, share: Option<SharedShareController>) -> Self {
        self.share = share;
        self
    }

    pub fn with_usb_controller(mut self, usb: Option<SharedUsbController>) -> Self {
        self.usb = usb;
        self
    }

    pub fn with_identity_store(mut self, state_file: PathBuf) -> Self {
        self.identity_store = Some(AgentIdentityStore { state_file });
        self
    }

    pub fn with_kiosk_state_file(mut self, kiosk_state_file: Option<PathBuf>) -> Self {
        self.kiosk_state_file = kiosk_state_file;
        self
    }

    /// Projects the current pairing + screen state onto the kiosk state file.
    /// Failures are logged, never fatal: the API must stay up even if the
    /// kiosk display file is momentarily unwritable.
    pub fn sync_kiosk(&self) {
        let Some(kiosk_state_file) = &self.kiosk_state_file else {
            return;
        };

        let kiosk_state = self.kiosk_projection();
        if let Err(error) = write_kiosk_state(kiosk_state_file, &kiosk_state) {
            eprintln!("sw-agent: {error}");
        }
    }

    fn kiosk_projection(&self) -> KioskState {
        let pairing = self
            .pairing
            .read()
            .expect("pairing state lock should not be poisoned");

        match &*pairing {
            PairingState::Unavailable { .. } => KioskState::Starting,
            PairingState::Waiting { offer } => KioskState::Unpaired {
                node_name: offer.node_name.clone(),
                pin: offer.pin.expose_for_pairing_display().to_string(),
                qr_payload: sw_core::PairingQrPayload::from_offer(offer)
                    .to_json()
                    .unwrap_or_default(),
                certificate_fingerprint: offer.certificate_fingerprint.clone(),
            },
            PairingState::Paired { host_name } => {
                let screen = self
                    .screen
                    .read()
                    .expect("screen state lock should not be poisoned");
                match &*screen {
                    ScreenState::Streaming { host_address } => KioskState::Streaming {
                        paired_host_name: host_name.clone(),
                        host_address: host_address.clone(),
                        stream_pair_pin: self.pending_stream_pair_pin(),
                    },
                    _ => KioskState::Idle {
                        node_name: self.capabilities.node_name.clone(),
                        paired_host_name: host_name.clone(),
                    },
                }
            }
        }
    }

    fn pending_stream_pair_pin(&self) -> Option<String> {
        self.stream_pair_pin
            .read()
            .expect("stream pin lock should not be poisoned")
            .clone()
    }

    pub fn service_status(&self) -> ServiceStatus {
        if self.capabilities.screen.supports_h264_decode() {
            ServiceStatus::Ready
        } else {
            ServiceStatus::Degraded
        }
    }
}

#[derive(Debug, Clone)]
pub struct AgentIdentityStore {
    state_file: PathBuf,
}

impl AgentIdentityStore {
    pub fn persist_paired_host(
        &self,
        paired_host: PairedHostTrust,
    ) -> Result<(), crate::identity::IdentityStoreError> {
        persist_paired_host(&self.state_file, paired_host).map(|_| ())
    }
}

pub fn router(state: AgentState) -> Router {
    Router::new()
        .route(HEALTH_PATH, get(health))
        .route(CAPABILITIES_PATH, get(capabilities))
        .route(PAIRING_PATH, get(pairing_status).post(submit_pairing))
        .route(SCREEN_PATH, get(screen_status).post(screen_command))
        .route(DISK_PATH, get(disk_status).post(disk_command))
        .route(APPS_PATH, get(apps_status).post(launch_app))
        .route(SHARE_PATH, get(share_status).post(configure_share))
        .route(USB_PATH, get(usb_devices).post(usb_command))
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

pub async fn screen_status(
    axum::extract::State(state): axum::extract::State<AgentState>,
) -> Json<ScreenStatusResponse> {
    Json(screen_status_response(&state))
}

pub async fn screen_command(
    axum::extract::State(state): axum::extract::State<AgentState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Json(request): Json<ScreenCommandRequest>,
) -> Json<ScreenStatusResponse> {
    Json(apply_screen_command(&state, peer.ip().to_string(), request))
}

pub async fn disk_status(
    axum::extract::State(state): axum::extract::State<AgentState>,
) -> Json<DiskStatusResponse> {
    Json(disk_status_response(&state))
}

pub async fn disk_command(
    axum::extract::State(state): axum::extract::State<AgentState>,
    Json(request): Json<DiskCommandRequest>,
) -> Json<DiskStatusResponse> {
    Json(apply_disk_command(&state, request))
}

pub async fn apps_status(
    axum::extract::State(state): axum::extract::State<AgentState>,
) -> Json<AppsStatusResponse> {
    Json(apps_status_response(&state))
}

pub async fn launch_app(
    axum::extract::State(state): axum::extract::State<AgentState>,
    Json(request): Json<AppLaunchRequest>,
) -> Json<AppLaunchResponse> {
    Json(apply_app_launch(&state, request))
}

pub async fn usb_devices(
    axum::extract::State(state): axum::extract::State<AgentState>,
) -> Json<UsbDevicesResponse> {
    Json(usb_devices_response(&state))
}

pub async fn usb_command(
    axum::extract::State(state): axum::extract::State<AgentState>,
    Json(request): Json<UsbCommandRequest>,
) -> Json<UsbDevicesResponse> {
    Json(apply_usb_command(&state, request))
}

pub async fn share_status(
    axum::extract::State(state): axum::extract::State<AgentState>,
) -> Json<ShareStatusResponse> {
    Json(share_status_response(&state))
}

pub async fn configure_share(
    axum::extract::State(state): axum::extract::State<AgentState>,
    Json(request): Json<ShareConfigRequest>,
) -> Json<ShareStatusResponse> {
    Json(apply_share_config(&state, request))
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
    let result = state
        .pairing
        .read()
        .expect("pairing state lock should not be poisoned")
        .prepare_submit_request(request);

    let Some(paired_host) = result.paired_host else {
        return result.response;
    };

    if let Some(identity_store) = &state.identity_store {
        if identity_store
            .persist_paired_host(paired_host.clone())
            .is_err()
        {
            return PairingResponse {
                accepted: false,
                node_certificate_fingerprint: result.response.node_certificate_fingerprint,
            };
        }
    }

    state
        .pairing
        .write()
        .expect("pairing state lock should not be poisoned")
        .mark_paired(paired_host.host_name);
    state.sync_kiosk();

    result.response
}

pub fn screen_status_response(state: &AgentState) -> ScreenStatusResponse {
    let paired = matches!(
        &*state
            .pairing
            .read()
            .expect("pairing state lock should not be poisoned"),
        PairingState::Paired { .. }
    );

    if !paired {
        return ScreenStatusResponse {
            screen: ScreenState::NotPaired,
            message: Some("This node is not paired with a host yet.".to_string()),
        };
    }

    ScreenStatusResponse {
        screen: state
            .screen
            .read()
            .expect("screen state lock should not be poisoned")
            .clone(),
        message: None,
    }
}

pub fn apply_screen_command(
    state: &AgentState,
    peer_address: String,
    request: ScreenCommandRequest,
) -> ScreenStatusResponse {
    let paired = matches!(
        &*state
            .pairing
            .read()
            .expect("pairing state lock should not be poisoned"),
        PairingState::Paired { .. }
    );
    if !paired {
        return ScreenStatusResponse {
            screen: ScreenState::NotPaired,
            message: Some("Pair with this node before using the screen.".to_string()),
        };
    }

    match request.action {
        ScreenAction::Connect { stream_pair_pin } => {
            *state
                .stream_pair_pin
                .write()
                .expect("stream pin lock should not be poisoned") = stream_pair_pin;
            *state
                .screen
                .write()
                .expect("screen state lock should not be poisoned") = ScreenState::Streaming {
                host_address: peer_address,
            };
        }
        ScreenAction::Disconnect => {
            *state
                .stream_pair_pin
                .write()
                .expect("stream pin lock should not be poisoned") = None;
            *state
                .screen
                .write()
                .expect("screen state lock should not be poisoned") = ScreenState::Idle;
        }
    }

    state.sync_kiosk();
    screen_status_response(state)
}

fn is_paired(state: &AgentState) -> bool {
    matches!(
        &*state
            .pairing
            .read()
            .expect("pairing state lock should not be poisoned"),
        PairingState::Paired { .. }
    )
}

pub fn disk_status_response(state: &AgentState) -> DiskStatusResponse {
    if !is_paired(state) {
        return DiskStatusResponse {
            disk: DiskState::NotPaired,
            message: Some("This node is not paired with a host yet.".to_string()),
        };
    }

    match &state.disk {
        None => DiskStatusResponse {
            disk: DiskState::Unavailable {
                reason: "This node has no SecondWind data disk configured.".to_string(),
            },
            message: None,
        },
        Some(controller) => DiskStatusResponse {
            disk: disk_state(controller.as_ref()),
            message: None,
        },
    }
}

pub fn apply_disk_command(state: &AgentState, request: DiskCommandRequest) -> DiskStatusResponse {
    if !is_paired(state) {
        return DiskStatusResponse {
            disk: DiskState::NotPaired,
            message: Some("Pair with this node before using the disk.".to_string()),
        };
    }

    let Some(controller) = &state.disk else {
        return DiskStatusResponse {
            disk: DiskState::Unavailable {
                reason: "This node has no SecondWind data disk configured.".to_string(),
            },
            message: None,
        };
    };

    let result = match request.action {
        DiskAction::Enable => controller.export(),
        DiskAction::Disable => controller.unexport(),
    };

    match result {
        Ok(()) => DiskStatusResponse {
            disk: disk_state(controller.as_ref()),
            message: None,
        },
        Err(error) => DiskStatusResponse {
            disk: disk_state(controller.as_ref()),
            message: Some(error.to_string()),
        },
    }
}

pub fn apps_status_response(state: &AgentState) -> AppsStatusResponse {
    if !is_paired(state) {
        return AppsStatusResponse {
            session: AppSessionState::NotPaired,
            apps: Vec::new(),
            message: Some("This node is not paired with a host yet.".to_string()),
        };
    }

    let Some(controller) = &state.apps else {
        return AppsStatusResponse {
            session: AppSessionState::Unavailable {
                reason: "This node has no app session configured.".to_string(),
            },
            apps: Vec::new(),
            message: None,
        };
    };

    let session = match controller.session_endpoint() {
        Some(endpoint) => AppSessionState::Ready { endpoint },
        None => AppSessionState::Unavailable {
            reason: "The node's app session is not ready yet.".to_string(),
        },
    };

    AppsStatusResponse {
        session,
        apps: app_infos(controller.as_ref()),
        message: None,
    }
}

pub fn apply_app_launch(state: &AgentState, request: AppLaunchRequest) -> AppLaunchResponse {
    if !is_paired(state) {
        return AppLaunchResponse {
            launched: false,
            message: Some("Pair with this node before launching apps.".to_string()),
        };
    }

    let Some(controller) = &state.apps else {
        return AppLaunchResponse {
            launched: false,
            message: Some("This node has no app session configured.".to_string()),
        };
    };

    // Whitelist resolution: only the node's own catalog is executable.
    let Some(entry) = controller
        .catalog()
        .into_iter()
        .find(|entry| entry.app_id == request.app_id)
    else {
        return AppLaunchResponse {
            launched: false,
            message: Some(format!(
                "{} is not in this node's app library.",
                request.app_id
            )),
        };
    };

    if !controller.is_installed(&entry.node_command) {
        return AppLaunchResponse {
            launched: false,
            message: Some(format!(
                "{} is not installed on this node.",
                entry.display_name
            )),
        };
    }

    match controller.launch(&entry) {
        Ok(()) => AppLaunchResponse {
            launched: true,
            message: None,
        },
        Err(error) => AppLaunchResponse {
            launched: false,
            message: Some(error.to_string()),
        },
    }
}

pub fn usb_devices_response(state: &AgentState) -> UsbDevicesResponse {
    if !is_paired(state) {
        return UsbDevicesResponse {
            usb: UsbState::NotPaired,
            devices: Vec::new(),
            message: Some("This node is not paired with a host yet.".to_string()),
        };
    }

    match &state.usb {
        None => UsbDevicesResponse {
            usb: UsbState::Unavailable {
                reason: "This node has no USB sharing configured.".to_string(),
            },
            devices: Vec::new(),
            message: None,
        },
        Some(controller) if !controller.available() => UsbDevicesResponse {
            usb: UsbState::Unavailable {
                reason: "This node has no USB sharing configured.".to_string(),
            },
            devices: Vec::new(),
            message: None,
        },
        Some(controller) => UsbDevicesResponse {
            usb: UsbState::Ready,
            devices: controller.devices(),
            message: None,
        },
    }
}

pub fn apply_usb_command(state: &AgentState, request: UsbCommandRequest) -> UsbDevicesResponse {
    if !is_paired(state) {
        return UsbDevicesResponse {
            usb: UsbState::NotPaired,
            devices: Vec::new(),
            message: Some("Pair with this node before sharing USB devices.".to_string()),
        };
    }

    let Some(controller) = &state.usb else {
        return UsbDevicesResponse {
            usb: UsbState::Unavailable {
                reason: "This node has no USB sharing configured.".to_string(),
            },
            devices: Vec::new(),
            message: None,
        };
    };

    let result = match &request.action {
        UsbAction::Bind { bus_id } => controller.bind(bus_id),
        UsbAction::Unbind { bus_id } => controller.unbind(bus_id),
    };

    UsbDevicesResponse {
        usb: UsbState::Ready,
        devices: controller.devices(),
        message: result.err().map(|error| error.to_string()),
    }
}

pub fn share_status_response(state: &AgentState) -> ShareStatusResponse {
    if !is_paired(state) {
        return ShareStatusResponse {
            share: ShareState::NotPaired,
            message: Some("This node is not paired with a host yet.".to_string()),
        };
    }

    match &state.share {
        None => ShareStatusResponse {
            share: ShareState::Unavailable {
                reason: "This node cannot mount the host share.".to_string(),
            },
            message: None,
        },
        Some(controller) => ShareStatusResponse {
            share: share_state(controller.as_ref()),
            message: None,
        },
    }
}

pub fn apply_share_config(state: &AgentState, request: ShareConfigRequest) -> ShareStatusResponse {
    if !is_paired(state) {
        return ShareStatusResponse {
            share: ShareState::NotPaired,
            message: Some("Pair with this node before sharing files.".to_string()),
        };
    }

    let Some(controller) = &state.share else {
        return ShareStatusResponse {
            share: ShareState::Unavailable {
                reason: "This node cannot mount the host share.".to_string(),
            },
            message: None,
        };
    };

    match controller.configure_and_mount(&request) {
        Ok(()) => ShareStatusResponse {
            share: share_state(controller.as_ref()),
            message: None,
        },
        Err(error) => ShareStatusResponse {
            share: share_state(controller.as_ref()),
            message: Some(error.to_string()),
        },
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::{Arc, RwLock},
    };

    use sw_core::{
        DecodeApi, PairingOffer, PairingPin, PairingStatus, PanelMode, VideoCodec,
        VideoDecoderCapability,
    };

    use super::*;
    use crate::{
        identity::{AgentIdentity, PairedHostTrust, load_or_create_identity, write_identity},
        pairing_state::PairingState,
    };

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
                network_interfaces: Vec::new(),
            },
            pairing: Arc::new(RwLock::new(PairingState::Unavailable {
                reason: "test".to_string(),
            })),
            identity_store: None,
            screen: Arc::new(RwLock::new(ScreenState::Idle)),
            stream_pair_pin: Arc::new(RwLock::new(None)),
            kiosk_state_file: None,
            disk: None,
            apps: None,
            share: None,
            usb: None,
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
    fn detect_with_pairing_preserves_runtime_pairing_state() {
        let pairing = waiting_pairing_state();
        let state = AgentState::detect_with_pairing(
            NodeUuid::new("00000000-0000-4000-8000-000000000004").expect("valid uuid"),
            "node".to_string(),
            pairing,
        );

        assert_eq!(
            pairing_status_response(&state).status,
            PairingStatus::WaitingForHost
        );
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
                host_certificate_pem: "host certificate".to_string(),
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
    fn pairing_request_persists_host_trust_before_marking_paired() {
        let state_file = std::env::temp_dir().join(format!(
            "secondwind-api-identity-{}.json",
            std::process::id()
        ));
        let _ = fs::remove_file(&state_file);
        write_identity(
            &state_file,
            &AgentIdentity {
                node_uuid: NodeUuid::new("00000000-0000-4000-8000-000000000004")
                    .expect("valid uuid"),
                node_name: "node".to_string(),
                paired_host: None,
            },
        )
        .expect("write identity");
        let mut state = test_state(Vec::new()).with_identity_store(state_file.clone());
        state.pairing = Arc::new(RwLock::new(waiting_pairing_state()));

        let response = submit_pairing_request(
            &state,
            PairingRequest {
                host_name: "host".to_string(),
                host_certificate_fingerprint: "host-fingerprint".to_string(),
                host_certificate_pem: "host certificate".to_string(),
                pin: PairingPin::new("123456").expect("valid pin"),
            },
        );
        let identity = load_or_create_identity(&state_file, "unused").expect("load identity");
        let _ = fs::remove_file(&state_file);

        assert!(response.accepted);
        assert_eq!(
            identity.paired_host.expect("paired host"),
            PairedHostTrust {
                host_name: "host".to_string(),
                host_certificate_fingerprint: "host-fingerprint".to_string(),
                host_certificate_pem: Some("host certificate".to_string()),
            }
        );
    }

    #[test]
    fn router_builds() {
        let _router = router(test_state(Vec::new()));
    }

    #[test]
    fn screen_command_requires_pairing() {
        let state = test_state(Vec::new());

        let response = apply_screen_command(
            &state,
            "peer-address".to_string(),
            ScreenCommandRequest {
                action: ScreenAction::Connect {
                    stream_pair_pin: None,
                },
            },
        );

        assert_eq!(response.screen, ScreenState::NotPaired);
    }

    #[test]
    fn screen_connect_streams_to_requesting_peer_and_updates_kiosk() {
        let kiosk_file = std::env::temp_dir().join(format!(
            "secondwind-api-kiosk-{}.json",
            std::process::id()
        ));
        let _ = fs::remove_file(&kiosk_file);
        let mut state = test_state(Vec::new()).with_kiosk_state_file(Some(kiosk_file.clone()));
        state.pairing = Arc::new(RwLock::new(PairingState::paired("host".to_string())));

        let response = apply_screen_command(
            &state,
            "peer-address".to_string(),
            ScreenCommandRequest {
                action: ScreenAction::Connect {
                    stream_pair_pin: Some("0000".to_string()),
                },
            },
        );
        let kiosk_state = sw_core::kiosk::read_kiosk_state(&kiosk_file).expect("kiosk state");
        let _ = fs::remove_file(&kiosk_file);

        assert_eq!(
            response.screen,
            ScreenState::Streaming {
                host_address: "peer-address".to_string()
            }
        );
        assert_eq!(
            kiosk_state,
            sw_core::KioskState::Streaming {
                paired_host_name: "host".to_string(),
                host_address: "peer-address".to_string(),
                stream_pair_pin: Some("0000".to_string()),
            }
        );
    }

    #[test]
    fn screen_disconnect_returns_to_idle_and_clears_stream_pin() {
        let kiosk_file = std::env::temp_dir().join(format!(
            "secondwind-api-kiosk-disconnect-{}.json",
            std::process::id()
        ));
        let _ = fs::remove_file(&kiosk_file);
        let mut state = test_state(Vec::new()).with_kiosk_state_file(Some(kiosk_file.clone()));
        state.pairing = Arc::new(RwLock::new(PairingState::paired("host".to_string())));
        apply_screen_command(
            &state,
            "peer-address".to_string(),
            ScreenCommandRequest {
                action: ScreenAction::Connect {
                    stream_pair_pin: Some("0000".to_string()),
                },
            },
        );

        let response = apply_screen_command(
            &state,
            "peer-address".to_string(),
            ScreenCommandRequest {
                action: ScreenAction::Disconnect,
            },
        );
        let kiosk_state = sw_core::kiosk::read_kiosk_state(&kiosk_file).expect("kiosk state");
        let _ = fs::remove_file(&kiosk_file);

        assert_eq!(response.screen, ScreenState::Idle);
        assert!(state.pending_stream_pair_pin().is_none());
        assert_eq!(
            kiosk_state,
            sw_core::KioskState::Idle {
                node_name: "node".to_string(),
                paired_host_name: "host".to_string(),
            }
        );
    }

    #[test]
    fn disk_requires_pairing() {
        let state = test_state(Vec::new());

        let response = apply_disk_command(
            &state,
            sw_core::DiskCommandRequest {
                action: sw_core::DiskAction::Enable,
            },
        );

        assert_eq!(response.disk, sw_core::DiskState::NotPaired);
    }

    #[test]
    fn disk_without_controller_is_unavailable() {
        let mut state = test_state(Vec::new());
        state.pairing = Arc::new(RwLock::new(PairingState::paired("host".to_string())));

        let response = disk_status_response(&state);

        assert!(matches!(
            response.disk,
            sw_core::DiskState::Unavailable { .. }
        ));
    }

    #[test]
    fn disk_enable_exposes_target_with_chap_over_paired_channel() {
        use crate::disk::test_support::{FakeDiskController, provisioned};

        let mut state = test_state(Vec::new()).with_disk_controller(Some(Arc::new(
            FakeDiskController {
                provisioning: Some(provisioned()),
                ..Default::default()
            },
        )));
        state.pairing = Arc::new(RwLock::new(PairingState::paired("host".to_string())));

        assert_eq!(disk_status_response(&state).disk, sw_core::DiskState::Ready);

        let enabled = apply_disk_command(
            &state,
            sw_core::DiskCommandRequest {
                action: sw_core::DiskAction::Enable,
            },
        );
        let sw_core::DiskState::Exposed { target } = enabled.disk else {
            panic!("expected exposed disk");
        };
        assert_eq!(target.target_iqn, "iqn.2026-07.app.secondwind:node-test");
        assert_eq!(target.chap_username, "secondwind");

        let disabled = apply_disk_command(
            &state,
            sw_core::DiskCommandRequest {
                action: sw_core::DiskAction::Disable,
            },
        );
        assert_eq!(disabled.disk, sw_core::DiskState::Ready);
    }

    #[test]
    fn apps_launch_rejects_unknown_and_uninstalled_apps() {
        use crate::apps::test_support::FakeAppsController;

        let mut state = test_state(Vec::new()).with_apps_controller(Some(Arc::new(
            FakeAppsController {
                endpoint: Some(sw_core::AppSessionEndpoint {
                    port: 14500,
                    password: "pass".to_string(),
                }),
                installed: vec!["vlc".to_string()],
                ..Default::default()
            },
        )));
        state.pairing = Arc::new(RwLock::new(PairingState::paired("host".to_string())));

        let unknown = apply_app_launch(
            &state,
            sw_core::AppLaunchRequest {
                app_id: "not-an-app".to_string(),
            },
        );
        assert!(!unknown.launched);

        let uninstalled = apply_app_launch(
            &state,
            sw_core::AppLaunchRequest {
                app_id: "firefox".to_string(),
            },
        );
        assert!(!uninstalled.launched);

        let launched = apply_app_launch(
            &state,
            sw_core::AppLaunchRequest {
                app_id: "vlc".to_string(),
            },
        );
        assert!(launched.launched);
    }

    #[test]
    fn apps_status_exposes_session_only_when_paired() {
        use crate::apps::test_support::FakeAppsController;

        let controller = Arc::new(FakeAppsController {
            endpoint: Some(sw_core::AppSessionEndpoint {
                port: 14500,
                password: "pass".to_string(),
            }),
            ..Default::default()
        });
        let unpaired = test_state(Vec::new()).with_apps_controller(Some(controller.clone()));
        assert!(matches!(
            apps_status_response(&unpaired).session,
            sw_core::AppSessionState::NotPaired
        ));

        let mut paired = test_state(Vec::new()).with_apps_controller(Some(controller));
        paired.pairing = Arc::new(RwLock::new(PairingState::paired("host".to_string())));
        let status = apps_status_response(&paired);
        assert!(matches!(
            status.session,
            sw_core::AppSessionState::Ready { .. }
        ));
        assert!(!status.apps.is_empty());
    }

    #[test]
    fn share_config_mounts_when_paired() {
        use crate::share::test_support::FakeShareController;

        let mut state = test_state(Vec::new())
            .with_share_controller(Some(Arc::new(FakeShareController::default())));
        state.pairing = Arc::new(RwLock::new(PairingState::paired("host".to_string())));

        let response = apply_share_config(
            &state,
            sw_core::ShareConfigRequest {
                unc_path: r"\\host\SecondWind".to_string(),
                username: "SecondWindShare".to_string(),
                password: "secret".to_string(),
            },
        );

        assert_eq!(response.share, sw_core::ShareState::Mounted);
    }

    #[test]
    fn usb_bind_and_unbind_round_trip_when_paired() {
        use crate::usb::test_support::FakeUsbController;

        let controller = Arc::new(FakeUsbController {
            devices: vec![sw_core::UsbDeviceInfo {
                bus_id: "1-1".to_string(),
                vendor_id: "0951".to_string(),
                product_id: "1666".to_string(),
                description: "Flash drive".to_string(),
                bound: false,
            }],
            ..Default::default()
        });
        let mut state = test_state(Vec::new()).with_usb_controller(Some(controller));
        state.pairing = Arc::new(RwLock::new(PairingState::paired("host".to_string())));

        let unpaired_check = usb_devices_response(&test_state(Vec::new()));
        assert!(matches!(unpaired_check.usb, sw_core::UsbState::NotPaired));

        let bound = apply_usb_command(
            &state,
            sw_core::UsbCommandRequest {
                action: sw_core::UsbAction::Bind {
                    bus_id: "1-1".to_string(),
                },
            },
        );
        assert!(bound.devices[0].bound);
        assert!(bound.message.is_none());

        let unbound = apply_usb_command(
            &state,
            sw_core::UsbCommandRequest {
                action: sw_core::UsbAction::Unbind {
                    bus_id: "1-1".to_string(),
                },
            },
        );
        assert!(!unbound.devices[0].bound);
    }

    #[test]
    fn pairing_acceptance_moves_kiosk_to_idle() {
        let kiosk_file = std::env::temp_dir().join(format!(
            "secondwind-api-kiosk-pairing-{}.json",
            std::process::id()
        ));
        let _ = fs::remove_file(&kiosk_file);
        let mut state = test_state(Vec::new()).with_kiosk_state_file(Some(kiosk_file.clone()));
        state.pairing = Arc::new(RwLock::new(waiting_pairing_state()));

        let response = submit_pairing_request(
            &state,
            PairingRequest {
                host_name: "host".to_string(),
                host_certificate_fingerprint: "host-fingerprint".to_string(),
                host_certificate_pem: "host certificate".to_string(),
                pin: PairingPin::new("123456").expect("valid pin"),
            },
        );
        let kiosk_state = sw_core::kiosk::read_kiosk_state(&kiosk_file).expect("kiosk state");
        let _ = fs::remove_file(&kiosk_file);

        assert!(response.accepted);
        assert!(matches!(kiosk_state, sw_core::KioskState::Idle { .. }));
    }
}
