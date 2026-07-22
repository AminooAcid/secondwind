//! Host-side USB attach/detach (v0.4).
//!
//! The node binds a device to its export (mTLS `POST /v1/usb`), then the
//! host attaches it with the bundled usbip-win2 client so it appears in
//! Device Manager. Detach reverses the order. All builders are pure and
//! tested; the client binary location is env-overridable and bundled by
//! the installer.

use std::{net::IpAddr, path::PathBuf, process::Command};

use sw_core::{UsbAction, UsbCommandRequest, UsbDevicesResponse};

use crate::{
    host_state::HostState,
    node_client::{self, NodeEndpoint},
};

pub const USBIP_CLIENT_ENV: &str = "SECONDWIND_USBIP_CLIENT";
const DEFAULT_USBIP_CLIENT: &str = "usbip.exe";

pub fn usbip_client() -> PathBuf {
    std::env::var_os(USBIP_CLIENT_ENV)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_USBIP_CLIENT))
}

pub fn attach_args(node_address: &IpAddr, bus_id: &str) -> Vec<String> {
    vec![
        "attach".to_string(),
        "-r".to_string(),
        node_address.to_string(),
        "-b".to_string(),
        bus_id.to_string(),
    ]
}

pub fn detach_args(port: u32) -> Vec<String> {
    vec!["detach".to_string(), "-p".to_string(), port.to_string()]
}

/// Parses `usbip port` output to find the local port a remote device is
/// attached on. Lines look like:
/// ```text
/// Port 00: <Port in Use> at Full Speed(12Mbps)
///        unknown vendor : unknown product (0951:1666)
///        3-1 -> usbip://10.0.0.7:3240/1-1.4
/// ```
/// Matching is by the remote **bus id** (the exact device the node
/// exported), never by vendor:product — two identical flash drives must
/// not detach each other. VID:PID is only a fallback when no remote-URL
/// line is present.
pub fn parse_attached_port(
    output: &str,
    bus_id: &str,
    vendor_id: &str,
    product_id: &str,
) -> Option<u32> {
    let bus_needle = format!("/{bus_id}");
    let id_needle = format!("({}:{})", vendor_id.to_lowercase(), product_id.to_lowercase());
    let mut current_port: Option<u32> = None;
    let mut id_fallback: Option<u32> = None;

    for line in output.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("Port ") {
            current_port = rest
                .split(':')
                .next()
                .and_then(|port| port.trim().parse().ok());
        } else if trimmed.contains("usbip://") && trimmed.ends_with(&bus_needle) {
            return current_port;
        } else if trimmed.to_lowercase().contains(&id_needle) && id_fallback.is_none() {
            id_fallback = current_port;
        }
    }
    id_fallback
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct UsbAttachResult {
    pub attached: bool,
    pub message: Option<String>,
}

/// Node-side bind, then host-side attach.
pub fn attach_device(
    state: &HostState,
    endpoint: &NodeEndpoint,
    bus_id: &str,
) -> Result<UsbDevicesResponse, String> {
    let response = node_client::post_usb_command(
        endpoint,
        Some(&state.certificate),
        &UsbCommandRequest {
            action: UsbAction::Bind {
                bus_id: bus_id.to_string(),
            },
        },
    )
    .map_err(|error| error.to_string())?;
    if let Some(message) = &response.message {
        return Err(message.clone());
    }

    let node_address = endpoint
        .addresses
        .first()
        .ok_or("The node did not advertise a reachable address.")?;
    let status = Command::new(usbip_client())
        .args(attach_args(node_address, bus_id))
        .status()
        .map_err(|_| {
            "SecondWind's USB client is missing from this installation.".to_string()
        })?;
    if !status.success() {
        return Err(
            "The device could not be attached. If this is the first USB use, the SecondWind \
             USB driver may still need to be installed — see the USB guide."
                .to_string(),
        );
    }

    Ok(response)
}

/// Host-side detach (by port, looked up via `usbip port`), then node-side
/// unbind.
pub fn detach_device(
    state: &HostState,
    endpoint: &NodeEndpoint,
    bus_id: &str,
    vendor_id: &str,
    product_id: &str,
) -> Result<(), String> {
    let port_output = Command::new(usbip_client())
        .arg("port")
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
        .unwrap_or_default();
    if let Some(port) = parse_attached_port(&port_output, bus_id, vendor_id, product_id) {
        let _ = Command::new(usbip_client()).args(detach_args(port)).status();
    }

    node_client::post_usb_command(
        endpoint,
        Some(&state.certificate),
        &UsbCommandRequest {
            action: UsbAction::Unbind {
                bus_id: bus_id.to_string(),
            },
        },
    )
    .map(|_| ())
    .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use super::*;

    #[test]
    fn attach_args_carry_remote_and_busid() {
        let args = attach_args(&IpAddr::V4(Ipv4Addr::new(10, 0, 0, 7)), "1-1.4");

        assert_eq!(args, vec!["attach", "-r", "10.0.0.7", "-b", "1-1.4"]);
    }

    #[test]
    fn detach_args_use_the_port_number() {
        assert_eq!(detach_args(3), vec!["detach", "-p", "3"]);
    }

    const PORT_OUTPUT: &str = r#"
Imported USB devices
====================
Port 00: <Port in Use> at Full Speed(12Mbps)
       unknown vendor : unknown product (046d:c31c)
       3-1 -> usbip://10.0.0.7:3240/1-1.4
Port 01: <Port in Use> at High Speed(480Mbps)
       unknown vendor : unknown product (0951:1666)
       3-2 -> usbip://10.0.0.7:3240/1-1
Port 02: <Port in Use> at High Speed(480Mbps)
       unknown vendor : unknown product (0951:1666)
       3-3 -> usbip://10.0.0.7:3240/2-1
"#;

    #[test]
    fn parses_attached_port_by_exact_bus_id() {
        assert_eq!(parse_attached_port(PORT_OUTPUT, "1-1", "0951", "1666"), Some(1));
        assert_eq!(parse_attached_port(PORT_OUTPUT, "1-1.4", "046D", "C31C"), Some(0));
        assert_eq!(parse_attached_port(PORT_OUTPUT, "9-9", "ffff", "0000"), None);
    }

    #[test]
    fn identical_devices_detach_the_right_port() {
        // Two flash drives with the same VID:PID on different bus ids
        // (BUG-014: VID:PID matching alone would always pick port 1).
        assert_eq!(parse_attached_port(PORT_OUTPUT, "2-1", "0951", "1666"), Some(2));
    }

    #[test]
    fn bus_id_suffix_matching_is_exact_not_prefix() {
        // "1-1" must not match the "…/1-1.4" line.
        assert_eq!(parse_attached_port(PORT_OUTPUT, "1", "ffff", "0000"), None);
    }
}
