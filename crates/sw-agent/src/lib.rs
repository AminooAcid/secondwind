pub mod api;
pub mod apps;
pub mod capability_detection;
pub mod discovery;
pub mod disk;
pub mod identity;
pub mod pairing_state;
pub mod share;
pub mod tls;
pub mod usb;

/// Certificate storage/generation is shared peer logic and lives in
/// `sw_core::certificates`; re-exported here for agent-local call sites.
pub use sw_core::certificates;
