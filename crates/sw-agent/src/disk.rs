//! Node disk exposure (v0.2).
//!
//! The node image designates one data partition at install time and ships a
//! `secondwind-disk` systemd unit that builds the iSCSI (LIO) target from
//! `/etc/secondwind/disk.env`. The agent's job is small and auditable:
//! report state, start/stop that unit, and hand the paired host the
//! connection details + CHAP credentials over mTLS. Only the designated
//! partition is ever exposed — the agent has no way to name any other
//! device.

use std::{
    fs, io,
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
};

use sw_core::{DiskState, DiskTarget};

pub const DISK_ENV_FILE_ENV: &str = "SECONDWIND_DISK_ENV_FILE";
pub const DISK_UNIT_ENV: &str = "SECONDWIND_DISK_UNIT";
const DEFAULT_DISK_UNIT: &str = "secondwind-disk.service";
const DEFAULT_ISCSI_PORT: u16 = 3260;

/// Parsed `/etc/secondwind/disk.env` (written at install time).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiskProvisioning {
    pub device: String,
    pub target_iqn: String,
    pub port: u16,
    pub chap_username: String,
    pub chap_secret: String,
}

/// Parses the simple KEY=VALUE env file the installer writes. Unknown keys
/// are ignored so the file can grow.
pub fn parse_disk_env(contents: &str) -> Option<DiskProvisioning> {
    let mut device = None;
    let mut iqn = None;
    let mut port = None;
    let mut chap_username = None;
    let mut chap_secret = None;

    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim().trim_matches('"').to_string();
        match key.trim() {
            "SECONDWIND_DISK_DEVICE" => device = Some(value),
            "SECONDWIND_DISK_IQN" => iqn = Some(value),
            "SECONDWIND_DISK_PORT" => port = value.parse().ok(),
            "SECONDWIND_DISK_CHAP_USER" => chap_username = Some(value),
            "SECONDWIND_DISK_CHAP_SECRET" => chap_secret = Some(value),
            _ => {}
        }
    }

    Some(DiskProvisioning {
        device: device?,
        target_iqn: iqn?,
        port: port.unwrap_or(DEFAULT_ISCSI_PORT),
        chap_username: chap_username?,
        chap_secret: chap_secret?,
    })
}

/// Abstract control surface so API logic is testable without systemd.
pub trait DiskController: Send + Sync + std::fmt::Debug {
    fn provisioning(&self) -> Option<DiskProvisioning>;
    fn is_exported(&self) -> bool;
    fn export(&self) -> Result<(), DiskControlError>;
    fn unexport(&self) -> Result<(), DiskControlError>;
}

pub type SharedDiskController = Arc<dyn DiskController>;

/// Production controller: provisioning from the disk env file, exported
/// state via the `secondwind-disk` systemd unit (a polkit rule installed by
/// the image lets the agent user manage exactly this unit).
#[derive(Debug)]
pub struct SystemdDiskController {
    env_file: PathBuf,
    unit: String,
}

impl SystemdDiskController {
    pub fn from_env() -> Option<Self> {
        let env_file = std::env::var_os(DISK_ENV_FILE_ENV)
            .map(PathBuf::from)
            .filter(|path| !path.as_os_str().is_empty())?;
        let unit = std::env::var(DISK_UNIT_ENV)
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_DISK_UNIT.to_string());

        Some(Self { env_file, unit })
    }

    fn systemctl(&self, verb: &str) -> Result<bool, DiskControlError> {
        Command::new("systemctl")
            .arg(verb)
            .arg(&self.unit)
            .status()
            .map(|status| status.success())
            .map_err(|source| DiskControlError::Systemctl { source })
    }
}

impl DiskController for SystemdDiskController {
    fn provisioning(&self) -> Option<DiskProvisioning> {
        read_provisioning(&self.env_file)
    }

    fn is_exported(&self) -> bool {
        self.systemctl("is-active").unwrap_or(false)
    }

    fn export(&self) -> Result<(), DiskControlError> {
        if self.systemctl("start")? {
            Ok(())
        } else {
            Err(DiskControlError::UnitFailed)
        }
    }

    fn unexport(&self) -> Result<(), DiskControlError> {
        if self.systemctl("stop")? {
            Ok(())
        } else {
            Err(DiskControlError::UnitFailed)
        }
    }
}

pub fn read_provisioning(env_file: &Path) -> Option<DiskProvisioning> {
    match fs::read_to_string(env_file) {
        Ok(contents) => parse_disk_env(&contents),
        Err(_) => None,
    }
}

pub fn disk_state(controller: &dyn DiskController) -> DiskState {
    match controller.provisioning() {
        None => DiskState::Unavailable {
            reason: "This node has no SecondWind data disk configured.".to_string(),
        },
        Some(provisioning) => {
            if controller.is_exported() {
                DiskState::Exposed {
                    target: DiskTarget {
                        target_iqn: provisioning.target_iqn,
                        port: provisioning.port,
                        chap_username: provisioning.chap_username,
                        chap_secret: provisioning.chap_secret,
                    },
                }
            } else {
                DiskState::Ready
            }
        }
    }
}

#[derive(Debug)]
pub enum DiskControlError {
    Systemctl { source: io::Error },
    UnitFailed,
}

impl std::fmt::Display for DiskControlError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Systemctl { .. } => write!(formatter, "could not control the disk service"),
            Self::UnitFailed => write!(formatter, "the disk service refused the request"),
        }
    }
}

impl std::error::Error for DiskControlError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Systemctl { source } => Some(source),
            Self::UnitFailed => None,
        }
    }
}

#[cfg(test)]
pub mod test_support {
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::*;

    #[derive(Debug, Default)]
    pub struct FakeDiskController {
        pub provisioning: Option<DiskProvisioning>,
        pub exported: AtomicBool,
    }

    impl DiskController for FakeDiskController {
        fn provisioning(&self) -> Option<DiskProvisioning> {
            self.provisioning.clone()
        }

        fn is_exported(&self) -> bool {
            self.exported.load(Ordering::SeqCst)
        }

        fn export(&self) -> Result<(), DiskControlError> {
            if self.provisioning.is_none() {
                return Err(DiskControlError::UnitFailed);
            }
            self.exported.store(true, Ordering::SeqCst);
            Ok(())
        }

        fn unexport(&self) -> Result<(), DiskControlError> {
            self.exported.store(false, Ordering::SeqCst);
            Ok(())
        }
    }

    pub fn provisioned() -> DiskProvisioning {
        DiskProvisioning {
            device: "/dev/disk/by-partlabel/SECONDWIND_DATA".to_string(),
            target_iqn: "iqn.2026-07.app.secondwind:node-test".to_string(),
            port: 3260,
            chap_username: "secondwind".to_string(),
            chap_secret: "secret".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{test_support::*, *};

    #[test]
    fn parses_installer_disk_env() {
        let env = r#"
# written by the SecondWind installer
SECONDWIND_DISK_DEVICE=/dev/disk/by-partlabel/SECONDWIND_DATA
SECONDWIND_DISK_IQN=iqn.2026-07.app.secondwind:node-abc
SECONDWIND_DISK_PORT=3260
SECONDWIND_DISK_CHAP_USER=secondwind
SECONDWIND_DISK_CHAP_SECRET="s3cr3t"
"#;

        let provisioning = parse_disk_env(env).expect("provisioning");

        assert_eq!(
            provisioning.device,
            "/dev/disk/by-partlabel/SECONDWIND_DATA"
        );
        assert_eq!(
            provisioning.target_iqn,
            "iqn.2026-07.app.secondwind:node-abc"
        );
        assert_eq!(provisioning.port, 3260);
        assert_eq!(provisioning.chap_secret, "s3cr3t");
    }

    #[test]
    fn missing_keys_mean_unprovisioned() {
        assert!(parse_disk_env("SECONDWIND_DISK_DEVICE=/dev/sda3").is_none());
        assert!(parse_disk_env("").is_none());
    }

    #[test]
    fn state_reflects_provisioning_and_export() {
        let unprovisioned = FakeDiskController::default();
        assert!(matches!(
            disk_state(&unprovisioned),
            DiskState::Unavailable { .. }
        ));

        let controller = FakeDiskController {
            provisioning: Some(provisioned()),
            ..Default::default()
        };
        assert_eq!(disk_state(&controller), DiskState::Ready);

        controller.export().expect("export");
        assert!(matches!(disk_state(&controller), DiskState::Exposed { .. }));
    }
}
