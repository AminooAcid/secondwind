use sw_core::{HealthResponse, ServiceStatus};

fn main() {
    let health = HealthResponse {
        service: "sw-agent".to_string(),
        status: ServiceStatus::Starting,
    };

    println!(
        "{}",
        serde_json::to_string(&health).expect("health response should serialize")
    );
}
