pub const HEALTH_PATH: &str = "/v1/health";
pub const CAPABILITIES_PATH: &str = "/v1/capabilities";
pub const PAIRING_PATH: &str = "/v1/pairing";
pub const SCREEN_PATH: &str = "/v1/screen";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentRoute {
    Health,
    Capabilities,
    Pairing,
    Screen,
}

impl AgentRoute {
    pub fn path(self) -> &'static str {
        match self {
            Self::Health => HEALTH_PATH,
            Self::Capabilities => CAPABILITIES_PATH,
            Self::Pairing => PAIRING_PATH,
            Self::Screen => SCREEN_PATH,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_paths_are_versioned() {
        for route in [
            AgentRoute::Health,
            AgentRoute::Capabilities,
            AgentRoute::Pairing,
            AgentRoute::Screen,
        ] {
            assert!(route.path().starts_with("/v1/"));
        }
    }
}
