//! Pure launch-decision engine.

use sw_core::apps::{AppCatalogEntry, AppPolicy};

/// What the launcher observes about the node right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeAvailability {
    /// Discovered and answering.
    Reachable,
    /// Paired but absent; wake targets are known.
    OffButWakeable,
    /// Paired but absent with no way to wake it.
    Unreachable,
}

/// Where a launch should happen. `AskUser` bubbles up to the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchDecision {
    OnNode,
    WakeThenOnNode,
    Local { command: String },
    AskUser,
    Unavailable { reason: String },
}

pub fn decide(entry: &AppCatalogEntry, availability: NodeAvailability) -> LaunchDecision {
    match entry.policy {
        AppPolicy::AlwaysLocal => local_or_unavailable(entry),
        AppPolicy::AskEachTime => LaunchDecision::AskUser,
        AppPolicy::AlwaysOnNode => match availability {
            NodeAvailability::Reachable => LaunchDecision::OnNode,
            NodeAvailability::OffButWakeable => LaunchDecision::WakeThenOnNode,
            NodeAvailability::Unreachable => {
                if entry.fallback_to_local {
                    local_or_unavailable(entry)
                } else {
                    LaunchDecision::Unavailable {
                        reason: format!(
                            "{} runs on your node, and the node can't be reached right now.",
                            entry.display_name
                        ),
                    }
                }
            }
        },
    }
}

/// Resolves an explicit user choice from the "ask each time" dialog.
pub fn decide_after_ask(
    entry: &AppCatalogEntry,
    availability: NodeAvailability,
    user_chose_node: bool,
) -> LaunchDecision {
    if !user_chose_node {
        return local_or_unavailable(entry);
    }

    match availability {
        NodeAvailability::Reachable => LaunchDecision::OnNode,
        NodeAvailability::OffButWakeable => LaunchDecision::WakeThenOnNode,
        NodeAvailability::Unreachable => LaunchDecision::Unavailable {
            reason: format!(
                "{} can't run on the node right now — the node is unreachable.",
                entry.display_name
            ),
        },
    }
}

fn local_or_unavailable(entry: &AppCatalogEntry) -> LaunchDecision {
    match &entry.local_command {
        Some(command) => LaunchDecision::Local {
            command: command.clone(),
        },
        None => LaunchDecision::Unavailable {
            reason: format!(
                "{} has no local copy on this PC.",
                entry.display_name
            ),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(policy: AppPolicy, fallback: bool, local: Option<&str>) -> AppCatalogEntry {
        AppCatalogEntry {
            app_id: "app".to_string(),
            display_name: "App".to_string(),
            node_command: "app".to_string(),
            local_command: local.map(|command| command.to_string()),
            policy,
            fallback_to_local: fallback,
            synced_profile: None,
        }
    }

    #[test]
    fn always_on_node_uses_node_when_reachable() {
        let decision = decide(
            &entry(AppPolicy::AlwaysOnNode, true, Some("app.exe")),
            NodeAvailability::Reachable,
        );

        assert_eq!(decision, LaunchDecision::OnNode);
    }

    #[test]
    fn powered_off_node_is_woken_first() {
        let decision = decide(
            &entry(AppPolicy::AlwaysOnNode, true, Some("app.exe")),
            NodeAvailability::OffButWakeable,
        );

        assert_eq!(decision, LaunchDecision::WakeThenOnNode);
    }

    #[test]
    fn unreachable_node_falls_back_when_allowed() {
        let allowed = decide(
            &entry(AppPolicy::AlwaysOnNode, true, Some("app.exe")),
            NodeAvailability::Unreachable,
        );
        let denied = decide(
            &entry(AppPolicy::AlwaysOnNode, false, Some("app.exe")),
            NodeAvailability::Unreachable,
        );

        assert_eq!(
            allowed,
            LaunchDecision::Local {
                command: "app.exe".to_string()
            }
        );
        assert!(matches!(denied, LaunchDecision::Unavailable { .. }));
    }

    #[test]
    fn fallback_without_local_copy_is_a_clear_notice() {
        let decision = decide(
            &entry(AppPolicy::AlwaysOnNode, true, None),
            NodeAvailability::Unreachable,
        );

        assert!(matches!(decision, LaunchDecision::Unavailable { .. }));
    }

    #[test]
    fn ask_policy_defers_to_the_user_then_resolves() {
        let entry = entry(AppPolicy::AskEachTime, true, Some("app.exe"));

        assert_eq!(
            decide(&entry, NodeAvailability::Reachable),
            LaunchDecision::AskUser
        );
        assert_eq!(
            decide_after_ask(&entry, NodeAvailability::Reachable, true),
            LaunchDecision::OnNode
        );
        assert_eq!(
            decide_after_ask(&entry, NodeAvailability::OffButWakeable, true),
            LaunchDecision::WakeThenOnNode
        );
        assert_eq!(
            decide_after_ask(&entry, NodeAvailability::Reachable, false),
            LaunchDecision::Local {
                command: "app.exe".to_string()
            }
        );
    }

    #[test]
    fn always_local_never_touches_the_node() {
        let decision = decide(
            &entry(AppPolicy::AlwaysLocal, false, Some("app.exe")),
            NodeAvailability::Reachable,
        );

        assert_eq!(
            decision,
            LaunchDecision::Local {
                command: "app.exe".to_string()
            }
        );
    }
}
