//! Node-side seamless apps (v0.3).
//!
//! The node image runs one xpra session (`sw-xpra.service`) bound to a
//! configured port with a first-boot-generated password. The agent reports
//! the session endpoint + which whitelisted apps are installed, and starts
//! apps inside that session via `xpra control`. Launch requests carry an
//! `app_id` resolved against the node's own catalog file — host-supplied
//! command lines are never executed.

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
};

use sw_core::apps::AppCatalogEntry;
use sw_core::{AppSessionEndpoint, NodeAppInfo};

pub const APPS_CATALOG_FILE_ENV: &str = "SECONDWIND_APPS_FILE";
pub const XPRA_PORT_ENV: &str = "SECONDWIND_XPRA_PORT";
pub const XPRA_PASSWORD_FILE_ENV: &str = "SECONDWIND_XPRA_PASSWORD_FILE";
pub const XPRA_DISPLAY_ENV: &str = "SECONDWIND_XPRA_DISPLAY";
const DEFAULT_XPRA_DISPLAY: &str = ":100";

pub trait AppsController: Send + Sync + std::fmt::Debug {
    fn catalog(&self) -> Vec<AppCatalogEntry>;
    fn session_endpoint(&self) -> Option<AppSessionEndpoint>;
    fn is_installed(&self, node_command: &str) -> bool;
    fn launch(&self, entry: &AppCatalogEntry) -> Result<(), AppsControlError>;
}

pub type SharedAppsController = Arc<dyn AppsController>;

/// Production controller reading image-provided config.
#[derive(Debug)]
pub struct XpraAppsController {
    catalog_file: PathBuf,
    port: u16,
    password_file: PathBuf,
    display: String,
}

impl XpraAppsController {
    pub fn from_env() -> Option<Self> {
        let catalog_file = std::env::var_os(APPS_CATALOG_FILE_ENV)
            .map(PathBuf::from)
            .filter(|path| !path.as_os_str().is_empty())?;
        let port = std::env::var(XPRA_PORT_ENV).ok()?.trim().parse().ok()?;
        let password_file = std::env::var_os(XPRA_PASSWORD_FILE_ENV)
            .map(PathBuf::from)
            .filter(|path| !path.as_os_str().is_empty())?;
        let display = std::env::var(XPRA_DISPLAY_ENV)
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_XPRA_DISPLAY.to_string());

        Some(Self {
            catalog_file,
            port,
            password_file,
            display,
        })
    }
}

impl AppsController for XpraAppsController {
    fn catalog(&self) -> Vec<AppCatalogEntry> {
        load_catalog(&self.catalog_file)
    }

    fn session_endpoint(&self) -> Option<AppSessionEndpoint> {
        let password = fs::read_to_string(&self.password_file).ok()?;
        let password = password.trim().to_string();
        if password.is_empty() {
            return None;
        }
        Some(AppSessionEndpoint {
            port: self.port,
            password,
        })
    }

    fn is_installed(&self, node_command: &str) -> bool {
        // `command -v` through sh: resolves exactly like the session will.
        Command::new("sh")
            .arg("-c")
            .arg(format!("command -v -- {}", shell_quote(node_command)))
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    fn launch(&self, entry: &AppCatalogEntry) -> Result<(), AppsControlError> {
        let command = wrapped_node_command(entry);
        let status = Command::new("xpra")
            .arg("control")
            .arg(&self.display)
            .arg("start")
            .arg(&command)
            .status()
            .map_err(|source| AppsControlError::Spawn { source })?;

        if status.success() {
            Ok(())
        } else {
            Err(AppsControlError::LaunchFailed {
                app_id: entry.app_id.clone(),
            })
        }
    }
}

/// Apps with a synced profile run through the cache-and-sync wrapper the
/// image installs; everything else runs directly.
pub fn wrapped_node_command(entry: &AppCatalogEntry) -> String {
    match &entry.synced_profile {
        Some(profile) => format!(
            "/usr/local/lib/secondwind/secondwind-run-synced.sh {} {} {}",
            shell_quote(&entry.app_id),
            shell_quote(profile),
            shell_quote(&entry.node_command)
        ),
        None => entry.node_command.clone(),
    }
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}

pub fn load_catalog(catalog_file: &Path) -> Vec<AppCatalogEntry> {
    fs::read_to_string(catalog_file)
        .ok()
        .and_then(|contents| serde_json::from_str(&contents).ok())
        .unwrap_or_else(sw_core::apps::default_catalog)
}

pub fn app_infos(controller: &dyn AppsController) -> Vec<NodeAppInfo> {
    controller
        .catalog()
        .iter()
        .map(|entry| NodeAppInfo {
            app_id: entry.app_id.clone(),
            installed: controller.is_installed(&entry.node_command),
        })
        .collect()
}

#[derive(Debug)]
pub enum AppsControlError {
    UnknownApp { app_id: String },
    NotInstalled { app_id: String },
    Spawn { source: std::io::Error },
    LaunchFailed { app_id: String },
}

impl std::fmt::Display for AppsControlError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownApp { app_id } => {
                write!(formatter, "{app_id} is not in this node's app library")
            }
            Self::NotInstalled { app_id } => {
                write!(formatter, "{app_id} is not installed on this node")
            }
            Self::Spawn { .. } => write!(formatter, "could not start the app session"),
            Self::LaunchFailed { app_id } => {
                write!(formatter, "{app_id} could not be started on the node")
            }
        }
    }
}

impl std::error::Error for AppsControlError {}

#[cfg(test)]
pub mod test_support {
    use std::sync::Mutex;

    use super::*;

    #[derive(Debug, Default)]
    pub struct FakeAppsController {
        pub endpoint: Option<AppSessionEndpoint>,
        pub installed: Vec<String>,
        pub launched: Mutex<Vec<String>>,
    }

    impl AppsController for FakeAppsController {
        fn catalog(&self) -> Vec<AppCatalogEntry> {
            sw_core::apps::default_catalog()
        }

        fn session_endpoint(&self) -> Option<AppSessionEndpoint> {
            self.endpoint.clone()
        }

        fn is_installed(&self, node_command: &str) -> bool {
            self.installed.iter().any(|entry| entry == node_command)
        }

        fn launch(&self, entry: &AppCatalogEntry) -> Result<(), AppsControlError> {
            self.launched
                .lock()
                .expect("launched lock")
                .push(entry.app_id.clone());
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synced_apps_run_through_the_sync_wrapper() {
        let catalog = sw_core::apps::default_catalog();
        let firefox = catalog
            .iter()
            .find(|entry| entry.app_id == "firefox")
            .expect("firefox");
        let vlc = catalog
            .iter()
            .find(|entry| entry.app_id == "vlc")
            .expect("vlc");

        let firefox_command = wrapped_node_command(firefox);
        assert!(firefox_command.contains("secondwind-run-synced.sh"));
        assert!(firefox_command.contains("'firefox'"));
        assert!(firefox_command.contains("'.mozilla'"));

        assert_eq!(wrapped_node_command(vlc), "vlc");
    }

    #[test]
    fn missing_catalog_file_falls_back_to_default() {
        let catalog = load_catalog(Path::new("does-not-exist.json"));

        assert!(!catalog.is_empty());
    }

    #[test]
    fn shell_quoting_neutralizes_quotes() {
        assert_eq!(shell_quote("simple"), "'simple'");
        assert_eq!(shell_quote("a'b"), r"'a'\''b'");
    }
}
