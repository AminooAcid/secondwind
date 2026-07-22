//! Host-side persistent state: the host identity certificate and the
//! UUID-keyed paired-node configuration.
//!
//! All state lives under one directory supplied by the caller (Tauri app data
//! dir in production, a temp dir in tests, or `SECONDWIND_COMPANION_STATE_DIR`
//! for development). No paths, names, or hardware values are baked in.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use sw_core::{
    CertificateMaterial, CertificateStoreError, DiskFeatureConfig, HostConfig, NodeConfig,
    NodeTrust, NodeUuid, ScreenConfig, SecondWindConfig, WakeConfig, load_or_create_certificate,
};

pub const CONFIG_FILE_NAME: &str = "config.json";
pub const HOST_CERTIFICATE_FILE_NAME: &str = "host-cert.pem";
pub const HOST_PRIVATE_KEY_FILE_NAME: &str = "host-key.pem";

#[derive(Debug, Clone)]
pub struct HostState {
    pub config: SecondWindConfig,
    pub certificate: CertificateMaterial,
    root: PathBuf,
}

impl HostState {
    /// Loads (or creates on first run) the host identity and paired-node
    /// config under `root`.
    pub fn load_or_create(root: impl AsRef<Path>) -> Result<Self, HostStateError> {
        let root = root.as_ref().to_path_buf();
        let config_file = root.join(CONFIG_FILE_NAME);

        let config = match fs::read_to_string(&config_file) {
            Ok(contents) => serde_json::from_str(&contents)
                .map_err(|source| HostStateError::Parse { source })?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let config = SecondWindConfig::empty(HostConfig {
                    display_name: default_host_display_name(),
                });
                write_config(&config_file, &config)?;
                config
            }
            Err(source) => {
                return Err(HostStateError::Read {
                    path: config_file,
                    source,
                });
            }
        };

        let certificate = load_or_create_certificate(
            root.join(HOST_CERTIFICATE_FILE_NAME),
            root.join(HOST_PRIVATE_KEY_FILE_NAME),
            &config.host.display_name,
        )?;

        Ok(Self {
            config,
            certificate,
            root,
        })
    }

    pub fn save(&self) -> Result<(), HostStateError> {
        write_config(&self.root.join(CONFIG_FILE_NAME), &self.config)
    }

    /// Records trust in a freshly paired node and persists it. Screen
    /// defaults to always-on so link-up auto-connect brings it up (product
    /// law: everything auto-connects after one-time pairing).
    pub fn record_paired_node(
        &mut self,
        node_uuid: NodeUuid,
        display_name: String,
        node_certificate_fingerprint: String,
    ) -> Result<(), HostStateError> {
        self.config.nodes.insert(
            node_uuid,
            NodeConfig {
                display_name,
                trust: NodeTrust {
                    peer_certificate_fingerprint: node_certificate_fingerprint,
                },
                screen: ScreenConfig {
                    always_on: true,
                    preferred_panel_mode: None,
                    stream_paired: false,
                },
                disk: DiskFeatureConfig::default(),
                wake: WakeConfig::default(),
                usb_auto_attach: Vec::new(),
            },
        );
        self.save()
    }

    /// Stores the node's Wake-on-LAN targets (learned from capabilities
    /// right after pairing).
    pub fn record_wake_targets(
        &mut self,
        node_uuid: &NodeUuid,
        mac_addresses: Vec<String>,
    ) -> Result<(), HostStateError> {
        if let Some(node) = self.config.nodes.get_mut(node_uuid) {
            node.wake = WakeConfig { mac_addresses };
            self.save()?;
        }

        Ok(())
    }

    pub fn paired_node(&self, node_uuid: &NodeUuid) -> Option<&NodeConfig> {
        self.config.nodes.get(node_uuid)
    }

    /// Marks the node's streaming client as paired with the host's streaming
    /// server, so future connects skip the one-shot stream PIN.
    pub fn mark_stream_paired(&mut self, node_uuid: &NodeUuid) -> Result<(), HostStateError> {
        if let Some(node) = self.config.nodes.get_mut(node_uuid) {
            node.screen.stream_paired = true;
            self.save()?;
        }

        Ok(())
    }
}

fn write_config(config_file: &Path, config: &SecondWindConfig) -> Result<(), HostStateError> {
    if let Some(parent) = config_file.parent() {
        fs::create_dir_all(parent).map_err(|source| HostStateError::Write {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    let contents = serde_json::to_string_pretty(config)
        .map_err(|source| HostStateError::Serialize { source })?;
    // Atomic so a crash mid-save never corrupts node trust.
    sw_core::certificates::write_atomic(config_file, contents.as_bytes(), false).map_err(|_| {
        HostStateError::Write {
            path: config_file.to_path_buf(),
            source: io::Error::other("atomic write failed"),
        }
    })
}

/// Detected host display name; never hardcoded. Falls back to a product
/// default when the OS exposes nothing.
pub fn default_host_display_name() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .ok()
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| "SecondWind host".to_string())
}

#[derive(Debug)]
pub enum HostStateError {
    Read { path: PathBuf, source: io::Error },
    Write { path: PathBuf, source: io::Error },
    Parse { source: serde_json::Error },
    Serialize { source: serde_json::Error },
    Certificate(CertificateStoreError),
}

impl From<CertificateStoreError> for HostStateError {
    fn from(source: CertificateStoreError) -> Self {
        Self::Certificate(source)
    }
}

impl std::fmt::Display for HostStateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read { path, .. } => {
                write!(
                    formatter,
                    "failed to read companion state from {}",
                    path.display()
                )
            }
            Self::Write { path, .. } => {
                write!(
                    formatter,
                    "failed to write companion state to {}",
                    path.display()
                )
            }
            Self::Parse { .. } => write!(formatter, "failed to parse companion state"),
            Self::Serialize { .. } => write!(formatter, "failed to serialize companion state"),
            Self::Certificate(_) => write!(formatter, "failed to prepare the host certificate"),
        }
    }
}

impl std::error::Error for HostStateError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read { source, .. } | Self::Write { source, .. } => Some(source),
            Self::Parse { source } | Self::Serialize { source } => Some(source),
            Self::Certificate(source) => Some(source),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempStateDir {
        root: PathBuf,
    }

    impl TempStateDir {
        fn new(name: &str) -> Self {
            let root = std::env::temp_dir().join(format!(
                "secondwind-host-state-{}-{name}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&root);
            Self { root }
        }
    }

    impl Drop for TempStateDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn creates_host_identity_on_first_run() {
        let dir = TempStateDir::new("creates");

        let state = HostState::load_or_create(&dir.root).expect("create host state");

        assert!(!state.config.host.display_name.trim().is_empty());
        assert!(state.certificate.fingerprint.starts_with("sha256:"));
        assert!(dir.root.join(CONFIG_FILE_NAME).exists());
        assert!(dir.root.join(HOST_CERTIFICATE_FILE_NAME).exists());
        assert!(dir.root.join(HOST_PRIVATE_KEY_FILE_NAME).exists());
    }

    #[test]
    fn keeps_host_identity_across_runs() {
        let dir = TempStateDir::new("keeps");
        let first = HostState::load_or_create(&dir.root).expect("create host state");

        let second = HostState::load_or_create(&dir.root).expect("reload host state");

        assert_eq!(
            second.certificate.fingerprint,
            first.certificate.fingerprint
        );
        assert_eq!(
            second.config.host.display_name,
            first.config.host.display_name
        );
    }

    #[test]
    fn records_paired_node_with_screen_always_on() {
        let dir = TempStateDir::new("records");
        let mut state = HostState::load_or_create(&dir.root).expect("create host state");
        let node_uuid = NodeUuid::new("00000000-0000-4000-8000-000000000005").expect("valid uuid");

        state
            .record_paired_node(
                node_uuid,
                "node".to_string(),
                "sha256:node-fingerprint".to_string(),
            )
            .expect("record paired node");
        let reloaded = HostState::load_or_create(&dir.root).expect("reload host state");

        let node = reloaded.paired_node(&node_uuid).expect("paired node");
        assert_eq!(node.display_name, "node");
        assert_eq!(
            node.trust.peer_certificate_fingerprint,
            "sha256:node-fingerprint"
        );
        assert!(node.screen.always_on);
    }
}
