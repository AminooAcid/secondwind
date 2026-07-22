pub const HEALTH_PATH: &str = "/v1/health";
pub const CAPABILITIES_PATH: &str = "/v1/capabilities";
pub const PAIRING_PATH: &str = "/v1/pairing";
pub const SCREEN_PATH: &str = "/v1/screen";
pub const DISK_PATH: &str = "/v1/disk";
pub const APPS_PATH: &str = "/v1/apps";
pub const SHARE_PATH: &str = "/v1/share";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentRoute {
    Health,
    Capabilities,
    Pairing,
    Screen,
    Disk,
    Apps,
    Share,
}

impl AgentRoute {
    pub fn path(self) -> &'static str {
        match self {
            Self::Health => HEALTH_PATH,
            Self::Capabilities => CAPABILITIES_PATH,
            Self::Pairing => PAIRING_PATH,
            Self::Screen => SCREEN_PATH,
            Self::Disk => DISK_PATH,
            Self::Apps => APPS_PATH,
            Self::Share => SHARE_PATH,
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
            AgentRoute::Disk,
            AgentRoute::Apps,
            AgentRoute::Share,
        ] {
            assert!(route.path().starts_with("/v1/"));
        }
    }
}
