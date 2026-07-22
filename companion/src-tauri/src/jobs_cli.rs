//! Headless job submission (v0.5) — the Explorer context-menu path.
//!
//! `secondwind-companion.exe --job <preset-id> <input>` submits a job to
//! the first reachable paired node. File inputs must live inside the
//! SecondWind shared folder (that's the whole point: the node operates on
//! the shared path directly, no copying); URLs go to download presets.

use std::{path::PathBuf, time::Duration};

use sw_core::{JobInput, JobSubmitRequest};

use crate::{
    app_control,
    host_state::HostState,
    node_client::{self, NodeEndpoint},
};

const DISCOVERY_WINDOW: Duration = Duration::from_millis(1500);

/// Returns a process exit code; all outcomes are printed for support logs.
pub fn run(args: &[String]) -> i32 {
    match run_inner(args) {
        Ok(message) => {
            println!("{message}");
            0
        }
        Err(message) => {
            eprintln!("SecondWind: {message}");
            1
        }
    }
}

fn state_dir() -> Result<PathBuf, String> {
    if let Ok(dir) = std::env::var("SECONDWIND_COMPANION_STATE_DIR")
        && !dir.trim().is_empty()
    {
        return Ok(PathBuf::from(dir));
    }
    // Same location Tauri's app_data_dir resolves to for this app id.
    std::env::var_os("APPDATA")
        .map(|appdata| PathBuf::from(appdata).join("app.secondwind.companion"))
        .ok_or_else(|| "could not find the SecondWind settings folder".to_string())
}

fn run_inner(args: &[String]) -> Result<String, String> {
    let [preset_id, raw_input, ..] = args else {
        return Err("usage: --job <preset-id> <file-or-url>".to_string());
    };

    let state_root = state_dir()?;
    let state = HostState::load_or_create(&state_root).map_err(|error| error.to_string())?;

    let input = if raw_input.starts_with("http://") || raw_input.starts_with("https://") {
        JobInput::Url {
            url: raw_input.clone(),
        }
    } else {
        JobInput::SharePath {
            path: share_relative_path(&state_root, raw_input)?,
        }
    };

    // Find a reachable paired node.
    let nodes = crate::discovery::discover_secondwind_nodes(DISCOVERY_WINDOW)
        .map_err(|error| error.to_string())?;
    let Some(node) = nodes
        .into_iter()
        .find(|node| state.paired_node(&node.node_uuid).is_some())
    else {
        return Err("no paired node is reachable right now".to_string());
    };
    let trusted_fingerprint = state
        .paired_node(&node.node_uuid)
        .map(|paired| paired.trust.peer_certificate_fingerprint.clone())
        .expect("paired node just matched");
    let endpoint = NodeEndpoint {
        addresses: node.addresses,
        api_port: node.api_port,
        certificate_fingerprint: trusted_fingerprint,
    };

    // Same preset + same input = same job: a double-clicked context menu
    // entry or a retried request never starts a duplicate container.
    let idempotency_key = Some(format!("{preset_id}:{raw_input}"));
    let response = node_client::post_job_submit(
        &endpoint,
        Some(&state.certificate),
        &JobSubmitRequest {
            preset_id: preset_id.clone(),
            input,
            idempotency_key,
        },
    )
    .map_err(|error| error.to_string())?;

    if response.accepted {
        Ok(format!(
            "job started on {} (id {})",
            node.node_name,
            response.job_id.unwrap_or_default()
        ))
    } else {
        Err(response
            .message
            .unwrap_or_else(|| "the node declined the job".to_string()))
    }
}

/// Maps an absolute Windows path to a share-relative path, requiring it to
/// live inside the SecondWind shared folder.
fn share_relative_path(state_root: &std::path::Path, raw_input: &str) -> Result<String, String> {
    let credentials_file = state_root.join(app_control::SHARE_CREDENTIALS_FILE);
    let contents = std::fs::read_to_string(&credentials_file).map_err(|_| {
        "the SecondWind folder is not set up yet — open SecondWind and launch an app on the \
         node once"
            .to_string()
    })?;
    let credentials: app_control::ShareCredentials =
        serde_json::from_str(&contents).map_err(|error| error.to_string())?;

    relative_inside(&credentials.folder, raw_input).ok_or_else(|| {
        format!(
            "only files inside your SecondWind folder ({}) can be processed on the node",
            credentials.folder
        )
    })
}

/// Pure path containment: `input` must be strictly inside `root`.
///
/// Component-based on purpose: no byte arithmetic (Unicode case folding
/// changes string length) and no trailing-separator pitfalls. Comparison
/// is ASCII-case-insensitive like Windows paths; the returned relative
/// path preserves the input's original casing.
pub fn relative_inside(root: &str, input: &str) -> Option<String> {
    let components = |value: &str| -> Vec<String> {
        value
            .replace('/', "\\")
            .split('\\')
            .filter(|part| !part.is_empty())
            .map(|part| part.to_string())
            .collect()
    };

    let root_parts = components(root);
    let input_parts = components(input);
    if root_parts.is_empty() || input_parts.len() <= root_parts.len() {
        return None;
    }

    let prefix_matches = root_parts
        .iter()
        .zip(&input_parts)
        .all(|(a, b)| a.eq_ignore_ascii_case(b));
    if !prefix_matches {
        return None;
    }

    Some(input_parts[root_parts.len()..].join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_inside_accepts_children_only() {
        assert_eq!(
            relative_inside(
                r"C:\Users\Me\SecondWind",
                r"C:\Users\Me\SecondWind\Videos\a.mkv"
            ),
            Some("Videos/a.mkv".to_string())
        );
        assert_eq!(
            relative_inside(
                r"C:\Users\Me\SecondWind\",
                r"c:\users\me\secondwind\file.txt"
            ),
            Some("file.txt".to_string())
        );
        assert_eq!(
            relative_inside(r"C:\Users\Me\SecondWind", r"C:\Users\Me\Other\file.txt"),
            None
        );
        assert_eq!(
            relative_inside(r"C:\Users\Me\SecondWind", r"C:\Users\Me\SecondWind"),
            None
        );
    }

    #[test]
    fn relative_inside_preserves_input_casing() {
        assert_eq!(
            relative_inside(
                r"c:\users\me\secondwind",
                r"C:\Users\Me\SecondWind\Videos\A.MKV"
            ),
            Some("Videos/A.MKV".to_string())
        );
    }

    #[test]
    fn relative_inside_survives_unicode_and_trailing_separators() {
        // Non-ASCII case-changing chars must not panic (BUG-011: the old
        // byte-arithmetic version could slice mid-character).
        assert_eq!(
            relative_inside(
                r"C:\Users\İbrahim\SecondWind",
                r"C:\Users\İbrahim\SecondWind\straße.txt"
            ),
            Some("straße.txt".to_string())
        );
        // Trailing separators on the input must not corrupt the result.
        assert_eq!(
            relative_inside(
                r"C:\Users\Me\SecondWind",
                "C:\\Users\\Me\\SecondWind\\dir\\"
            ),
            Some("dir".to_string())
        );
        // A sibling folder that merely starts with the root's name.
        assert_eq!(
            relative_inside(
                r"C:\Users\Me\SecondWind",
                r"C:\Users\Me\SecondWindOther\f.txt"
            ),
            None
        );
    }
}
