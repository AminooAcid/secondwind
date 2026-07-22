//! SecondWind node kiosk.
//!
//! Owns the node's physical screen through three states driven by the
//! agent's kiosk state file: pairing screen (QR + PIN), paired idle screen,
//! and supervised streaming. Users see SecondWind screens only — upstream
//! tool names never appear outside debug logs.

pub mod screens;
pub mod supervise;

pub fn startup_status() -> &'static str {
    "sw-kiosk scaffold ready"
}
