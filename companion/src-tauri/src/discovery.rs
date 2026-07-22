use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

use mdns_sd::{Receiver, ResolvedService, ServiceDaemon, ServiceEvent};
use serde::{Deserialize, Serialize};
use sw_core::{
    NodeUuid, SECONDWIND_MDNS_SERVICE_TYPE, TXT_KEY_API_VERSION, TXT_KEY_NODE_CERT_SHA256,
    TXT_KEY_NODE_NAME, TXT_KEY_NODE_UUID,
};

pub struct SecondWindNodeBrowser {
    daemon: ServiceDaemon,
    receiver: Receiver<ServiceEvent>,
}

impl SecondWindNodeBrowser {
    pub fn receiver(&self) -> &Receiver<ServiceEvent> {
        &self.receiver
    }

    pub fn shutdown(self) -> Result<(), CompanionDiscoveryError> {
        self.daemon
            .stop_browse(SECONDWIND_MDNS_SERVICE_TYPE)
            .map_err(CompanionDiscoveryError::StopBrowse)?;
        self.daemon
            .shutdown()
            .map(|_| ())
            .map_err(CompanionDiscoveryError::Daemon)
    }
}

pub fn browse_secondwind_nodes() -> Result<SecondWindNodeBrowser, CompanionDiscoveryError> {
    let daemon = ServiceDaemon::new().map_err(CompanionDiscoveryError::Daemon)?;
    let receiver = daemon
        .browse(SECONDWIND_MDNS_SERVICE_TYPE)
        .map_err(CompanionDiscoveryError::Browse)?;

    Ok(SecondWindNodeBrowser { daemon, receiver })
}

pub fn discover_secondwind_nodes(
    browse_window: Duration,
) -> Result<Vec<DiscoveredNode>, CompanionDiscoveryError> {
    let browser = browse_secondwind_nodes()?;
    let deadline = Instant::now() + browse_window;
    let mut nodes = BTreeMap::new();

    while let Some(remaining) = deadline.checked_duration_since(Instant::now()) {
        if remaining.is_zero() {
            break;
        }

        match browser.receiver().recv_timeout(remaining) {
            Ok(ServiceEvent::ServiceResolved(service)) => {
                if let Ok(node) = discovered_node_from_service(&service) {
                    nodes.insert(node.node_uuid, node);
                }
            }
            Ok(_) => {}
            Err(_) => break,
        }
    }

    browser.shutdown()?;

    Ok(nodes.into_values().collect())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveredNode {
    pub node_uuid: NodeUuid,
    pub node_name: String,
    pub host_name: String,
    pub api_port: u16,
    pub api_version: String,
    pub node_certificate_fingerprint: String,
}

pub fn discovered_node_from_service(
    service: &ResolvedService,
) -> Result<DiscoveredNode, CompanionDiscoveryError> {
    if service.ty_domain != SECONDWIND_MDNS_SERVICE_TYPE {
        return Err(CompanionDiscoveryError::UnexpectedServiceType {
            service_type: service.ty_domain.clone(),
        });
    }

    let node_uuid = required_txt(service, TXT_KEY_NODE_UUID).and_then(|value| {
        NodeUuid::new(value).map_err(|_| CompanionDiscoveryError::InvalidNodeUuid)
    })?;
    let node_name = required_txt(service, TXT_KEY_NODE_NAME)?.to_string();
    let api_version = required_txt(service, TXT_KEY_API_VERSION)?.to_string();
    let node_certificate_fingerprint = required_txt(service, TXT_KEY_NODE_CERT_SHA256)?.to_string();

    Ok(DiscoveredNode {
        node_uuid,
        node_name,
        host_name: service.get_hostname().to_string(),
        api_port: service.get_port(),
        api_version,
        node_certificate_fingerprint,
    })
}

fn required_txt<'a>(
    service: &'a ResolvedService,
    key: &'static str,
) -> Result<&'a str, CompanionDiscoveryError> {
    service
        .get_property_val_str(key)
        .filter(|value| !value.trim().is_empty())
        .ok_or(CompanionDiscoveryError::MissingTxt { key })
}

#[derive(Debug)]
pub enum CompanionDiscoveryError {
    Daemon(mdns_sd::Error),
    Browse(mdns_sd::Error),
    StopBrowse(mdns_sd::Error),
    UnexpectedServiceType { service_type: String },
    MissingTxt { key: &'static str },
    InvalidNodeUuid,
}

impl std::fmt::Display for CompanionDiscoveryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Daemon(_) => write!(formatter, "failed to start mDNS discovery"),
            Self::Browse(_) => write!(formatter, "failed to browse for SecondWind nodes"),
            Self::StopBrowse(_) => write!(formatter, "failed to stop SecondWind node browsing"),
            Self::UnexpectedServiceType { service_type } => {
                write!(formatter, "unexpected mDNS service type {service_type}")
            }
            Self::MissingTxt { key } => write!(formatter, "missing mDNS TXT property {key}"),
            Self::InvalidNodeUuid => {
                write!(formatter, "discovered node advertised an invalid UUID")
            }
        }
    }
}

impl std::error::Error for CompanionDiscoveryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Daemon(source) | Self::Browse(source) | Self::StopBrowse(source) => Some(source),
            Self::UnexpectedServiceType { .. }
            | Self::MissingTxt { .. }
            | Self::InvalidNodeUuid => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr};

    use mdns_sd::ServiceInfo;
    use sw_core::{CURRENT_AGENT_API_VERSION, NodeAdvertisementTxt};

    use super::*;

    fn resolved_service() -> ResolvedService {
        let node_uuid = "00000000-0000-4000-8000-000000000004";
        let txt = NodeAdvertisementTxt::new(node_uuid, "node", "sha256:fingerprint");
        ServiceInfo::new(
            SECONDWIND_MDNS_SERVICE_TYPE,
            node_uuid,
            "secondwind-node.local.",
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            49152,
            &txt.as_txt_pairs()[..],
        )
        .expect("service info")
        .as_resolved_service()
    }

    #[test]
    fn parses_discovered_node_from_resolved_service() {
        let node = discovered_node_from_service(&resolved_service()).expect("discovered node");

        assert_eq!(
            node.node_uuid.to_string(),
            "00000000-0000-4000-8000-000000000004"
        );
        assert_eq!(node.node_name, "node");
        assert_eq!(node.api_port, 49152);
        assert_eq!(node.api_version, CURRENT_AGENT_API_VERSION);
        assert_eq!(node.node_certificate_fingerprint, "sha256:fingerprint");
    }

    #[test]
    fn rejects_unexpected_service_type() {
        let mut service = resolved_service();
        service.ty_domain = "_other._tcp.local.".to_string();

        let error = discovered_node_from_service(&service).expect_err("wrong service type");

        assert!(matches!(
            error,
            CompanionDiscoveryError::UnexpectedServiceType { .. }
        ));
    }

    #[test]
    fn rejects_missing_node_uuid() {
        let service = ServiceInfo::new(
            SECONDWIND_MDNS_SERVICE_TYPE,
            "node",
            "secondwind-node.local.",
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            49152,
            &[(TXT_KEY_NODE_NAME, "node")][..],
        )
        .expect("service info")
        .as_resolved_service();

        let error = discovered_node_from_service(&service).expect_err("missing uuid");

        assert!(matches!(error, CompanionDiscoveryError::MissingTxt { .. }));
    }
}
