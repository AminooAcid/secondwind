//! Tiny test CLI for the launcher library (developer tool only).

use sw_core::apps::default_catalog;
use sw_launcher::{NodeAvailability, decide};

fn main() {
    println!("sw-launcher decision table (node reachable):");
    for entry in default_catalog() {
        let decision = decide(&entry, NodeAvailability::Reachable);
        println!("  {:<12} -> {:?}", entry.app_id, decision);
    }
}
