//! Shared agent ↔ kiosk contract.
//!
//! The agent owns node state; the kiosk owns the physical screen. They meet
//! at a small JSON state file (path supplied by configuration, e.g. a
//! tmpfs-backed runtime dir in the node image). The agent rewrites the file
//! atomically on every transition and the kiosk watches it.
//!
//! This is also the only place the pairing PIN/QR travel: node-local disk to
//! node-local screen. They are never served over the network API.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

/// What the node's screen should currently show.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum KioskState {
    /// Agent not ready yet (or file missing): show a neutral starting screen.
    Starting,
    /// Not paired: show the pairing screen (name, QR, PIN).
    Unpaired {
        node_name: String,
        pin: String,
        /// JSON-encoded [`crate::PairingQrPayload`] ready for QR rendering.
        qr_payload: String,
        certificate_fingerprint: String,
    },
    /// Paired but no active session: ambient "waiting for host" screen.
    Idle {
        node_name: String,
        paired_host_name: String,
    },
    /// Host requested the screen: run the streaming client supervised.
    Streaming {
        paired_host_name: String,
        /// Address of the host as observed by the agent; where the
        /// streaming client connects.
        host_address: String,
        /// One-shot PIN for the streaming client's own pairing with the
        /// host's streaming server, present only until that pairing exists.
        stream_pair_pin: Option<String>,
    },
}

pub fn read_kiosk_state(path: impl AsRef<Path>) -> Result<KioskState, KioskStateError> {
    let path = path.as_ref();
    match fs::read_to_string(path) {
        Ok(contents) => {
            serde_json::from_str(&contents).map_err(|source| KioskStateError::Parse { source })
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(KioskState::Starting),
        Err(source) => Err(KioskStateError::Read {
            path: path.to_path_buf(),
            source,
        }),
    }
}

/// Writes the state atomically (temp file + rename) so the kiosk never
/// observes a half-written file.
pub fn write_kiosk_state(
    path: impl AsRef<Path>,
    state: &KioskState,
) -> Result<(), KioskStateError> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| KioskStateError::Write {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    let contents = serde_json::to_string_pretty(state)
        .map_err(|source| KioskStateError::Serialize { source })?;
    // Append ".tmp" (never replace the extension) so the temp name can't
    // collide with a sibling file, and stay in the same directory so the
    // rename is atomic.
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default();
    let temp_path = path.with_file_name(format!("{file_name}.tmp"));
    fs::write(&temp_path, contents).map_err(|source| KioskStateError::Write {
        path: temp_path.clone(),
        source,
    })?;
    fs::rename(&temp_path, path).map_err(|source| KioskStateError::Write {
        path: path.to_path_buf(),
        source,
    })
}

#[derive(Debug)]
pub enum KioskStateError {
    Read { path: PathBuf, source: io::Error },
    Write { path: PathBuf, source: io::Error },
    Parse { source: serde_json::Error },
    Serialize { source: serde_json::Error },
}

impl std::fmt::Display for KioskStateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read { path, .. } => {
                write!(formatter, "failed to read kiosk state {}", path.display())
            }
            Self::Write { path, .. } => {
                write!(formatter, "failed to write kiosk state {}", path.display())
            }
            Self::Parse { .. } => write!(formatter, "failed to parse kiosk state"),
            Self::Serialize { .. } => write!(formatter, "failed to serialize kiosk state"),
        }
    }
}

impl std::error::Error for KioskStateError {
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

    struct TempStateFile {
        path: PathBuf,
    }

    impl TempStateFile {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "secondwind-kiosk-state-{}-{name}.json",
                std::process::id()
            ));
            let _ = fs::remove_file(&path);
            Self { path }
        }
    }

    impl Drop for TempStateFile {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.path);
            let _ = fs::remove_file(self.path.with_file_name(format!(
                "{}.tmp",
                self.path.file_name().expect("name").to_string_lossy()
            )));
        }
    }

    #[test]
    fn missing_file_reads_as_starting() {
        let file = TempStateFile::new("missing");

        let state = read_kiosk_state(&file.path).expect("read missing state");

        assert_eq!(state, KioskState::Starting);
    }

    #[test]
    fn kiosk_state_round_trips() {
        let file = TempStateFile::new("round-trip");
        let state = KioskState::Unpaired {
            node_name: "node".to_string(),
            pin: "123456".to_string(),
            qr_payload: "{\"schema_version\":1}".to_string(),
            certificate_fingerprint: "sha256:fingerprint".to_string(),
        };

        write_kiosk_state(&file.path, &state).expect("write state");
        let loaded = read_kiosk_state(&file.path).expect("read state");

        assert_eq!(loaded, state);
    }

    #[test]
    fn write_replaces_previous_state_atomically() {
        let file = TempStateFile::new("replace");
        write_kiosk_state(
            &file.path,
            &KioskState::Idle {
                node_name: "node".to_string(),
                paired_host_name: "host".to_string(),
            },
        )
        .expect("write idle");

        write_kiosk_state(
            &file.path,
            &KioskState::Streaming {
                paired_host_name: "host".to_string(),
                host_address: "peer-address".to_string(),
                stream_pair_pin: None,
            },
        )
        .expect("write streaming");

        let loaded = read_kiosk_state(&file.path).expect("read state");
        assert!(matches!(loaded, KioskState::Streaming { .. }));
        let temp = file.path.with_file_name(format!(
            "{}.tmp",
            file.path.file_name().expect("name").to_string_lossy()
        ));
        assert!(!temp.exists());
    }
}
