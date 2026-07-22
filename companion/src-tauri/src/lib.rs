pub mod apollo;
pub mod app_control;
pub mod auto_connect;
pub mod discovery;
pub mod disk_control;
pub mod host_state;
pub mod jobs_cli;
pub mod node_client;
pub mod screen_control;
pub mod usb_control;

pub fn run() {
    let active_screens = auto_connect::new_active_screens();
    let watcher_screens = active_screens.clone();

    tauri::Builder::default()
        .manage(active_screens)
        .setup(move |app| {
            auto_connect::spawn(app.handle().clone(), watcher_screens.clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::discover_nodes,
            commands::pair_node,
            commands::paired_nodes,
            commands::set_screen,
            commands::set_disk,
            commands::app_library,
            commands::set_app_policy,
            commands::launch_app,
            commands::usb_devices,
            commands::set_usb_attached,
            commands::set_usb_auto_attach,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run SecondWind companion");
}

/// Helpers shared by the command handlers and the auto-connect watcher.
pub(crate) mod commands_support {
    use std::path::PathBuf;

    use tauri::Manager;

    pub const STATE_DIR_ENV: &str = "SECONDWIND_COMPANION_STATE_DIR";

    pub fn state_root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
        if let Ok(dir) = std::env::var(STATE_DIR_ENV) {
            if !dir.trim().is_empty() {
                return Ok(PathBuf::from(dir));
            }
        }

        app.path()
            .app_data_dir()
            .map_err(|_| "SecondWind could not find a place to store its settings.".to_string())
    }

    pub fn load_host_state(
        app: &tauri::AppHandle,
    ) -> Result<crate::host_state::HostState, String> {
        crate::host_state::HostState::load_or_create(state_root(app)?)
            .map_err(|error| error.to_string())
    }
}

mod commands {
    use std::time::Duration;

    use serde::Serialize;
    use sw_core::{NodeUuid, PairingPin, PairingRequest};

    use crate::{
        app_control,
        auto_connect::ActiveScreens,
        commands_support::{load_host_state, state_root},
        discovery::{self, DiscoveredNode},
        disk_control,
        host_state::HostState,
        node_client::{self, NodeEndpoint},
        screen_control, usb_control,
    };

    const DISCOVERY_BROWSE_WINDOW_MS: u64 = 900;

    #[derive(Debug, Clone, Serialize)]
    pub struct PairedNodeSummary {
        pub node_uuid: NodeUuid,
        pub display_name: String,
        pub node_certificate_fingerprint: String,
        pub screen_always_on: bool,
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

        // Learn wake targets right away (best-effort; mTLS is live now).
        if let Ok(capabilities) =
            node_client::get_capabilities(&endpoint, Some(&state.certificate))
        {
            let macs: Vec<String> = capabilities
                .capabilities
                .network_interfaces
                .iter()
                .map(|interface| interface.mac_address.clone())
                .collect();
            if !macs.is_empty() {
                let _ = state.record_wake_targets(&node.node_uuid, macs);
            }
        }

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
        screens: tauri::State<'_, ActiveScreens>,
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

        let streaming = matches!(response.screen, sw_core::ScreenState::Streaming { .. });
        if let Ok(mut active) = screens.lock() {
            if streaming {
                active.insert(node.node_uuid);
            } else {
                active.remove(&node.node_uuid);
            }
        }

        Ok(ScreenToggleResult {
            streaming,
            message: response.message,
        })
    }

    #[derive(Debug, Clone, Serialize)]
    pub struct DiskToggleResult {
        pub attached: bool,
        pub drive_letter: Option<char>,
        pub message: Option<String>,
    }

    #[tauri::command]
    pub fn set_disk(
        app: tauri::AppHandle,
        node: DiscoveredNode,
        enabled: bool,
    ) -> Result<DiskToggleResult, String> {
        let state = load_host_state(&app)?;
        let endpoint = screen_control::paired_endpoint(
            &state,
            &node.node_uuid,
            node.addresses.clone(),
            node.api_port,
        )
        .map_err(|error| error.to_string())?;

        if enabled {
            let outcome = disk_control::connect_disk(&state, &node.node_uuid, &endpoint)
                .map_err(|error| error.to_string())?;
            Ok(DiskToggleResult {
                attached: true,
                drive_letter: outcome.drive_letter,
                message: None,
            })
        } else {
            disk_control::disconnect_disk(&state, &endpoint, None)
                .map_err(|error| error.to_string())?;
            Ok(DiskToggleResult {
                attached: false,
                drive_letter: None,
                message: None,
            })
        }
    }

    #[tauri::command]
    pub fn app_library(app: tauri::AppHandle) -> Result<Vec<sw_core::AppCatalogEntry>, String> {
        Ok(load_host_state(&app)?.config.apps)
    }

    #[tauri::command]
    pub fn set_app_policy(
        app: tauri::AppHandle,
        app_id: String,
        policy: sw_core::AppPolicy,
        fallback_to_local: bool,
    ) -> Result<(), String> {
        let mut state = load_host_state(&app)?;
        let Some(entry) = state
            .config
            .apps
            .iter_mut()
            .find(|entry| entry.app_id == app_id)
        else {
            return Err("That app is not in the SecondWind library.".to_string());
        };
        entry.policy = policy;
        entry.fallback_to_local = fallback_to_local;
        state.save().map_err(|error| error.to_string())
    }

    /// Launches an app per its policy. `node` is the discovered node when
    /// present; `choice_on_node` carries the user's answer for
    /// ask-each-time apps (None on the first call).
    #[tauri::command]
    pub fn launch_app(
        app: tauri::AppHandle,
        app_id: String,
        node: Option<DiscoveredNode>,
        choice_on_node: Option<bool>,
    ) -> Result<app_control::LaunchOutcome, String> {
        let state_root = state_root(&app)?;
        let state = load_host_state(&app)?;
        let Some(entry) = state
            .config
            .apps
            .iter()
            .find(|entry| entry.app_id == app_id)
            .cloned()
        else {
            return Err("That app is not in the SecondWind library.".to_string());
        };

        // The app library is per-host; the node used is the paired node in
        // view (v0.3 UI shows one node at a time, data model supports N).
        let (node_uuid, endpoint) = match &node {
            Some(node) => (
                node.node_uuid,
                screen_control::paired_endpoint(
                    &state,
                    &node.node_uuid,
                    node.addresses.clone(),
                    node.api_port,
                )
                .ok(),
            ),
            None => match state.config.nodes.keys().next() {
                Some(node_uuid) => (*node_uuid, None),
                None => {
                    return Err("Pair with a node before launching apps.".to_string());
                }
            },
        };

        let availability =
            app_control::availability(&state, &node_uuid, endpoint.is_some());
        let decision = app_control::resolve_decision(&entry, availability, choice_on_node);

        Ok(app_control::execute_decision(
            &state,
            &state_root,
            &entry,
            decision,
            endpoint,
            &node_uuid,
        ))
    }

    fn paired_endpoint_for(
        state: &HostState,
        node: &DiscoveredNode,
    ) -> Result<NodeEndpoint, String> {
        screen_control::paired_endpoint(
            state,
            &node.node_uuid,
            node.addresses.clone(),
            node.api_port,
        )
        .map_err(|error| error.to_string())
    }

    /// Node USB device merged with the host-side auto-attach rule.
    #[derive(Debug, Clone, Serialize)]
    pub struct UsbDeviceView {
        #[serde(flatten)]
        pub device: sw_core::UsbDeviceInfo,
        pub auto_attach: bool,
    }

    #[derive(Debug, Clone, Serialize)]
    pub struct UsbDevicesView {
        pub usb: sw_core::UsbState,
        pub devices: Vec<UsbDeviceView>,
        pub message: Option<String>,
    }

    #[tauri::command]
    pub fn usb_devices(
        app: tauri::AppHandle,
        node: DiscoveredNode,
    ) -> Result<UsbDevicesView, String> {
        let state = load_host_state(&app)?;
        let endpoint = paired_endpoint_for(&state, &node)?;
        let response = node_client::get_usb_devices(&endpoint, Some(&state.certificate))
            .map_err(|error| error.to_string())?;

        let rules = state
            .paired_node(&node.node_uuid)
            .map(|paired| paired.usb_auto_attach.clone())
            .unwrap_or_default();
        let devices = response
            .devices
            .into_iter()
            .map(|device| UsbDeviceView {
                auto_attach: rules.iter().any(|rule| {
                    rule.vendor_id == device.vendor_id && rule.product_id == device.product_id
                }),
                device,
            })
            .collect();

        Ok(UsbDevicesView {
            usb: response.usb,
            devices,
            message: response.message,
        })
    }

    #[tauri::command]
    pub fn set_usb_attached(
        app: tauri::AppHandle,
        node: DiscoveredNode,
        bus_id: String,
        vendor_id: String,
        product_id: String,
        attached: bool,
    ) -> Result<(), String> {
        let state = load_host_state(&app)?;
        let endpoint = paired_endpoint_for(&state, &node)?;

        if attached {
            usb_control::attach_device(&state, &endpoint, &bus_id).map(|_| ())
        } else {
            usb_control::detach_device(&state, &endpoint, &bus_id, &vendor_id, &product_id)
        }
    }

    #[tauri::command]
    pub fn set_usb_auto_attach(
        app: tauri::AppHandle,
        node_uuid: NodeUuid,
        vendor_id: String,
        product_id: String,
        enabled: bool,
    ) -> Result<(), String> {
        let mut state = load_host_state(&app)?;
        let Some(node) = state.config.nodes.get_mut(&node_uuid) else {
            return Err("Pair with this node first.".to_string());
        };

        node.usb_auto_attach
            .retain(|rule| !(rule.vendor_id == vendor_id && rule.product_id == product_id));
        if enabled {
            node.usb_auto_attach.push(sw_core::UsbAutoAttachRule {
                vendor_id,
                product_id,
            });
        }
        state.save().map_err(|error| error.to_string())
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
