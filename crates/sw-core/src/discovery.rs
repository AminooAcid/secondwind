use serde::{Deserialize, Serialize};

pub const SECONDWIND_MDNS_SERVICE_TYPE: &str = "_secondwind._tcp.local.";
pub const TXT_KEY_NODE_UUID: &str = "node_uuid";
pub const TXT_KEY_NODE_NAME: &str = "node_name";
pub const TXT_KEY_NODE_CERT_SHA256: &str = "node_cert_sha256";
pub const TXT_KEY_API_VERSION: &str = "api_version";
pub const CURRENT_AGENT_API_VERSION: &str = "v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeAdvertisementTxt {
    pub node_uuid: String,
    pub node_name: String,
    pub node_certificate_fingerprint: String,
    pub api_version: String,
}

impl NodeAdvertisementTxt {
    pub fn new(
        node_uuid: impl Into<String>,
        node_name: impl Into<String>,
        node_certificate_fingerprint: impl Into<String>,
    ) -> Self {
        Self {
            node_uuid: node_uuid.into(),
            node_name: node_name.into(),
            node_certificate_fingerprint: node_certificate_fingerprint.into(),
            api_version: CURRENT_AGENT_API_VERSION.to_string(),
        }
    }

    pub fn as_txt_pairs(&self) -> [(String, String); 4] {
        [
            (TXT_KEY_NODE_UUID.to_string(), self.node_uuid.clone()),
            (TXT_KEY_NODE_NAME.to_string(), self.node_name.clone()),
            (
                TXT_KEY_NODE_CERT_SHA256.to_string(),
                self.node_certificate_fingerprint.clone(),
            ),
            (TXT_KEY_API_VERSION.to_string(), self.api_version.clone()),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secondwind_service_type_is_tcp_local() {
        assert_eq!(SECONDWIND_MDNS_SERVICE_TYPE, "_secondwind._tcp.local.");
    }

    #[test]
    fn advertisement_txt_contains_discovery_contract() {
        let txt = NodeAdvertisementTxt::new("node-id", "node", "sha256:fingerprint");
        let pairs = txt.as_txt_pairs();

        assert!(pairs.contains(&(TXT_KEY_NODE_UUID.to_string(), "node-id".to_string())));
        assert!(pairs.contains(&(TXT_KEY_NODE_NAME.to_string(), "node".to_string())));
        assert!(pairs.contains(&(
            TXT_KEY_NODE_CERT_SHA256.to_string(),
            "sha256:fingerprint".to_string()
        )));
        assert!(pairs.contains(&(
            TXT_KEY_API_VERSION.to_string(),
            CURRENT_AGENT_API_VERSION.to_string()
        )));
    }
}
