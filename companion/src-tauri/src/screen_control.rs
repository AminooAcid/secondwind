//! Host-side Screen feature orchestration.
//!
//! Connect: make sure the screen engine (Apollo) is detected, configured,
//! and running; arm a one-shot stream pairing PIN if this node's streaming
//! client has never paired with it; then ask the node to bring its screen
//! up. Disconnect: ask the node to stop; the engine tears the virtual
//! display down when the stream ends, so Windows reflows like a monitor was
//! unplugged.

use sw_core::{NodeUuid, ScreenAction, ScreenCommandRequest, ScreenState, ScreenStatusResponse};

use crate::{
    apollo,
    host_state::HostState,
    node_client::{self, NodeClientError, NodeEndpoint},
};

#[derive(Debug)]
pub enum ScreenControlError {
    NotPaired,
    Apollo(apollo::ApolloError),
    Node(NodeClientError),
    State(crate::host_state::HostStateError),
    NodeDeclined { message: Option<String> },
}

impl std::fmt::Display for ScreenControlError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotPaired => write!(formatter, "Pair with this node before using the screen."),
            Self::Apollo(source) => write!(formatter, "{source}"),
            Self::Node(source) => write!(formatter, "{source}"),
            Self::State(source) => write!(formatter, "{source}"),
            Self::NodeDeclined { message } => match message {
                Some(message) => write!(formatter, "{message}"),
                None => write!(formatter, "The node could not start the screen."),
            },
        }
    }
}

impl std::error::Error for ScreenControlError {}

/// Builds the endpoint for a *paired* node: addresses/port may come from
/// live discovery, but the pinned fingerprint always comes from stored
/// trust, never from the network.
pub fn paired_endpoint(
    state: &HostState,
    node_uuid: &NodeUuid,
    addresses: Vec<std::net::IpAddr>,
    api_port: u16,
) -> Result<NodeEndpoint, ScreenControlError> {
    let node = state
        .paired_node(node_uuid)
        .ok_or(ScreenControlError::NotPaired)?;

    Ok(NodeEndpoint {
        addresses,
        api_port,
        certificate_fingerprint: node.trust.peer_certificate_fingerprint.clone(),
    })
}

pub fn connect_screen(
    state: &mut HostState,
    state_root: &std::path::Path,
    node_uuid: &NodeUuid,
    endpoint: &NodeEndpoint,
) -> Result<ScreenStatusResponse, ScreenControlError> {
    let node = state
        .paired_node(node_uuid)
        .ok_or(ScreenControlError::NotPaired)?;
    let node_display_name = node.display_name.clone();
    let stream_already_paired = node.screen.stream_paired;

    // 1. Screen engine detected and its managed config written.
    let installation = apollo::detect_installation().ok_or(ScreenControlError::Apollo(
        apollo::ApolloError::NotInstalled,
    ))?;
    let config_changed =
        apollo::apply_managed_config(&installation, &state.config.host.display_name)
            .map_err(ScreenControlError::Apollo)?;

    // 2. Arm the one-shot inner pairing PIN if this node never paired.
    let stream_pair_pin = if stream_already_paired {
        // Still make sure Apollo is up if we changed its config.
        if config_changed {
            let (credentials, created) = apollo::load_or_create_credentials(state_root)
                .map_err(ScreenControlError::Apollo)?;
            apollo::prepare_apollo(&installation, &credentials, created)
                .map_err(ScreenControlError::Apollo)?;
        }
        None
    } else {
        let (credentials, credentials_created) =
            apollo::load_or_create_credentials(state_root).map_err(ScreenControlError::Apollo)?;

        // Bring Apollo to a known-good state in one clean cycle when we
        // changed its config or minted new credentials; otherwise just
        // confirm the API is up, and only then repair it if it isn't.
        if config_changed || credentials_created {
            apollo::prepare_apollo(&installation, &credentials, credentials_created)
                .map_err(ScreenControlError::Apollo)?;
        } else {
            match apollo::wait_for_api_ready(&credentials) {
                apollo::ApiReadiness::Ready => {}
                apollo::ApiReadiness::StaleCredentials => {
                    apollo::prepare_apollo(&installation, &credentials, true)
                        .map_err(ScreenControlError::Apollo)?;
                }
                apollo::ApiReadiness::Down => {
                    apollo::prepare_apollo(&installation, &credentials, false)
                        .map_err(ScreenControlError::Apollo)?;
                }
            }
        }

        let pin = apollo::generate_stream_pair_pin().map_err(ScreenControlError::Apollo)?;
        apollo::arm_stream_pair_pin(&apollo::api_base(), &credentials, &pin, &node_display_name)
            .map_err(ScreenControlError::Apollo)?;
        Some(pin)
    };

    // 3. Tell the node to bring the screen up (mTLS with host identity).
    let request = ScreenCommandRequest {
        action: ScreenAction::Connect {
            stream_pair_pin: stream_pair_pin.clone(),
        },
    };
    let response: ScreenStatusResponse =
        node_client::post_screen_command(endpoint, Some(&state.certificate), &request)
            .map_err(ScreenControlError::Node)?;

    match &response.screen {
        ScreenState::Streaming { .. } => {
            if stream_pair_pin.is_some() {
                state
                    .mark_stream_paired(node_uuid)
                    .map_err(ScreenControlError::State)?;
            }
            Ok(response)
        }
        _ => Err(ScreenControlError::NodeDeclined {
            message: response.message.clone(),
        }),
    }
}

pub fn disconnect_screen(
    state: &HostState,
    endpoint: &NodeEndpoint,
) -> Result<ScreenStatusResponse, ScreenControlError> {
    node_client::post_screen_command(
        endpoint,
        Some(&state.certificate),
        &ScreenCommandRequest {
            action: ScreenAction::Disconnect,
        },
    )
    .map_err(ScreenControlError::Node)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host_state::HostState;

    fn temp_state(name: &str) -> (HostState, std::path::PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "secondwind-screen-control-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let state = HostState::load_or_create(&root).expect("host state");
        (state, root)
    }

    #[test]
    fn paired_endpoint_pins_stored_trust_not_discovery() {
        let (mut state, root) = temp_state("pins-trust");
        let node_uuid = NodeUuid::new("00000000-0000-4000-8000-000000000006").expect("valid uuid");
        state
            .record_paired_node(node_uuid, "node".to_string(), "sha256:trusted".to_string())
            .expect("record");

        let endpoint = paired_endpoint(&state, &node_uuid, vec![], 49152).expect("paired endpoint");

        assert_eq!(endpoint.certificate_fingerprint, "sha256:trusted");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn unpaired_node_cannot_get_screen_endpoint() {
        let (state, root) = temp_state("unpaired");
        let node_uuid = NodeUuid::new("00000000-0000-4000-8000-000000000007").expect("valid uuid");

        let error = paired_endpoint(&state, &node_uuid, vec![], 49152).expect_err("unpaired");

        assert!(matches!(error, ScreenControlError::NotPaired));
        let _ = std::fs::remove_dir_all(&root);
    }
}
