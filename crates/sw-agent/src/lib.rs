pub mod api;
pub mod capability_detection;
pub mod discovery;
pub mod identity;
pub mod pairing_state;
pub mod tls;

/// Certificate storage/generation is shared peer logic and lives in
/// `sw_core::certificates`; re-exported here for agent-local call sites.
pub use sw_core::certificates;
