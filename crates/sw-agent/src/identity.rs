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
}

impl AgentIdentity {
    pub fn new(node_name: String) -> Self {
        Self {
            node_uuid: NodeUuid::new_v4(),
            node_name,
        }
    }
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
    fs::write(state_file, contents).map_err(|source| IdentityStoreError::Write {
        path: state_file.to_path_buf(),
        source,
    })
}

#[derive(Debug)]
pub enum IdentityStoreError {
    Read { path: PathBuf, source: io::Error },
    Write { path: PathBuf, source: io::Error },
    Parse { source: serde_json::Error },
    Serialize { source: serde_json::Error },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "secondwind-identity-{}-{name}.json",
            std::process::id()
        ))
    }

    #[test]
    fn creates_identity_when_missing() {
        let path = test_path("creates");
        let _ = fs::remove_file(&path);

        let identity =
            load_or_create_identity(&path, "node-name").expect("create identity on first boot");

        assert_eq!(identity.node_name, "node-name");
        assert!(path.exists());

        let _ = fs::remove_file(path);
    }

    #[test]
    fn keeps_existing_identity() {
        let path = test_path("keeps");
        let _ = fs::remove_file(&path);
        let original = AgentIdentity {
            node_uuid: NodeUuid::new("00000000-0000-4000-8000-000000000003").expect("valid uuid"),
            node_name: "original".to_string(),
        };
        write_identity(&path, &original).expect("write identity");

        let loaded = load_or_create_identity(&path, "different").expect("load existing identity");

        assert_eq!(loaded, original);

        let _ = fs::remove_file(path);
    }
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
