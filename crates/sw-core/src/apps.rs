//! App catalog and per-app policy types (v0.3).
//!
//! Policy is set once per app in the companion library: run on the node,
//! run locally, or ask each time — plus whether falling back to a local
//! copy is allowed when the node is unreachable. The node agent keeps its
//! own catalog whitelist; a launch request only ever names an `app_id`,
//! never a command line.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppPolicy {
    AlwaysOnNode,
    AlwaysLocal,
    AskEachTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppCatalogEntry {
    /// Stable identifier shared between host library and node whitelist.
    pub app_id: String,
    pub display_name: String,
    /// Program the node runs (resolved on the node's PATH).
    pub node_command: String,
    /// Optional local Windows counterpart (resolved/overridden per host).
    pub local_command: Option<String>,
    pub policy: AppPolicy,
    pub fallback_to_local: bool,
    /// For apps holding live databases (browser profiles): the profile
    /// directory (relative to the app's home) that the node syncs to tmpfs
    /// at session start and back on close.
    pub synced_profile: Option<String>,
}

/// The v1 catalog. Commands are upstream program names with no paths;
/// policies default to asking the user.
pub fn default_catalog() -> Vec<AppCatalogEntry> {
    let entry = |app_id: &str,
                 display_name: &str,
                 node_command: &str,
                 local_command: Option<&str>,
                 synced_profile: Option<&str>| AppCatalogEntry {
        app_id: app_id.to_string(),
        display_name: display_name.to_string(),
        node_command: node_command.to_string(),
        local_command: local_command.map(|command| command.to_string()),
        policy: AppPolicy::AskEachTime,
        fallback_to_local: true,
        synced_profile: synced_profile.map(|profile| profile.to_string()),
    };

    vec![
        entry(
            "firefox",
            "Firefox",
            "firefox",
            Some("firefox.exe"),
            Some(".mozilla"),
        ),
        entry(
            "chromium",
            "Chromium",
            "chromium",
            Some("chrome.exe"),
            Some(".config/chromium"),
        ),
        entry("vlc", "VLC", "vlc", Some("vlc.exe"), None),
        entry(
            "libreoffice",
            "LibreOffice",
            "libreoffice",
            Some("soffice.exe"),
            None,
        ),
        entry("gimp", "GIMP", "gimp", Some("gimp.exe"), None),
        entry("pdf-reader", "PDF Reader", "evince", None, None),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_catalog_matches_the_plan_v1_list() {
        let catalog = default_catalog();
        let ids: Vec<&str> = catalog.iter().map(|entry| entry.app_id.as_str()).collect();

        for expected in [
            "firefox",
            "chromium",
            "vlc",
            "libreoffice",
            "gimp",
            "pdf-reader",
        ] {
            assert!(ids.contains(&expected), "missing {expected}");
        }
    }

    #[test]
    fn browsers_use_cache_and_sync() {
        let catalog = default_catalog();

        for browser in ["firefox", "chromium"] {
            let entry = catalog
                .iter()
                .find(|entry| entry.app_id == browser)
                .expect("browser in catalog");
            assert!(entry.synced_profile.is_some(), "{browser} must sync profile");
        }
    }

    #[test]
    fn catalog_round_trips_as_json() {
        let catalog = default_catalog();
        let json = serde_json::to_string(&catalog).expect("serialize");
        let decoded: Vec<AppCatalogEntry> = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(decoded, catalog);
    }
}
