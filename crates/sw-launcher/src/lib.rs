//! Host-side per-app launch logic (v0.3).
//!
//! One icon per app. Launch → decide where it runs from the app's policy
//! and the node's current state, waking the node first when possible. The
//! decision engine is pure; executing a decision (magic packets, spawning
//! the seamless client) lives in small side-effect helpers.

pub mod decision;
pub mod seamless;
pub mod wol;

pub use decision::*;
pub use seamless::*;
pub use wol::*;
