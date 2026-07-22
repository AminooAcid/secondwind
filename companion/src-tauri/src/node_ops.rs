//! Per-node operation serialization.
//!
//! Every state-changing operation against a node — manual toggles from the
//! UI, auto-connect bring-up/teardown, app launches, USB attach — runs
//! inside that node's lock. The UI and the background watcher can then
//! never interleave half-finished sequences on the same node (e.g. a
//! manual disk detach racing an auto-connect attach), while operations on
//! *different* nodes stay concurrent.
//!
//! This is the deliberately small first step toward a full per-node
//! reconciliation engine (see docs/BACKLOG.md): serialization first,
//! desired-state reconciliation after hardware validation.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use sw_core::NodeUuid;

#[derive(Debug, Default)]
pub struct NodeOps {
    locks: Mutex<HashMap<NodeUuid, Arc<Mutex<()>>>>,
}

pub type SharedNodeOps = Arc<NodeOps>;

pub fn new_shared() -> SharedNodeOps {
    Arc::new(NodeOps::default())
}

impl NodeOps {
    fn lock_for(&self, node_uuid: NodeUuid) -> Arc<Mutex<()>> {
        self.locks
            .lock()
            .expect("node ops registry lock")
            .entry(node_uuid)
            .or_default()
            .clone()
    }

    /// Runs `operation` while holding the node's lock. Blocks until any
    /// in-flight operation on the same node finishes.
    pub fn run<T>(&self, node_uuid: NodeUuid, operation: impl FnOnce() -> T) -> T {
        let node_lock = self.lock_for(node_uuid);
        let _guard = node_lock.lock().expect("node operation lock");
        operation()
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::atomic::{AtomicUsize, Ordering},
        time::Duration,
    };

    use super::*;

    fn uuid(digit: char) -> NodeUuid {
        NodeUuid::new(format!("00000000-0000-4000-8000-00000000000{digit}")).expect("valid uuid")
    }

    #[test]
    fn same_node_operations_are_serialized() {
        let ops = new_shared();
        let concurrent = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::new();
        for _ in 0..8 {
            let ops = ops.clone();
            let concurrent = concurrent.clone();
            let peak = peak.clone();
            handles.push(std::thread::spawn(move || {
                ops.run(uuid('1'), || {
                    let now = concurrent.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(now, Ordering::SeqCst);
                    std::thread::sleep(Duration::from_millis(5));
                    concurrent.fetch_sub(1, Ordering::SeqCst);
                });
            }));
        }
        for handle in handles {
            handle.join().expect("thread");
        }

        assert_eq!(peak.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn different_nodes_run_concurrently() {
        let ops = new_shared();
        let ops_clone = ops.clone();

        // Hold node 1's lock in a thread; node 2 must still proceed.
        let holder = std::thread::spawn(move || {
            ops_clone.run(uuid('1'), || {
                std::thread::sleep(Duration::from_millis(50));
            });
        });
        std::thread::sleep(Duration::from_millis(5));

        let start = std::time::Instant::now();
        ops.run(uuid('2'), || {});
        assert!(start.elapsed() < Duration::from_millis(40));

        holder.join().expect("holder thread");
    }

    #[test]
    fn operation_results_pass_through() {
        let ops = new_shared();

        let value = ops.run(uuid('3'), || 42);

        assert_eq!(value, 42);
    }
}
