//! Link-up auto-connect.
//!
//! A background watcher periodically browses mDNS for paired nodes. When a
//! paired node appears, every "always on" feature is brought up without
//! clicks (v0.1: Screen). When it disappears, features are torn down in
//! reverse order (v0.1: the screen path simply ends; later phases add disk
//! flush ahead of display teardown). Presence is debounced so one missed
//! mDNS scan — e.g. the host's USB Ethernet adapter re-enumerating — does
//! not bounce the session.

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
    time::Duration,
};

use sw_core::NodeUuid;

/// Consecutive missed scans before a node counts as gone.
const DEFAULT_MISS_THRESHOLD: u32 = 3;
/// Pause between browse windows.
const SCAN_INTERVAL: Duration = Duration::from_secs(4);
/// Browse window per scan.
const SCAN_WINDOW: Duration = Duration::from_millis(1200);

pub const NODE_CONNECTED_EVENT: &str = "secondwind://node-connected";
pub const NODE_DISCONNECTED_EVENT: &str = "secondwind://node-disconnected";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PresenceEvent {
    Appeared(NodeUuid),
    Vanished(NodeUuid),
}

/// Debounced presence tracking, independent of any I/O.
#[derive(Debug)]
pub struct PresenceTracker {
    miss_threshold: u32,
    /// Present nodes → consecutive scans they were missing.
    present: HashMap<NodeUuid, u32>,
}

impl PresenceTracker {
    pub fn new(miss_threshold: u32) -> Self {
        Self {
            miss_threshold: miss_threshold.max(1),
            present: HashMap::new(),
        }
    }

    pub fn observe(&mut self, seen: &HashSet<NodeUuid>) -> Vec<PresenceEvent> {
        let mut events = Vec::new();

        for node in seen {
            match self.present.insert(*node, 0) {
                None => events.push(PresenceEvent::Appeared(*node)),
                Some(_) => {}
            }
        }

        let mut vanished = Vec::new();
        for (node, misses) in self.present.iter_mut() {
            if !seen.contains(node) {
                *misses += 1;
                if *misses >= self.miss_threshold {
                    vanished.push(*node);
                }
            }
        }
        for node in vanished {
            self.present.remove(&node);
            events.push(PresenceEvent::Vanished(node));
        }

        events
    }
}

impl Default for PresenceTracker {
    fn default() -> Self {
        Self::new(DEFAULT_MISS_THRESHOLD)
    }
}

/// Nodes whose screen the companion currently keeps up. Shared between the
/// watcher and the manual toggle command so they never fight.
pub type ActiveScreens = Arc<Mutex<HashSet<NodeUuid>>>;

pub fn new_active_screens() -> ActiveScreens {
    Arc::new(Mutex::new(HashSet::new()))
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct NodeEventPayload {
    pub node_uuid: NodeUuid,
    pub display_name: String,
    pub message: String,
}

/// Spawns the watcher thread. Runs for the lifetime of the companion.
pub fn spawn(app: tauri::AppHandle, active_screens: ActiveScreens) {
    std::thread::spawn(move || {
        let mut tracker = PresenceTracker::default();
        loop {
            scan_once(&app, &active_screens, &mut tracker);
            std::thread::sleep(SCAN_INTERVAL);
        }
    });
}

fn scan_once(app: &tauri::AppHandle, active_screens: &ActiveScreens, tracker: &mut PresenceTracker) {
    use tauri::Emitter;

    let discovered = match crate::discovery::discover_secondwind_nodes(SCAN_WINDOW) {
        Ok(nodes) => nodes,
        // Discovery hiccups (adapter changes) are normal; treat as empty
        // scan so debounce absorbs them.
        Err(_) => Vec::new(),
    };
    let by_uuid: HashMap<NodeUuid, crate::discovery::DiscoveredNode> = discovered
        .into_iter()
        .map(|node| (node.node_uuid, node))
        .collect();
    let seen: HashSet<NodeUuid> = by_uuid.keys().copied().collect();

    for event in tracker.observe(&seen) {
        match event {
            PresenceEvent::Appeared(node_uuid) => {
                let Some(node) = by_uuid.get(&node_uuid) else {
                    continue;
                };
                let already_active = active_screens
                    .lock()
                    .map(|active| active.contains(&node_uuid))
                    .unwrap_or(false);
                if already_active {
                    continue;
                }

                match auto_bring_up(app, node) {
                    Ok(true) => {
                        if let Ok(mut active) = active_screens.lock() {
                            active.insert(node_uuid);
                        }
                        let _ = app.emit(
                            NODE_CONNECTED_EVENT,
                            NodeEventPayload {
                                node_uuid,
                                display_name: node.node_name.clone(),
                                message: format!("{} connected — ready", node.node_name),
                            },
                        );
                    }
                    Ok(false) => {}
                    Err(message) => {
                        let _ = app.emit(
                            NODE_CONNECTED_EVENT,
                            NodeEventPayload {
                                node_uuid,
                                display_name: node.node_name.clone(),
                                message,
                            },
                        );
                    }
                }
            }
            PresenceEvent::Vanished(node_uuid) => {
                let was_active = active_screens
                    .lock()
                    .map(|mut active| active.remove(&node_uuid))
                    .unwrap_or(false);
                if was_active {
                    let _ = app.emit(
                        NODE_DISCONNECTED_EVENT,
                        NodeEventPayload {
                            node_uuid,
                            display_name: String::new(),
                            message: "Your extra screen was disconnected.".to_string(),
                        },
                    );
                }
            }
        }
    }
}

/// Brings the screen up for a paired, always-on node. Returns Ok(false)
/// when the node is not paired or not marked always-on (nothing to do).
fn auto_bring_up(
    app: &tauri::AppHandle,
    node: &crate::discovery::DiscoveredNode,
) -> Result<bool, String> {
    let state_root = crate::commands_support::state_root(app)?;
    let mut state =
        crate::host_state::HostState::load_or_create(&state_root).map_err(|e| e.to_string())?;

    let Some(paired) = state.paired_node(&node.node_uuid) else {
        return Ok(false);
    };
    if !paired.screen.always_on {
        return Ok(false);
    }

    let endpoint = crate::screen_control::paired_endpoint(
        &state,
        &node.node_uuid,
        node.addresses.clone(),
        node.api_port,
    )
    .map_err(|error| error.to_string())?;

    crate::screen_control::connect_screen(&mut state, &state_root, &node.node_uuid, &endpoint)
        .map(|_| true)
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uuid(digit: char) -> NodeUuid {
        NodeUuid::new(format!("00000000-0000-4000-8000-00000000000{digit}"))
            .expect("valid uuid")
    }

    #[test]
    fn appearing_node_fires_once() {
        let mut tracker = PresenceTracker::new(3);
        let seen: HashSet<_> = [uuid('1')].into_iter().collect();

        let first = tracker.observe(&seen);
        let second = tracker.observe(&seen);

        assert_eq!(first, vec![PresenceEvent::Appeared(uuid('1'))]);
        assert!(second.is_empty());
    }

    #[test]
    fn single_missed_scan_does_not_disconnect() {
        let mut tracker = PresenceTracker::new(3);
        tracker.observe(&[uuid('1')].into_iter().collect());

        let empty = HashSet::new();
        assert!(tracker.observe(&empty).is_empty());
        assert!(tracker.observe(&empty).is_empty());

        // Node returns before the threshold: no vanish, no re-appear event.
        let back = tracker.observe(&[uuid('1')].into_iter().collect());
        assert!(back.is_empty());
    }

    #[test]
    fn sustained_absence_disconnects_then_reappearance_reconnects() {
        let mut tracker = PresenceTracker::new(2);
        tracker.observe(&[uuid('1')].into_iter().collect());

        let empty = HashSet::new();
        assert!(tracker.observe(&empty).is_empty());
        assert_eq!(
            tracker.observe(&empty),
            vec![PresenceEvent::Vanished(uuid('1'))]
        );

        assert_eq!(
            tracker.observe(&[uuid('1')].into_iter().collect()),
            vec![PresenceEvent::Appeared(uuid('1'))]
        );
    }

    #[test]
    fn multiple_nodes_tracked_independently() {
        let mut tracker = PresenceTracker::new(2);
        let both: HashSet<_> = [uuid('1'), uuid('2')].into_iter().collect();
        let events = tracker.observe(&both);
        assert_eq!(events.len(), 2);

        let only_one: HashSet<_> = [uuid('1')].into_iter().collect();
        assert!(tracker.observe(&only_one).is_empty());
        assert_eq!(
            tracker.observe(&only_one),
            vec![PresenceEvent::Vanished(uuid('2'))]
        );
    }
}
