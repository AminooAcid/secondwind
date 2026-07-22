//! Host-side Disk feature orchestration (v0.2).
//!
//! Connect: ask the node (mTLS) to export its designated data partition,
//! then drive the Windows iSCSI initiator through the bundled PowerShell
//! script (attach, first-use format of the SecondWind disk, drive letter).
//! Disconnect: flush + detach on Windows *first*, then ask the node to stop
//! exporting — reverse order so no write is ever lost.

use std::{net::IpAddr, path::PathBuf, process::Command};

use sw_core::{DiskAction, DiskCommandRequest, DiskState, DiskStatusResponse, DiskTarget};

use crate::{
    host_state::HostState,
    node_client::{self, NodeEndpoint},
};

pub const SCRIPTS_DIR_ENV: &str = "SECONDWIND_SCRIPTS_DIR";
const CONNECT_SCRIPT: &str = "Connect-SecondWindDisk.ps1";
const DISCONNECT_SCRIPT: &str = "Disconnect-SecondWindDisk.ps1";

/// Locates the bundled PowerShell scripts: env override (dev) → `scripts`
/// next to the executable (installed layout) → the repo layout (dev runs).
pub fn scripts_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os(SCRIPTS_DIR_ENV).filter(|dir| !dir.is_empty()) {
        return Some(PathBuf::from(dir));
    }

    let exe_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    let installed = exe_dir.join("scripts");
    if installed.is_dir() {
        return Some(installed);
    }

    // Development fallback: companion runs from target/, scripts live in
    // <repo>/scripts/windows.
    let mut dir = exe_dir;
    for _ in 0..6 {
        let candidate = dir.join("scripts").join("windows");
        if candidate.is_dir() {
            return Some(candidate);
        }
        dir = dir.parent()?.to_path_buf();
    }

    None
}

/// Builds the argument vector for the attach script. Pure for testing: no
/// machine values are invented here — everything comes from the node's
/// mTLS response and the user's config.
pub fn connect_script_args(
    script: &str,
    address: &IpAddr,
    target: &DiskTarget,
    drive_letter: Option<char>,
) -> Vec<String> {
    let mut args = vec![
        "-NoProfile".to_string(),
        "-NonInteractive".to_string(),
        "-ExecutionPolicy".to_string(),
        "Bypass".to_string(),
        "-File".to_string(),
        script.to_string(),
        "-TargetAddress".to_string(),
        address.to_string(),
        "-TargetIqn".to_string(),
        target.target_iqn.clone(),
        "-TargetPort".to_string(),
        target.port.to_string(),
        "-ChapUser".to_string(),
        target.chap_username.clone(),
        "-ChapSecret".to_string(),
        target.chap_secret.clone(),
    ];
    if let Some(letter) = drive_letter {
        args.push("-DriveLetter".to_string());
        args.push(letter.to_string());
    }
    args
}

pub fn disconnect_script_args(script: &str, target_iqn: &str) -> Vec<String> {
    vec![
        "-NoProfile".to_string(),
        "-NonInteractive".to_string(),
        "-ExecutionPolicy".to_string(),
        "Bypass".to_string(),
        "-File".to_string(),
        script.to_string(),
        "-TargetIqn".to_string(),
        target_iqn.to_string(),
    ]
}

fn run_powershell(args: &[String]) -> Result<String, DiskControlError> {
    let output = Command::new("powershell.exe")
        .args(args)
        .output()
        .map_err(|source| DiskControlError::Script { source })?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();

    if output.status.success() {
        Ok(stdout)
    } else {
        Err(DiskControlError::ScriptFailed {
            detail: parse_script_message(&stdout),
        })
    }
}

/// The scripts print a single JSON object; surface its message field.
fn parse_script_message(stdout: &str) -> Option<String> {
    let line = stdout.lines().rev().find(|line| line.trim().starts_with('{'))?;
    let value: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
    value
        .get("message")
        .and_then(|message| message.as_str())
        .map(|message| message.to_string())
}

pub fn parse_attached_drive_letter(stdout: &str) -> Option<char> {
    let line = stdout.lines().rev().find(|line| line.trim().starts_with('{'))?;
    let value: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
    value
        .get("drive_letter")
        .and_then(|letter| letter.as_str())
        .and_then(|letter| letter.chars().next())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiskConnectOutcome {
    pub drive_letter: Option<char>,
    /// Remembered by callers so a vanished node's session can still be
    /// cleaned up locally.
    pub target_iqn: String,
}

/// Brings the node disk up end-to-end. Returns the assigned drive letter.
pub fn connect_disk(
    state: &HostState,
    node_uuid: &sw_core::NodeUuid,
    endpoint: &NodeEndpoint,
) -> Result<DiskConnectOutcome, DiskControlError> {
    let node = state
        .paired_node(node_uuid)
        .ok_or(DiskControlError::NotPaired)?;

    // 1. Node exports its designated partition (mTLS).
    let response: DiskStatusResponse = node_client::post_disk_command(
        endpoint,
        Some(&state.certificate),
        &DiskCommandRequest {
            action: DiskAction::Enable,
        },
    )
    .map_err(DiskControlError::Node)?;

    let target = match response.disk {
        DiskState::Exposed { target } => target,
        DiskState::Unavailable { reason } => {
            return Err(DiskControlError::Unavailable { reason });
        }
        other => {
            return Err(DiskControlError::UnexpectedState {
                detail: format!("{other:?}"),
                message: response.message,
            });
        }
    };

    // 2. Attach on Windows.
    let scripts = scripts_dir().ok_or(DiskControlError::ScriptsMissing)?;
    let script = scripts.join(CONNECT_SCRIPT);
    let address = endpoint
        .addresses
        .first()
        .ok_or(DiskControlError::NoAddresses)?;
    let args = connect_script_args(
        &script.to_string_lossy(),
        address,
        &target,
        node.disk.drive_letter,
    );
    let stdout = run_powershell(&args)?;

    Ok(DiskConnectOutcome {
        drive_letter: parse_attached_drive_letter(&stdout),
        target_iqn: target.target_iqn,
    })
}

/// Windows-side flush + detach only — used when the node is already gone
/// (link loss) and cannot be asked to unexport.
pub fn detach_local(target_iqn: &str) -> Result<(), DiskControlError> {
    let scripts = scripts_dir().ok_or(DiskControlError::ScriptsMissing)?;
    let script = scripts.join(DISCONNECT_SCRIPT);
    run_powershell(&disconnect_script_args(
        &script.to_string_lossy(),
        target_iqn,
    ))
    .map(|_| ())
}

/// Tears the disk down: Windows flush + detach first, then the node stops
/// exporting. When the node is already unreachable (link lost), the local
/// detach still runs so the initiator session is cleaned up.
pub fn disconnect_disk(
    state: &HostState,
    endpoint: &NodeEndpoint,
    target_iqn_hint: Option<&str>,
) -> Result<(), DiskControlError> {
    // Learn the IQN from the node when reachable, else use the hint.
    let target_iqn = match node_client::get_disk_status(endpoint, Some(&state.certificate)) {
        Ok(DiskStatusResponse {
            disk: DiskState::Exposed { target },
            ..
        }) => Some(target.target_iqn),
        _ => target_iqn_hint.map(|iqn| iqn.to_string()),
    };

    if let Some(target_iqn) = &target_iqn {
        let scripts = scripts_dir().ok_or(DiskControlError::ScriptsMissing)?;
        let script = scripts.join(DISCONNECT_SCRIPT);
        run_powershell(&disconnect_script_args(
            &script.to_string_lossy(),
            target_iqn,
        ))?;
    }

    // Best effort: node may be gone already.
    let _ = node_client::post_disk_command(
        endpoint,
        Some(&state.certificate),
        &DiskCommandRequest {
            action: DiskAction::Disable,
        },
    );

    Ok(())
}

#[derive(Debug)]
pub enum DiskControlError {
    NotPaired,
    Node(node_client::NodeClientError),
    Unavailable { reason: String },
    UnexpectedState { detail: String, message: Option<String> },
    ScriptsMissing,
    Script { source: std::io::Error },
    ScriptFailed { detail: Option<String> },
    NoAddresses,
}

impl std::fmt::Display for DiskControlError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotPaired => write!(formatter, "Pair with this node before using the disk."),
            Self::Node(source) => write!(formatter, "{source}"),
            Self::Unavailable { reason } => write!(formatter, "{reason}"),
            Self::UnexpectedState { message, .. } => match message {
                Some(message) => write!(formatter, "{message}"),
                None => write!(formatter, "The node could not share its disk."),
            },
            Self::ScriptsMissing => write!(
                formatter,
                "SecondWind's disk tools are missing from this installation."
            ),
            Self::Script { .. } => write!(formatter, "Could not run the disk attach step."),
            Self::ScriptFailed { detail } => match detail {
                Some(detail) => write!(formatter, "{detail}"),
                None => write!(formatter, "Attaching the disk failed."),
            },
            Self::NoAddresses => {
                write!(formatter, "The node did not advertise a reachable address.")
            }
        }
    }
}

impl std::error::Error for DiskControlError {}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use super::*;

    fn target() -> DiskTarget {
        DiskTarget {
            target_iqn: "iqn.2026-07.app.secondwind:node-ab".to_string(),
            port: 3260,
            chap_username: "user".to_string(),
            chap_secret: "secret".to_string(),
        }
    }

    #[test]
    fn connect_args_carry_target_and_optional_letter() {
        let args = connect_script_args(
            "C:/scripts/Connect-SecondWindDisk.ps1",
            &IpAddr::V4(Ipv4Addr::LOCALHOST),
            &target(),
            Some('S'),
        );

        let joined = args.join(" ");
        assert!(joined.contains("-TargetAddress 127.0.0.1"));
        assert!(joined.contains("-TargetIqn iqn.2026-07.app.secondwind:node-ab"));
        assert!(joined.contains("-TargetPort 3260"));
        assert!(joined.contains("-ChapUser user"));
        assert!(joined.contains("-DriveLetter S"));
        assert!(joined.contains("-NonInteractive"));
    }

    #[test]
    fn connect_args_without_letter_let_windows_pick() {
        let args = connect_script_args(
            "script.ps1",
            &IpAddr::V4(Ipv4Addr::LOCALHOST),
            &target(),
            None,
        );

        assert!(!args.join(" ").contains("-DriveLetter"));
    }

    #[test]
    fn script_json_output_parses() {
        let stdout = "noise\n{\"status\":\"ok\",\"message\":\"done\",\"drive_letter\":\"S\"}\n";

        assert_eq!(parse_attached_drive_letter(stdout), Some('S'));
        assert_eq!(parse_script_message(stdout).as_deref(), Some("done"));
    }

    #[test]
    fn disconnect_args_reference_only_the_iqn() {
        let args = disconnect_script_args("script.ps1", "iqn.x");

        assert!(args.join(" ").contains("-TargetIqn iqn.x"));
        assert!(!args.join(" ").to_lowercase().contains("chap"));
    }
}
