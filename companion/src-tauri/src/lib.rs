pub mod apollo;
pub mod discovery;
pub mod host_state;
pub mod node_client;
pub mod screen_control;

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::discover_nodes,
            commands::pair_node,
            commands::paired_nodes,
            commands::set_screen,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run SecondWind companion");
}

mod commands {
    use std::{path::PathBuf, time::Duration};

    use serde::Serialize;
    use sw_core::{NodeUuid, PairingPin, PairingRequest};
    use tauri::Manager;

    use crate::{
        discovery::{self, DiscoveredNode},
        host_state::HostState,
        node_client::{self, NodeEndpoint},
        screen_control,
    };

    const DISCOVERY_BROWSE_WINDOW_MS: u64 = 900;
    const STATE_DIR_ENV: &str = "SECONDWIND_COMPANION_STATE_DIR";

    #[derive(Debug, Clone, Serialize)]
    pub struct PairedNodeSummary {
        pub node_uuid: NodeUuid,
        pub display_name: String,
        pub node_certificate_fingerprint: String,
        pub screen_always_on: bool,
    }

    fn state_root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
        if let Ok(dir) = std::env::var(STATE_DIR_ENV) {
            if !dir.trim().is_empty() {
                return Ok(PathBuf::from(dir));
            }
        }

        app.path()
            .app_data_dir()
            .map_err(|_| "SecondWind could not find a place to store its settings.".to_string())
    }

    fn load_host_state(app: &tauri::AppHandle) -> Result<HostState, String> {
        HostState::load_or_create(state_root(app)?).map_err(|error| error.to_string())
    }

    fn summaries(state: &HostState) -> Vec<PairedNodeSummary> {
        state
            .config
            .nodes
            .iter()
            .map(|(node_uuid, node)| PairedNodeSummary {
                node_uuid: *node_uuid,
                display_name: node.display_name.clone(),
                node_certificate_fingerprint: node.trust.peer_certificate_fingerprint.clone(),
                screen_always_on: node.screen.always_on,
            })
            .collect()
    }

    #[tauri::command]
    pub fn discover_nodes() -> Result<Vec<DiscoveredNode>, String> {
        discovery::discover_secondwind_nodes(Duration::from_millis(DISCOVERY_BROWSE_WINDOW_MS))
            .map_err(|error| error.to_string())
    }

    #[tauri::command]
    pub fn paired_nodes(app: tauri::AppHandle) -> Result<Vec<PairedNodeSummary>, String> {
        Ok(summaries(&load_host_state(&app)?))
    }

    #[tauri::command]
    pub fn pair_node(
        app: tauri::AppHandle,
        node: DiscoveredNode,
        pin: String,
    ) -> Result<PairedNodeSummary, String> {
        let pin = PairingPin::new(pin.trim())
            .map_err(|_| "Enter the 6-digit PIN shown on the node's screen.".to_string())?;
        let mut state = load_host_state(&app)?;
        let endpoint = NodeEndpoint {
            addresses: node.addresses.clone(),
            api_port: node.api_port,
            certificate_fingerprint: node.node_certificate_fingerprint.clone(),
        };
        let request = PairingRequest {
            host_name: state.config.host.display_name.clone(),
            host_certificate_fingerprint: state.certificate.fingerprint.clone(),
            host_certificate_pem: state.certificate.certificate_pem.clone(),
            pin,
        };

        let response =
            node_client::submit_pairing(&endpoint, &request).map_err(|error| error.to_string())?;
        if !response.accepted {
            return Err(
                "The node declined pairing. Check the PIN on the node's screen and try again."
                    .to_string(),
            );
        }

        state
            .record_paired_node(
                node.node_uuid,
                node.node_name.clone(),
                node.node_certificate_fingerprint.clone(),
            )
            .map_err(|error| error.to_string())?;

        Ok(PairedNodeSummary {
            node_uuid: node.node_uuid,
            display_name: node.node_name,
            node_certificate_fingerprint: node.node_certificate_fingerprint,
            screen_always_on: true,
        })
    }

    #[derive(Debug, Clone, Serialize)]
    pub struct ScreenToggleResult {
        pub streaming: bool,
        pub message: Option<String>,
    }

    #[tauri::command]
    pub fn set_screen(
        app: tauri::AppHandle,
        node: DiscoveredNode,
        enabled: bool,
    ) -> Result<ScreenToggleResult, String> {
        let state_root = state_root(&app)?;
        let mut state = load_host_state(&app)?;
        let endpoint = screen_control::paired_endpoint(
            &state,
            &node.node_uuid,
            node.addresses.clone(),
            node.api_port,
        )
        .map_err(|error| error.to_string())?;

        let response = if enabled {
            screen_control::connect_screen(&mut state, &state_root, &node.node_uuid, &endpoint)
                .map_err(|error| error.to_string())?
        } else {
            screen_control::disconnect_screen(&state, &endpoint)
                .map_err(|error| error.to_string())?
        };

        Ok(ScreenToggleResult {
            streaming: matches!(response.screen, sw_core::ScreenState::Streaming { .. }),
            message: response.message,
        })
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn discovery_window_is_short_enough_for_refresh_ui() {
            assert!(DISCOVERY_BROWSE_WINDOW_MS <= 1_000);
        }
    }
}
