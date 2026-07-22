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
            if self.present.insert(*node, 0).is_none() {
                events.push(PresenceEvent::Appeared(*node))
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
pub fn spawn(
    app: tauri::AppHandle,
    active_screens: ActiveScreens,
    node_ops: crate::node_ops::SharedNodeOps,
) {
    std::thread::spawn(move || {
        let mut tracker = PresenceTracker::default();
        // Last-known attached disk IQN per node, so a vanished node's
        // initiator session can still be flushed + detached locally.
        let mut attached_disks: HashMap<NodeUuid, String> = HashMap::new();
        loop {
            scan_once(
                &app,
                &active_screens,
                &node_ops,
                &mut tracker,
                &mut attached_disks,
            );
            std::thread::sleep(SCAN_INTERVAL);
        }
    });
}

fn scan_once(
    app: &tauri::AppHandle,
    active_screens: &ActiveScreens,
    node_ops: &crate::node_ops::SharedNodeOps,
    tracker: &mut PresenceTracker,
    attached_disks: &mut HashMap<NodeUuid, String>,
) {
    use tauri::Emitter;

    // Discovery hiccups (adapter changes) are normal; an empty scan lets
    // the debounce absorb them.
    let discovered = crate::discovery::discover_secondwind_nodes(SCAN_WINDOW).unwrap_or_default();
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

                // Bring-up runs inside the node's operation lock so it can
                // never interleave with a manual toggle from the UI.
                match node_ops.run(node_uuid, || auto_bring_up(app, node)) {
                    Ok(Some(outcome)) => {
                        if outcome.screen_up
                            && let Ok(mut active) = active_screens.lock()
                        {
                            active.insert(node_uuid);
                        }
                        if let Some(iqn) = outcome.disk_iqn {
                            attached_disks.insert(node_uuid, iqn);
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
                    Ok(None) => {}
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
                // Teardown in reverse feature order: flush + detach the disk
                // first, then the screen (which has already ended with the
                // link; we just update state and tell the user). Serialized
                // per node like every other operation.
                if let Some(iqn) = attached_disks.remove(&node_uuid) {
                    node_ops.run(node_uuid, || {
                        let _ = crate::disk_control::detach_local(&iqn);
                    });
                }
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

/// What auto-connect actually brought up for a node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BringUpOutcome {
    pub screen_up: bool,
    pub disk_iqn: Option<String>,
}

/// Brings every always-on feature up for a paired node, in order: screen,
/// then disk. Returns Ok(None) when the node is not paired or nothing is
/// marked always-on.
fn auto_bring_up(
    app: &tauri::AppHandle,
    node: &crate::discovery::DiscoveredNode,
) -> Result<Option<BringUpOutcome>, String> {
    let state_root = crate::commands_support::state_root(app)?;
    let mut state =
        crate::host_state::HostState::load_or_create(&state_root).map_err(|e| e.to_string())?;

    let Some(paired) = state.paired_node(&node.node_uuid) else {
        return Ok(None);
    };
    let screen_wanted = paired.screen.always_on;
    let disk_wanted = paired.disk.always_on;
    if !screen_wanted && !disk_wanted {
        return Ok(None);
    }

    let endpoint = crate::screen_control::paired_endpoint(
        &state,
        &node.node_uuid,
        node.addresses.clone(),
        node.api_port,
    )
    .map_err(|error| error.to_string())?;

    let mut outcome = BringUpOutcome {
        screen_up: false,
        disk_iqn: None,
    };

    if screen_wanted {
        crate::screen_control::connect_screen(&mut state, &state_root, &node.node_uuid, &endpoint)
            .map_err(|error| error.to_string())?;
        outcome.screen_up = true;
    }

    if disk_wanted {
        // A node without a provisioned data disk is normal — stay quiet.
        match crate::disk_control::connect_disk(&state, &node.node_uuid, &endpoint) {
            Ok(disk) => outcome.disk_iqn = Some(disk.target_iqn),
            Err(crate::disk_control::DiskControlError::Unavailable { .. }) => {}
            Err(error) => {
                if outcome.screen_up {
                    // Screen made it up; report the disk problem softly.
                    use tauri::Emitter;
                    let _ = app.emit(
                        NODE_CONNECTED_EVENT,
                        NodeEventPayload {
                            node_uuid: node.node_uuid,
                            display_name: node.node_name.clone(),
                            message: error.to_string(),
                        },
                    );
                } else {
                    return Err(error.to_string());
                }
            }
        }
    }

    // USB auto-attach rules (v0.4): best-effort, last in the bring-up order.
    let rules = state
        .paired_node(&node.node_uuid)
        .map(|paired| paired.usb_auto_attach.clone())
        .unwrap_or_default();
    if !rules.is_empty()
        && let Ok(devices) =
            crate::node_client::get_usb_devices(&endpoint, Some(&state.certificate))
    {
        for device in devices.devices {
            let wanted = rules.iter().any(|rule| {
                rule.vendor_id == device.vendor_id && rule.product_id == device.product_id
            });
            if wanted && !device.bound {
                let _ = crate::usb_control::attach_device(&state, &endpoint, &device.bus_id);
            }
        }
    }

    Ok(Some(outcome))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uuid(digit: char) -> NodeUuid {
        NodeUuid::new(format!("00000000-0000-4000-8000-00000000000{digit}")).expect("valid uuid")
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
