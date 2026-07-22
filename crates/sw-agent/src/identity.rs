use std::{
    fs, io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sw_core::NodeUuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentIdentity {
    pub node_uuid: NodeUuid,
    pub node_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paired_host: Option<PairedHostTrust>,
    /// The node's own certificate fingerprint at the time trust was
    /// established. Lets startup detect a regenerated certificate store
    /// and reset pairing *intentionally* instead of silently breaking
    /// mTLS with a stale fingerprint (BUG-005).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_certificate_fingerprint: Option<String>,
}

impl AgentIdentity {
    pub fn new(node_name: String) -> Self {
        Self {
            node_uuid: NodeUuid::new_v4(),
            node_name,
            paired_host: None,
            node_certificate_fingerprint: None,
        }
    }
}

/// Outcome of matching the stored fingerprint against the live cert.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CertificateReconciliation {
    /// Fingerprint unchanged — nothing to do.
    Unchanged,
    /// First run (or pre-upgrade identity): fingerprint recorded.
    Recorded,
    /// The certificate changed under an established pairing: trust was
    /// cleared, the node returns to the pairing screen, and the host must
    /// re-pair. Never silent.
    TrustReset,
}

/// Reconciles the identity with the certificate the agent actually loaded.
/// Mutates `identity` (fingerprint always brought current; `paired_host`
/// cleared on a reset); the caller persists when the result is not
/// `Unchanged`.
pub fn reconcile_certificate(
    identity: &mut AgentIdentity,
    current_fingerprint: &str,
) -> CertificateReconciliation {
    match identity.node_certificate_fingerprint.as_deref() {
        Some(stored) if stored == current_fingerprint => CertificateReconciliation::Unchanged,
        Some(_) => {
            let was_paired = identity.paired_host.take().is_some();
            identity.node_certificate_fingerprint = Some(current_fingerprint.to_string());
            if was_paired {
                CertificateReconciliation::TrustReset
            } else {
                CertificateReconciliation::Recorded
            }
        }
        None => {
            identity.node_certificate_fingerprint = Some(current_fingerprint.to_string());
            CertificateReconciliation::Recorded
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairedHostTrust {
    pub host_name: String,
    pub host_certificate_fingerprint: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_certificate_pem: Option<String>,
}

pub fn load_or_create_identity(
    state_file: impl AsRef<Path>,
    default_node_name: impl Into<String>,
) -> Result<AgentIdentity, IdentityStoreError> {
    let state_file = state_file.as_ref();
    match fs::read_to_string(state_file) {
        Ok(contents) => {
            let identity = serde_json::from_str(&contents)
                .map_err(|source| IdentityStoreError::Parse { source })?;
            Ok(identity)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let identity = AgentIdentity::new(default_node_name.into());
            write_identity(state_file, &identity)?;
            Ok(identity)
        }
        Err(source) => Err(IdentityStoreError::Read {
            path: state_file.to_path_buf(),
            source,
        }),
    }
}

pub fn persist_paired_host(
    state_file: impl AsRef<Path>,
    paired_host: PairedHostTrust,
) -> Result<AgentIdentity, IdentityStoreError> {
    let state_file = state_file.as_ref();
    let contents = fs::read_to_string(state_file).map_err(|source| IdentityStoreError::Read {
        path: state_file.to_path_buf(),
        source,
    })?;
    let mut identity: AgentIdentity =
        serde_json::from_str(&contents).map_err(|source| IdentityStoreError::Parse { source })?;
    identity.paired_host = Some(paired_host);
    write_identity(state_file, &identity)?;
    Ok(identity)
}

pub fn write_identity(
    state_file: impl AsRef<Path>,
    identity: &AgentIdentity,
) -> Result<(), IdentityStoreError> {
    let state_file = state_file.as_ref();
    if let Some(parent) = state_file.parent() {
        fs::create_dir_all(parent).map_err(|source| IdentityStoreError::Write {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    let contents = serde_json::to_string_pretty(identity)
        .map_err(|source| IdentityStoreError::Serialize { source })?;
    // Atomic + owner-only: identity carries the paired host trust.
    sw_core::certificates::write_atomic(state_file, contents.as_bytes(), true).map_err(|_| {
        IdentityStoreError::Write {
            path: state_file.to_path_buf(),
            source: io::Error::other("atomic write failed"),
        }
    })
}

#[derive(Debug)]
pub enum IdentityStoreError {
    Read { path: PathBuf, source: io::Error },
    Write { path: PathBuf, source: io::Error },
    Parse { source: serde_json::Error },
    Serialize { source: serde_json::Error },
}

impl std::fmt::Display for IdentityStoreError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read { path, .. } => {
                write!(formatter, "failed to read identity from {}", path.display())
            }
            Self::Write { path, .. } => {
                write!(formatter, "failed to write identity to {}", path.display())
            }
            Self::Parse { .. } => write!(formatter, "failed to parse identity state"),
            Self::Serialize { .. } => write!(formatter, "failed to serialize identity state"),
        }
    }
}

impl std::error::Error for IdentityStoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read { source, .. } | Self::Write { source, .. } => Some(source),
            Self::Parse { source } | Self::Serialize { source } => Some(source),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempIdentityFile {
        path: PathBuf,
    }

    impl TempIdentityFile {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "secondwind-identity-{}-{name}.json",
                std::process::id()
            ));
            let _ = fs::remove_file(&path);
            Self { path }
        }
    }

    impl Drop for TempIdentityFile {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.path);
        }
    }

    #[test]
    fn creates_identity_when_missing() {
        let file = TempIdentityFile::new("creates");

        let identity = load_or_create_identity(&file.path, "node-name")
            .expect("create identity on first boot");

        assert_eq!(identity.node_name, "node-name");
        assert!(identity.paired_host.is_none());
        assert!(file.path.exists());
    }

    #[test]
    fn keeps_existing_identity() {
        let file = TempIdentityFile::new("keeps");
        let original = AgentIdentity {
            node_uuid: NodeUuid::new("00000000-0000-4000-8000-000000000003").expect("valid uuid"),
            node_name: "original".to_string(),
            paired_host: None,
            node_certificate_fingerprint: None,
        };
        write_identity(&file.path, &original).expect("write identity");

        let loaded =
            load_or_create_identity(&file.path, "different").expect("load existing identity");

        assert_eq!(loaded, original);
    }

    #[test]
    fn loads_legacy_identity_without_paired_host() {
        let file = TempIdentityFile::new("legacy");
        fs::write(
            &file.path,
            r#"{
  "node_uuid": "00000000-0000-4000-8000-000000000003",
  "node_name": "legacy"
}"#,
        )
        .expect("write legacy identity");

        let loaded = load_or_create_identity(&file.path, "different").expect("load legacy");

        assert_eq!(loaded.node_name, "legacy");
        assert!(loaded.paired_host.is_none());
    }

    #[test]
    fn certificate_change_under_pairing_resets_trust_explicitly() {
        let mut identity = AgentIdentity {
            node_uuid: NodeUuid::new("00000000-0000-4000-8000-000000000003").expect("valid uuid"),
            node_name: "node".to_string(),
            paired_host: Some(PairedHostTrust {
                host_name: "host".to_string(),
                host_certificate_fingerprint: "sha256:host".to_string(),
                host_certificate_pem: Some("pem".to_string()),
            }),
            node_certificate_fingerprint: Some("sha256:OLD".to_string()),
        };

        let outcome = reconcile_certificate(&mut identity, "sha256:NEW");

        assert_eq!(outcome, CertificateReconciliation::TrustReset);
        assert!(identity.paired_host.is_none());
        assert_eq!(
            identity.node_certificate_fingerprint.as_deref(),
            Some("sha256:NEW")
        );
    }

    #[test]
    fn unchanged_certificate_keeps_trust() {
        let mut identity = AgentIdentity {
            node_uuid: NodeUuid::new("00000000-0000-4000-8000-000000000003").expect("valid uuid"),
            node_name: "node".to_string(),
            paired_host: Some(PairedHostTrust {
                host_name: "host".to_string(),
                host_certificate_fingerprint: "sha256:host".to_string(),
                host_certificate_pem: Some("pem".to_string()),
            }),
            node_certificate_fingerprint: Some("sha256:SAME".to_string()),
        };

        let outcome = reconcile_certificate(&mut identity, "sha256:SAME");

        assert_eq!(outcome, CertificateReconciliation::Unchanged);
        assert!(identity.paired_host.is_some());
    }

    #[test]
    fn first_run_records_fingerprint_without_reset() {
        let mut identity = AgentIdentity::new("node".to_string());

        let outcome = reconcile_certificate(&mut identity, "sha256:FIRST");

        assert_eq!(outcome, CertificateReconciliation::Recorded);
        assert_eq!(
            identity.node_certificate_fingerprint.as_deref(),
            Some("sha256:FIRST")
        );
    }

    #[test]
    fn persists_paired_host_trust() {
        let file = TempIdentityFile::new("trust");
        let original = AgentIdentity {
            node_uuid: NodeUuid::new("00000000-0000-4000-8000-000000000003").expect("valid uuid"),
            node_name: "node".to_string(),
            paired_host: None,
            node_certificate_fingerprint: None,
        };
        write_identity(&file.path, &original).expect("write identity");

        persist_paired_host(
            &file.path,
            PairedHostTrust {
                host_name: "host".to_string(),
                host_certificate_fingerprint: "sha256:host".to_string(),
                host_certificate_pem: Some("host certificate".to_string()),
            },
        )
        .expect("persist host trust");
        let loaded = load_or_create_identity(&file.path, "different").expect("load identity");

        assert_eq!(loaded.node_uuid, original.node_uuid);
        assert_eq!(loaded.paired_host.expect("paired host").host_name, "host");
    }
    #[test]
    fn persisting_paired_host_requires_existing_identity() {
        let file = TempIdentityFile::new("missing-trust");

        let error = persist_paired_host(
            &file.path,
            PairedHostTrust {
                host_name: "host".to_string(),
                host_certificate_fingerprint: "sha256:host".to_string(),
                host_certificate_pem: Some("host certificate".to_string()),
            },
        )
        .expect_err("missing identity should fail");

        assert!(matches!(error, IdentityStoreError::Read { .. }));
    }
}
