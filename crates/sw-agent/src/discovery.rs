use std::net::{IpAddr, Ipv4Addr};

use mdns_sd::{ServiceDaemon, ServiceInfo};
use sw_core::{NodeAdvertisementTxt, NodeUuid, SECONDWIND_MDNS_SERVICE_TYPE};

pub struct MdnsAdvertisement {
    daemon: ServiceDaemon,
    fullname: String,
}

impl MdnsAdvertisement {
    pub fn fullname(&self) -> &str {
        &self.fullname
    }
}

impl Drop for MdnsAdvertisement {
    fn drop(&mut self) {
        let _ = self.daemon.unregister(&self.fullname);
        let _ = self.daemon.shutdown();
    }
}

pub fn advertise_node(
    node_uuid: NodeUuid,
    node_name: &str,
    node_certificate_fingerprint: &str,
    api_port: u16,
) -> Result<MdnsAdvertisement, MdnsAdvertisementError> {
    let daemon = ServiceDaemon::new().map_err(MdnsAdvertisementError::Daemon)?;
    let service = node_service_info(node_uuid, node_name, node_certificate_fingerprint, api_port)?;
    let fullname = service.get_fullname().to_string();
    daemon
        .register(service)
        .map_err(MdnsAdvertisementError::Register)?;

    Ok(MdnsAdvertisement { daemon, fullname })
}

pub fn node_service_info(
    node_uuid: NodeUuid,
    node_name: &str,
    node_certificate_fingerprint: &str,
    api_port: u16,
) -> Result<ServiceInfo, MdnsAdvertisementError> {
    let instance_name = node_uuid.to_string();
    let host_name = format!("secondwind-{node_uuid}.local.");
    let txt = NodeAdvertisementTxt::new(
        node_uuid.to_string(),
        node_name.to_string(),
        node_certificate_fingerprint.to_string(),
    );
    let placeholder_ip = IpAddr::V4(Ipv4Addr::UNSPECIFIED);
    let service = ServiceInfo::new(
        SECONDWIND_MDNS_SERVICE_TYPE,
        &instance_name,
        &host_name,
        placeholder_ip,
        api_port,
        &txt.as_txt_pairs()[..],
    )
    .map_err(MdnsAdvertisementError::ServiceInfo)?
    .enable_addr_auto();

    Ok(service)
}

#[derive(Debug)]
pub enum MdnsAdvertisementError {
    Daemon(mdns_sd::Error),
    ServiceInfo(mdns_sd::Error),
    Register(mdns_sd::Error),
}

impl std::fmt::Display for MdnsAdvertisementError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Daemon(_) => write!(formatter, "failed to start mDNS daemon"),
            Self::ServiceInfo(_) => write!(formatter, "failed to build mDNS service info"),
            Self::Register(_) => write!(formatter, "failed to register mDNS service"),
        }
    }
}

impl std::error::Error for MdnsAdvertisementError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Daemon(source) | Self::ServiceInfo(source) | Self::Register(source) => {
                Some(source)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use sw_core::{
        CURRENT_AGENT_API_VERSION, TXT_KEY_API_VERSION, TXT_KEY_NODE_CERT_SHA256,
        TXT_KEY_NODE_NAME, TXT_KEY_NODE_UUID,
    };

    use super::*;

    fn node_uuid() -> NodeUuid {
        NodeUuid::new("00000000-0000-4000-8000-000000000004").expect("valid uuid")
    }

    #[test]
    fn service_info_uses_secondwind_service_type_and_runtime_port() {
        let service = node_service_info(node_uuid(), "node", "sha256:fingerprint", 49152)
            .expect("service info");

        assert_eq!(service.get_type(), SECONDWIND_MDNS_SERVICE_TYPE);
        assert_eq!(service.get_port(), 49152);
        assert!(service.is_addr_auto());
    }

    #[test]
    fn service_info_txt_contains_pairing_metadata() {
        let service = node_service_info(node_uuid(), "node", "sha256:fingerprint", 49152)
            .expect("service info");

        assert_eq!(
            service.get_property_val_str(TXT_KEY_NODE_UUID),
            Some("00000000-0000-4000-8000-000000000004")
        );
        assert_eq!(
            service.get_property_val_str(TXT_KEY_NODE_NAME),
            Some("node")
        );
        assert_eq!(
            service.get_property_val_str(TXT_KEY_NODE_CERT_SHA256),
            Some("sha256:fingerprint")
        );
        assert_eq!(
            service.get_property_val_str(TXT_KEY_API_VERSION),
            Some(CURRENT_AGENT_API_VERSION)
        );
    }
}
