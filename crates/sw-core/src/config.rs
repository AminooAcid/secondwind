use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::capabilities::PanelMode;

pub const CURRENT_CONFIG_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NodeUuid(Uuid);

impl NodeUuid {
    pub fn new(value: impl AsRef<str>) -> Result<Self, ConfigError> {
        let value = value.as_ref().trim();
        let uuid = Uuid::parse_str(value).map_err(|_| ConfigError::InvalidNodeUuid)?;

        Ok(Self(uuid))
    }

    pub fn new_v4() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl std::fmt::Display for NodeUuid {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    InvalidNodeUuid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecondWindConfig {
    pub config_version: u16,
    pub host: HostConfig,
    pub nodes: BTreeMap<NodeUuid, NodeConfig>,
    /// v0.3 app library: catalog + per-app policy. Defaults to the v1
    /// catalog on first read of an older config.
    #[serde(default = "crate::apps::default_catalog")]
    pub apps: Vec<crate::apps::AppCatalogEntry>,
}

impl SecondWindConfig {
    pub fn empty(host: HostConfig) -> Self {
        Self {
            config_version: CURRENT_CONFIG_VERSION,
            host,
            nodes: BTreeMap::new(),
            apps: crate::apps::default_catalog(),
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
    /// v0.2 disk feature; defaulted so v0.1 configs migrate forward.
    #[serde(default)]
    pub disk: DiskFeatureConfig,
    /// v0.3: Wake-on-LAN targets learned from the node's capabilities at
    /// pairing time; defaulted for older configs.
    #[serde(default)]
    pub wake: WakeConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct WakeConfig {
    /// MACs to send magic packets to (every detected interface).
    pub mac_addresses: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiskFeatureConfig {
    /// Bring the node disk up automatically with the link.
    pub always_on: bool,
    /// Preferred Windows drive letter; None lets Windows pick.
    pub drive_letter: Option<char>,
}

impl Default for DiskFeatureConfig {
    fn default() -> Self {
        Self {
            always_on: true,
            drive_letter: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeTrust {
    pub peer_certificate_fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScreenConfig {
    pub always_on: bool,
    pub preferred_panel_mode: Option<PanelMode>,
    /// Whether the node's streaming client has already completed its own
    /// one-time pairing with the host's streaming server. Defaults false so
    /// existing configs migrate forward.
    #[serde(default)]
    pub stream_paired: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_round_trips_as_json() {
        let node_id =
            NodeUuid::new("00000000-0000-4000-8000-000000000001").expect("valid test uuid");
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
                    stream_paired: false,
                },
                disk: DiskFeatureConfig::default(),
                wake: WakeConfig {
                    mac_addresses: vec!["aa:bb:cc:dd:ee:ff".to_string()],
                },
            },
        );

        let json = serde_json::to_string(&config).expect("serialize config");
        let decoded: SecondWindConfig = serde_json::from_str(&json).expect("deserialize config");

        assert_eq!(decoded, config);
    }

    #[test]
    fn node_uuid_rejects_invalid_values() {
        assert_eq!(NodeUuid::new("  "), Err(ConfigError::InvalidNodeUuid));
        assert_eq!(
            NodeUuid::new("not-a-uuid"),
            Err(ConfigError::InvalidNodeUuid)
        );
    }
}
