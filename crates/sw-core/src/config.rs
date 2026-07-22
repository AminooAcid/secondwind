use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::capabilities::PanelMode;

pub const CURRENT_CONFIG_VERSION: u16 = 1;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NodeUuid(String);

impl NodeUuid {
    pub fn new(value: impl Into<String>) -> Result<Self, ConfigError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(ConfigError::EmptyNodeUuid);
        }

        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    EmptyNodeUuid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecondWindConfig {
    pub config_version: u16,
    pub host: HostConfig,
    pub nodes: BTreeMap<NodeUuid, NodeConfig>,
}

impl SecondWindConfig {
    pub fn empty(host: HostConfig) -> Self {
        Self {
            config_version: CURRENT_CONFIG_VERSION,
            host,
            nodes: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostConfig {
    pub display_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeConfig {
    pub display_name: String,
    pub trust: NodeTrust,
    pub screen: ScreenConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeTrust {
    pub peer_certificate_fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScreenConfig {
    pub always_on: bool,
    pub preferred_panel_mode: Option<PanelMode>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_round_trips_as_json() {
        let node_id = NodeUuid::new("node-test-uuid").expect("valid test uuid");
        let mut config = SecondWindConfig::empty(HostConfig {
            display_name: "host".to_string(),
        });
        config.nodes.insert(
            node_id,
            NodeConfig {
                display_name: "node".to_string(),
                trust: NodeTrust {
                    peer_certificate_fingerprint: "sha256:test".to_string(),
                },
                screen: ScreenConfig {
                    always_on: true,
                    preferred_panel_mode: Some(PanelMode {
                        width_px: 1,
                        height_px: 1,
                        refresh_millihz: 1,
                    }),
                },
            },
        );

        let json = serde_json::to_string(&config).expect("serialize config");
        let decoded: SecondWindConfig = serde_json::from_str(&json).expect("deserialize config");

        assert_eq!(decoded, config);
    }

    #[test]
    fn node_uuid_rejects_empty_values() {
        assert_eq!(NodeUuid::new("  "), Err(ConfigError::EmptyNodeUuid));
    }
}
