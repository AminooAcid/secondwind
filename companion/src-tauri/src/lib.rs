pub mod discovery;

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![commands::discover_nodes])
        .run(tauri::generate_context!())
        .expect("failed to run SecondWind companion");
}

mod commands {
    use std::time::Duration;

    use crate::discovery::{self, DiscoveredNode};

    const DISCOVERY_BROWSE_WINDOW_MS: u64 = 900;

    #[tauri::command]
    pub fn discover_nodes() -> Result<Vec<DiscoveredNode>, String> {
        discovery::discover_secondwind_nodes(Duration::from_millis(DISCOVERY_BROWSE_WINDOW_MS))
            .map_err(|error| error.to_string())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn discovery_window_is_short_enough_for_refresh_ui() {
            assert!(DISCOVERY_BROWSE_WINDOW_MS <= 1_000);
        }
    }
}
