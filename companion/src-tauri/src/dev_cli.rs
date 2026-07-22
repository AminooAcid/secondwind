//! Headless validation/support commands (no GUI):
//!
//!   secondwind-companion --discover
//!       browse mDNS and print discovered nodes as JSON
//!   secondwind-companion --pair <node-uuid> <pin>
//!       pair with a discovered node exactly like the UI would
//!   secondwind-companion --node-health <node-uuid>
//!       fetch /v1/health from a paired node over mTLS
//!   secondwind-companion --screen-on <node-uuid>   (needs admin: writes Apollo config)
//!   secondwind-companion --screen-off <node-uuid>
//!       drive the Screen feature exactly like the UI toggle
//!
//! Used during hardware validation and support sessions; the product flow
//! remains the UI.

use std::time::Duration;

use sw_core::{NodeUuid, PairingPin, PairingRequest};

use crate::{
    discovery::{self, DiscoveredNode},
    host_state::HostState,
    node_client::{self, NodeEndpoint},
};

const BROWSE: Duration = Duration::from_millis(2000);

pub fn run(args: &[String]) -> i32 {
    let result = match args.first().map(String::as_str) {
        Some("--discover") => discover(),
        Some("--pair") => pair(args.get(1), args.get(2)),
        Some("--node-health") => node_health(args.get(1)),
        Some("--screen-on") => screen(args.get(1), true),
        Some("--screen-off") => screen(args.get(1), false),
        _ => Err("usage: --discover | --pair <uuid> <pin> | --node-health <uuid> | \
                  --screen-on <uuid> | --screen-off <uuid>"
            .to_string()),
    };

    match result {
        Ok(output) => {
            println!("{output}");
            0
        }
        Err(message) => {
            eprintln!("SecondWind: {message}");
            1
        }
    }
}

fn browse() -> Result<Vec<DiscoveredNode>, String> {
    discovery::discover_secondwind_nodes(BROWSE).map_err(|error| error.to_string())
}

fn find(nodes: Vec<DiscoveredNode>, uuid_text: &str) -> Result<DiscoveredNode, String> {
    let node_uuid = NodeUuid::new(uuid_text).map_err(|_| "invalid node uuid".to_string())?;
    nodes
        .into_iter()
        .find(|node| node.node_uuid == node_uuid)
        .ok_or_else(|| "that node was not discovered on the network".to_string())
}

fn discover() -> Result<String, String> {
    let nodes = browse()?;
    serde_json::to_string_pretty(&nodes).map_err(|error| error.to_string())
}

fn pair(uuid_arg: Option<&String>, pin_arg: Option<&String>) -> Result<String, String> {
    let (uuid_text, pin_text) = match (uuid_arg, pin_arg) {
        (Some(uuid), Some(pin)) => (uuid, pin),
        _ => return Err("usage: --pair <node-uuid> <pin>".to_string()),
    };
    let pin = PairingPin::new(pin_text.trim()).map_err(|_| "the PIN must be 6 digits".to_string())?;

    let node = find(browse()?, uuid_text)?;
    let state_root = crate::jobs_cli::state_dir()?;
    let mut state = HostState::load_or_create(&state_root).map_err(|error| error.to_string())?;

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
        return Err("the node declined pairing (check the PIN)".to_string());
    }
    state
        .record_paired_node(
            node.node_uuid,
            node.node_name.clone(),
            node.node_certificate_fingerprint.clone(),
        )
        .map_err(|error| error.to_string())?;

    // Wake targets, same as the UI flow (best-effort).
    if let Ok(capabilities) = node_client::get_capabilities(&endpoint, Some(&state.certificate)) {
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

    Ok(format!("paired with {} ({})", node.node_name, node.node_uuid))
}

fn screen(uuid_arg: Option<&String>, on: bool) -> Result<String, String> {
    let uuid_text = uuid_arg.ok_or("usage: --screen-on|--screen-off <node-uuid>")?;
    let node = find(browse()?, uuid_text)?;
    let state_root = crate::jobs_cli::state_dir()?;
    let mut state = HostState::load_or_create(&state_root).map_err(|error| error.to_string())?;
    let endpoint = crate::screen_control::paired_endpoint(
        &state,
        &node.node_uuid,
        node.addresses.clone(),
        node.api_port,
    )
    .map_err(|error| error.to_string())?;

    if on {
        let response =
            crate::screen_control::connect_screen(&mut state, &state_root, &node.node_uuid, &endpoint)
                .map_err(|error| error.to_string())?;
        Ok(format!("screen on: {:?}", response.screen))
    } else {
        let response = crate::screen_control::disconnect_screen(&state, &endpoint)
            .map_err(|error| error.to_string())?;
        Ok(format!("screen off: {:?}", response.screen))
    }
}

fn node_health(uuid_arg: Option<&String>) -> Result<String, String> {
    let uuid_text = uuid_arg.ok_or("usage: --node-health <node-uuid>")?;
    let node = find(browse()?, uuid_text)?;
    let state_root = crate::jobs_cli::state_dir()?;
    let state = HostState::load_or_create(&state_root).map_err(|error| error.to_string())?;
    let trusted = state
        .paired_node(&node.node_uuid)
        .ok_or("not paired with that node yet")?
        .trust
        .peer_certificate_fingerprint
        .clone();

    let endpoint = NodeEndpoint {
        addresses: node.addresses,
        api_port: node.api_port,
        certificate_fingerprint: trusted,
    };
    let health: sw_core::HealthResponse =
        node_client::get_json(&endpoint, Some(&state.certificate), "/v1/health")
            .map_err(|error| error.to_string())?;
    serde_json::to_string_pretty(&health).map_err(|error| error.to_string())
}
