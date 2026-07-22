//! Host-side Apps orchestration (v0.3).
//!
//! Launching an app resolves its policy through `sw-launcher`'s decision
//! engine, then executes the decision: run the local copy, run it on the
//! node through the seamless session (waking the node first when needed),
//! or bubble "ask" up to the UI. The host share is (re)configured on the
//! node before node-side launches so apps see the user's files.

use std::{
    fs,
    net::{IpAddr, SocketAddr, UdpSocket},
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use sw_core::{
    AppLaunchRequest, AppSessionState, AppsStatusResponse, ShareConfigRequest,
    apps::AppCatalogEntry,
};
use sw_launcher::{
    LaunchDecision, NodeAvailability, attach_spec, decide, decide_after_ask, send_wake,
};

use crate::{
    host_state::HostState,
    node_client::{self, NodeEndpoint},
};

pub const XPRA_CLIENT_ENV: &str = "SECONDWIND_XPRA_CLIENT";
const DEFAULT_XPRA_CLIENT: &str = "xpra";
const WAKE_TIMEOUT: Duration = Duration::from_secs(90);
const WAKE_POLL: Duration = Duration::from_secs(3);

pub const SHARE_CREDENTIALS_FILE: &str = "share-credentials.json";
pub const SHARE_NAME: &str = "SecondWind";
pub const SHARE_ACCOUNT: &str = "SecondWindShare";

/// Where a launch ended up (returned to the UI).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum LaunchOutcome {
    OnNode,
    Local,
    /// Policy says ask; the UI shows the choice and calls back with it.
    NeedsChoice,
    Failed {
        message: String,
    },
}

/// Host-side share credentials (dedicated account, random password).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareCredentials {
    pub folder: String,
    pub share_name: String,
    pub username: String,
    pub password: String,
}

/// Loads or creates the host share: generates credentials once, then runs
/// the elevated setup script (UAC prompt on first use).
pub fn ensure_host_share(state_root: &Path) -> Result<ShareCredentials, String> {
    let credentials_file = state_root.join(SHARE_CREDENTIALS_FILE);
    if let Ok(contents) = fs::read_to_string(&credentials_file)
        && let Ok(credentials) = serde_json::from_str::<ShareCredentials>(&contents)
    {
        return Ok(credentials);
    }

    let folder = default_share_folder()?;
    let credentials = ShareCredentials {
        folder: folder.to_string_lossy().to_string(),
        share_name: SHARE_NAME.to_string(),
        username: SHARE_ACCOUNT.to_string(),
        password: random_password()?,
    };

    run_share_setup_elevated(state_root, &credentials)?;

    fs::create_dir_all(state_root).map_err(|error| error.to_string())?;
    fs::write(
        &credentials_file,
        serde_json::to_string_pretty(&credentials).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;

    Ok(credentials)
}

fn default_share_folder() -> Result<PathBuf, String> {
    std::env::var_os("USERPROFILE")
        .map(|profile| PathBuf::from(profile).join("SecondWind"))
        .ok_or_else(|| "SecondWind could not find your user folder.".to_string())
}

fn random_password() -> Result<String, String> {
    let mut bytes = [0_u8; 24];
    getrandom::getrandom(&mut bytes)
        .map_err(|_| "SecondWind could not generate a secure password.".to_string())?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// First-time share setup needs elevation; `-Verb RunAs` triggers the UAC
/// prompt. Elevated children do NOT inherit the parent's environment, and
/// argv is visible in process listings — so the password travels through a
/// short-lived file in the user's own state dir (readable by the elevated
/// same-user process, deleted by the script after use).
fn run_share_setup_elevated(
    state_root: &Path,
    credentials: &ShareCredentials,
) -> Result<(), String> {
    let scripts = crate::disk_control::scripts_dir()
        .ok_or("SecondWind's setup tools are missing from this installation.")?;
    let script = scripts.join("Enable-SecondWindShare.ps1");

    fs::create_dir_all(state_root).map_err(|error| error.to_string())?;
    let password_file = state_root.join("share-setup.pass");
    sw_core::certificates::write_atomic(&password_file, credentials.password.as_bytes(), true)
        .map_err(|_| "SecondWind could not prepare the share setup.".to_string())?;

    let quote = |value: &str| value.replace('\'', "''");
    let inner = format!(
        "& '{}' -FolderPath '{}' -AccountPasswordFile '{}' -ShareName '{}' -AccountName '{}'",
        quote(&script.to_string_lossy()),
        quote(&credentials.folder),
        quote(&password_file.to_string_lossy()),
        credentials.share_name,
        credentials.username,
    );
    let status = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &format!(
                "$p = Start-Process powershell.exe -Verb RunAs -Wait -PassThru -ArgumentList \
                 '-NoProfile','-ExecutionPolicy','Bypass','-Command',@'\n{inner}\n'@; exit $p.ExitCode"
            ),
        ])
        .status()
        .map_err(|_| "SecondWind could not start the share setup.".to_string())?;

    // Belt and braces: the script deletes it, but never leave it behind.
    let _ = fs::remove_file(&password_file);

    if status.success() {
        Ok(())
    } else {
        Err("Setting up the SecondWind folder was cancelled or failed.".to_string())
    }
}

/// The host address as the node sees it: the local address of a socket
/// "connected" toward the node. No interface names, no configuration.
pub fn local_address_toward(node_address: &IpAddr) -> Option<IpAddr> {
    let socket = UdpSocket::bind(match node_address {
        IpAddr::V4(_) => SocketAddr::from(([0, 0, 0, 0], 0)),
        IpAddr::V6(_) => SocketAddr::from(([0_u16; 8], 0)),
    })
    .ok()?;
    socket.connect((*node_address, 9)).ok()?;
    Some(socket.local_addr().ok()?.ip())
}

/// Ensures the node has the host share mounted (best-effort, but errors
/// surface so the user learns why files are missing).
pub fn configure_node_share(
    state: &HostState,
    state_root: &Path,
    endpoint: &NodeEndpoint,
) -> Result<(), String> {
    let credentials = ensure_host_share(state_root)?;
    let node_address = endpoint
        .addresses
        .first()
        .ok_or("The node did not advertise a reachable address.")?;
    let host_address = local_address_toward(node_address)
        .ok_or("SecondWind could not work out its own network address.")?;

    let request = ShareConfigRequest {
        unc_path: format!(r"\\{host_address}\{}", credentials.share_name),
        username: credentials.username.clone(),
        password: credentials.password.clone(),
    };
    node_client::post_share_config(endpoint, Some(&state.certificate), &request)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

pub fn availability(
    state: &HostState,
    node_uuid: &sw_core::NodeUuid,
    discovered: bool,
) -> NodeAvailability {
    if discovered {
        return NodeAvailability::Reachable;
    }
    let wakeable = state
        .paired_node(node_uuid)
        .map(|node| !node.wake.mac_addresses.is_empty())
        .unwrap_or(false);
    if wakeable {
        NodeAvailability::OffButWakeable
    } else {
        NodeAvailability::Unreachable
    }
}

pub fn resolve_decision(
    entry: &AppCatalogEntry,
    availability: NodeAvailability,
    user_chose_node: Option<bool>,
) -> LaunchDecision {
    match user_chose_node {
        Some(choice) => decide_after_ask(entry, availability, choice),
        None => decide(entry, availability),
    }
}

/// Executes a decision. `rediscover` re-browses after a wake so the node's
/// fresh addresses are used.
pub fn execute_decision(
    state: &HostState,
    state_root: &Path,
    entry: &AppCatalogEntry,
    decision: LaunchDecision,
    endpoint: Option<NodeEndpoint>,
    node_uuid: &sw_core::NodeUuid,
) -> LaunchOutcome {
    match decision {
        LaunchDecision::AskUser => LaunchOutcome::NeedsChoice,
        LaunchDecision::Local { command } => match Command::new(&command).spawn() {
            Ok(_) => LaunchOutcome::Local,
            Err(_) => LaunchOutcome::Failed {
                message: format!(
                    "{} could not be started on this PC ({command} not found).",
                    entry.display_name
                ),
            },
        },
        LaunchDecision::Unavailable { reason } => LaunchOutcome::Failed { message: reason },
        LaunchDecision::OnNode => match endpoint {
            Some(endpoint) => launch_on_node(state, state_root, entry, &endpoint),
            None => LaunchOutcome::Failed {
                message: "The node is not reachable right now.".to_string(),
            },
        },
        LaunchDecision::WakeThenOnNode => {
            let macs = state
                .paired_node(node_uuid)
                .map(|node| node.wake.mac_addresses.clone())
                .unwrap_or_default();
            if send_wake(&macs) == 0 {
                return LaunchOutcome::Failed {
                    message: "SecondWind could not send the wake signal.".to_string(),
                };
            }
            match wait_for_node(node_uuid) {
                Some(endpoint_from_discovery) => {
                    // Trust still comes from stored config.
                    let endpoint = NodeEndpoint {
                        certificate_fingerprint: state
                            .paired_node(node_uuid)
                            .map(|node| node.trust.peer_certificate_fingerprint.clone())
                            .unwrap_or_default(),
                        ..endpoint_from_discovery
                    };
                    launch_on_node(state, state_root, entry, &endpoint)
                }
                None => LaunchOutcome::Failed {
                    message: "Your node is starting but didn't come online in time.".to_string(),
                },
            }
        }
    }
}

fn wait_for_node(node_uuid: &sw_core::NodeUuid) -> Option<NodeEndpoint> {
    let deadline = Instant::now() + WAKE_TIMEOUT;
    while Instant::now() < deadline {
        if let Ok(nodes) = crate::discovery::discover_secondwind_nodes(Duration::from_millis(1200))
            && let Some(node) = nodes.into_iter().find(|node| node.node_uuid == *node_uuid)
        {
            return Some(NodeEndpoint {
                addresses: node.addresses,
                api_port: node.api_port,
                certificate_fingerprint: node.node_certificate_fingerprint,
            });
        }
        std::thread::sleep(WAKE_POLL);
    }
    None
}

fn launch_on_node(
    state: &HostState,
    state_root: &Path,
    entry: &AppCatalogEntry,
    endpoint: &NodeEndpoint,
) -> LaunchOutcome {
    // 1. Files first: the app must see the user's data.
    if let Err(message) = configure_node_share(state, state_root, endpoint) {
        return LaunchOutcome::Failed { message };
    }

    // 2. Session endpoint over mTLS.
    let status: AppsStatusResponse =
        match node_client::get_apps_status(endpoint, Some(&state.certificate)) {
            Ok(status) => status,
            Err(error) => {
                return LaunchOutcome::Failed {
                    message: error.to_string(),
                };
            }
        };
    let session = match status.session {
        AppSessionState::Ready { endpoint } => endpoint,
        AppSessionState::Unavailable { reason } => {
            return LaunchOutcome::Failed { message: reason };
        }
        AppSessionState::NotPaired => {
            return LaunchOutcome::Failed {
                message: "Pair with this node before launching apps.".to_string(),
            };
        }
    };

    // 3. Attach the seamless client (idempotent: xpra ignores a second
    // attach to the same session).
    let Some(node_address) = endpoint.addresses.first() else {
        return LaunchOutcome::Failed {
            message: "The node did not advertise a reachable address.".to_string(),
        };
    };
    let password_file = state_root.join("app-session.pass");
    if sw_core::certificates::write_atomic(&password_file, session.password.as_bytes(), true)
        .is_err()
    {
        return LaunchOutcome::Failed {
            message: "SecondWind could not prepare the app session.".to_string(),
        };
    }
    let client = std::env::var(XPRA_CLIENT_ENV).unwrap_or_else(|_| DEFAULT_XPRA_CLIENT.to_string());
    let spec = attach_spec(
        &client,
        node_address,
        session.port,
        &password_file.to_string_lossy(),
    );
    if Command::new(&spec.program)
        .args(&spec.args)
        .spawn()
        .is_err()
    {
        return LaunchOutcome::Failed {
            message: "SecondWind's window client is missing from this installation.".to_string(),
        };
    }

    // 4. Start the app inside the session.
    match node_client::post_app_launch(
        endpoint,
        Some(&state.certificate),
        &AppLaunchRequest {
            app_id: entry.app_id.clone(),
        },
    ) {
        Ok(response) if response.launched => LaunchOutcome::OnNode,
        Ok(response) => LaunchOutcome::Failed {
            message: response
                .message
                .unwrap_or_else(|| "The node could not start the app.".to_string()),
        },
        Err(error) => LaunchOutcome::Failed {
            message: error.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sw_core::apps::AppPolicy;

    fn entry(policy: AppPolicy) -> AppCatalogEntry {
        AppCatalogEntry {
            app_id: "app".to_string(),
            display_name: "App".to_string(),
            node_command: "app".to_string(),
            local_command: Some("app.exe".to_string()),
            policy,
            fallback_to_local: true,
            synced_profile: None,
        }
    }

    #[test]
    fn ask_policy_surfaces_choice_and_resolves_with_it() {
        let ask = resolve_decision(
            &entry(AppPolicy::AskEachTime),
            NodeAvailability::Reachable,
            None,
        );
        assert_eq!(ask, LaunchDecision::AskUser);

        let chosen = resolve_decision(
            &entry(AppPolicy::AskEachTime),
            NodeAvailability::Reachable,
            Some(true),
        );
        assert_eq!(chosen, LaunchDecision::OnNode);
    }

    #[test]
    fn local_address_toward_loopback_is_loopback() {
        let address =
            local_address_toward(&"127.0.0.1".parse().expect("ip")).expect("local address");

        assert!(address.is_loopback());
    }

    #[test]
    fn unc_path_shape_is_valid_for_ipv4() {
        let host: IpAddr = "192.0.2.7".parse().expect("ip");
        let unc = format!(r"\\{host}\{SHARE_NAME}");

        assert_eq!(unc, r"\\192.0.2.7\SecondWind");
    }
}
