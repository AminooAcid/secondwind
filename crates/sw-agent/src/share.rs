//! Host file-share mount on the node (v0.3).
//!
//! The host exposes one dedicated SecondWind folder over SMB with a
//! dedicated account; the paired host sends the UNC + credentials over
//! mTLS. The agent writes a root-owned credentials file and starts the
//! `secondwind-share` systemd unit (polkit-scoped like the disk unit),
//! whose script mounts the share at the configured mountpoint. Apps read
//! and write user files through this mount — nothing of the user's
//! persists on the node.

use std::{
    fs, io,
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
};

use sw_core::{ShareConfigRequest, ShareState};

pub const SHARE_ENV_FILE_ENV: &str = "SECONDWIND_SHARE_ENV_FILE";
pub const SHARE_UNIT_ENV: &str = "SECONDWIND_SHARE_UNIT";
const DEFAULT_SHARE_UNIT: &str = "secondwind-share.service";

pub trait ShareController: Send + Sync + std::fmt::Debug {
    fn is_configured(&self) -> bool;
    fn is_mounted(&self) -> bool;
    fn configure_and_mount(&self, request: &ShareConfigRequest) -> Result<(), ShareControlError>;
    fn unmount(&self) -> Result<(), ShareControlError>;
}

pub type SharedShareController = Arc<dyn ShareController>;

#[derive(Debug)]
pub struct SystemdShareController {
    env_file: PathBuf,
    unit: String,
}

impl SystemdShareController {
    pub fn from_env() -> Option<Self> {
        let env_file = std::env::var_os(SHARE_ENV_FILE_ENV)
            .map(PathBuf::from)
            .filter(|path| !path.as_os_str().is_empty())?;
        let unit = std::env::var(SHARE_UNIT_ENV)
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_SHARE_UNIT.to_string());

        Some(Self { env_file, unit })
    }

    fn systemctl(&self, verb: &str) -> Result<bool, ShareControlError> {
        Command::new("systemctl")
            .arg(verb)
            .arg(&self.unit)
            .status()
            .map(|status| status.success())
            .map_err(|source| ShareControlError::Systemctl { source })
    }
}

impl ShareController for SystemdShareController {
    fn is_configured(&self) -> bool {
        self.env_file.exists()
    }

    fn is_mounted(&self) -> bool {
        self.systemctl("is-active").unwrap_or(false)
    }

    fn configure_and_mount(&self, request: &ShareConfigRequest) -> Result<(), ShareControlError> {
        write_share_env(&self.env_file, request)?;
        if self.systemctl("restart")? {
            Ok(())
        } else {
            Err(ShareControlError::UnitFailed)
        }
    }

    fn unmount(&self) -> Result<(), ShareControlError> {
        if self.systemctl("stop")? {
            Ok(())
        } else {
            Err(ShareControlError::UnitFailed)
        }
    }
}

/// Writes UNC + credentials for the mount script. Restrictive permissions:
/// the file carries the dedicated share account's secret.
pub fn write_share_env(
    env_file: &Path,
    request: &ShareConfigRequest,
) -> Result<(), ShareControlError> {
    if let Some(parent) = env_file.parent() {
        fs::create_dir_all(parent).map_err(|source| ShareControlError::Write {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    let contents = format!(
        "SECONDWIND_SHARE_UNC={}\nSECONDWIND_SHARE_USERNAME={}\nSECONDWIND_SHARE_PASSWORD={}\n",
        request.unc_path, request.username, request.password
    );
    fs::write(env_file, contents).map_err(|source| ShareControlError::Write {
        path: env_file.to_path_buf(),
        source,
    })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(env_file, fs::Permissions::from_mode(0o600));
    }

    Ok(())
}

/// The host re-sends configuration and remounts on every connect, so the
/// only states worth distinguishing are mounted / not.
pub fn share_state(controller: &dyn ShareController) -> ShareState {
    if controller.is_mounted() {
        ShareState::Mounted
    } else {
        ShareState::NotConfigured
    }
}

#[derive(Debug)]
pub enum ShareControlError {
    Write { path: PathBuf, source: io::Error },
    Systemctl { source: io::Error },
    UnitFailed,
}

impl std::fmt::Display for ShareControlError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Write { path, .. } => write!(
                formatter,
                "could not store the share settings at {}",
                path.display()
            ),
            Self::Systemctl { .. } => write!(formatter, "could not control the share service"),
            Self::UnitFailed => write!(formatter, "the share service refused the request"),
        }
    }
}

impl std::error::Error for ShareControlError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Write { source, .. } | Self::Systemctl { source } => Some(source),
            Self::UnitFailed => None,
        }
    }
}

#[cfg(test)]
pub mod test_support {
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::*;

    #[derive(Debug, Default)]
    pub struct FakeShareController {
        pub configured: AtomicBool,
        pub mounted: AtomicBool,
    }

    impl ShareController for FakeShareController {
        fn is_configured(&self) -> bool {
            self.configured.load(Ordering::SeqCst)
        }

        fn is_mounted(&self) -> bool {
            self.mounted.load(Ordering::SeqCst)
        }

        fn configure_and_mount(
            &self,
            _request: &ShareConfigRequest,
        ) -> Result<(), ShareControlError> {
            self.configured.store(true, Ordering::SeqCst);
            self.mounted.store(true, Ordering::SeqCst);
            Ok(())
        }

        fn unmount(&self) -> Result<(), ShareControlError> {
            self.mounted.store(false, Ordering::SeqCst);
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn share_env_round_trips_credentials() {
        let file = std::env::temp_dir().join(format!(
            "secondwind-share-env-{}.env",
            std::process::id()
        ));
        let _ = fs::remove_file(&file);

        write_share_env(
            &file,
            &ShareConfigRequest {
                unc_path: r"\\10.0.0.1\SecondWind".to_string(),
                username: "SecondWindShare".to_string(),
                password: "secret".to_string(),
            },
        )
        .expect("write share env");
        let contents = fs::read_to_string(&file).expect("read env");
        let _ = fs::remove_file(&file);

        assert!(contents.contains(r"SECONDWIND_SHARE_UNC=\\10.0.0.1\SecondWind"));
        assert!(contents.contains("SECONDWIND_SHARE_USERNAME=SecondWindShare"));
        assert!(contents.contains("SECONDWIND_SHARE_PASSWORD=secret"));
    }
}
