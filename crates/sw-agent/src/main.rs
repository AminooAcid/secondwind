use sw_agent::api::{AgentState, health_response};
use sw_core::NodeUuid;

fn main() {
    let node_uuid = NodeUuid::new("unconfigured-node").expect("static fallback is non-empty");
    let state = AgentState::detect(node_uuid, "unconfigured-node".to_string());
    let health = health_response(&state);

    println!(
        "{}",
        serde_json::to_string(&health).expect("health response should serialize")
    );
}
