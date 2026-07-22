//! Streaming-client supervision logic.
//!
//! Pure decisions live here (unit tested); the binary executes them. The
//! kiosk never hardcodes machine specifics: the client program, stream app
//! name, and timings are configuration with sane defaults, and the host
//! address always comes from the agent's kiosk state (the address that
//! actually reached the node).

use std::{collections::HashSet, path::PathBuf, time::Duration};

use sw_core::KioskState;

pub const STATE_FILE_ENV: &str = "SECONDWIND_KIOSK_STATE_FILE";
pub const STREAM_CLIENT_ENV: &str = "SECONDWIND_STREAM_CLIENT";
pub const STREAM_APP_ENV: &str = "SECONDWIND_STREAM_APP";
pub const POLL_MS_ENV: &str = "SECONDWIND_KIOSK_POLL_MS";
pub const ALLOW_EXIT_ENV: &str = "SECONDWIND_KIOSK_ALLOW_EXIT";

const DEFAULT_STREAM_CLIENT: &str = "moonlight";
const DEFAULT_STREAM_APP: &str = "Desktop";
const DEFAULT_POLL_MS: u64 = 500;
const MAX_BACKOFF: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KioskRuntimeConfig {
    pub state_file: PathBuf,
    pub client_program: String,
    pub stream_app: String,
    pub poll_interval: Duration,
    /// Development escape hatch: when enabled, pressing `q` on the pairing
    /// or idle screen exits the kiosk (never enabled in the product image).
    pub allow_exit_key: bool,
}

impl KioskRuntimeConfig {
    pub fn from_env() -> Result<Self, KioskConfigError> {
        let state_file = std::env::var_os(STATE_FILE_ENV)
            .map(PathBuf::from)
            .filter(|path| !path.as_os_str().is_empty())
            .ok_or(KioskConfigError::MissingStateFile)?;

        let client_program = env_or(STREAM_CLIENT_ENV, DEFAULT_STREAM_CLIENT);
        let stream_app = env_or(STREAM_APP_ENV, DEFAULT_STREAM_APP);
        let poll_interval = Duration::from_millis(
            std::env::var(POLL_MS_ENV)
                .ok()
                .and_then(|value| value.trim().parse().ok())
                .unwrap_or(DEFAULT_POLL_MS),
        );
        let allow_exit_key = std::env::var(ALLOW_EXIT_ENV)
            .map(|value| value.trim() == "1")
            .unwrap_or(false);

        Ok(Self {
            state_file,
            client_program,
            stream_app,
            poll_interval,
            allow_exit_key,
        })
    }
}

fn env_or(name: &str, default: &str) -> String {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| default.to_string())
}

#[derive(Debug)]
pub enum KioskConfigError {
    MissingStateFile,
}

impl std::fmt::Display for KioskConfigError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingStateFile => write!(
                formatter,
                "missing {STATE_FILE_ENV}; the node image or dev shell must provide the kiosk state file path"
            ),
        }
    }
}

impl std::error::Error for KioskConfigError {}

/// A concrete process invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec {
    pub program: String,
    pub args: Vec<String>,
}

pub fn pair_command(config: &KioskRuntimeConfig, host_address: &str, pin: &str) -> CommandSpec {
    CommandSpec {
        program: config.client_program.clone(),
        args: vec![
            "pair".to_string(),
            host_address.to_string(),
            "--pin".to_string(),
            pin.to_string(),
        ],
    }
}

pub fn stream_command(config: &KioskRuntimeConfig, host_address: &str) -> CommandSpec {
    CommandSpec {
        program: config.client_program.clone(),
        args: vec![
            "stream".to_string(),
            host_address.to_string(),
            config.stream_app.clone(),
            // Leave the stream when the host ends the session instead of
            // returning to the client's own UI.
            "--quit-after".to_string(),
        ],
    }
}

/// What the kiosk main loop should do for the current state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KioskAction {
    /// Show the state's screen; no streaming client should be running.
    ShowScreen,
    /// A stream should be up. `pair_first` carries the one-shot PIN when the
    /// inner pairing has not completed yet.
    EnsureStreaming {
        host_address: String,
        pair_first: Option<String>,
    },
}

/// Supervision memory: which one-shot PINs were already consumed and how
/// often the client crashed in a row (for restart backoff).
#[derive(Debug, Default)]
pub struct Supervisor {
    completed_pair_pins: HashSet<String>,
    consecutive_failures: u32,
}

impl Supervisor {
    pub fn decide(&self, state: &KioskState) -> KioskAction {
        match state {
            KioskState::Streaming {
                host_address,
                stream_pair_pin,
                ..
            } => KioskAction::EnsureStreaming {
                host_address: host_address.clone(),
                pair_first: stream_pair_pin
                    .as_ref()
                    .filter(|pin| !self.completed_pair_pins.contains(*pin))
                    .cloned(),
            },
            _ => KioskAction::ShowScreen,
        }
    }

    pub fn mark_pair_completed(&mut self, pin: &str) {
        self.completed_pair_pins.insert(pin.to_string());
    }

    /// Exponential backoff so a crashing client cannot spin the node.
    pub fn restart_backoff(&self) -> Duration {
        if self.consecutive_failures == 0 {
            return Duration::ZERO;
        }
        let exponent = self.consecutive_failures.min(5);
        Duration::from_secs(2_u64.pow(exponent)).min(MAX_BACKOFF)
    }

    pub fn record_failure(&mut self) {
        self.consecutive_failures = self.consecutive_failures.saturating_add(1);
    }

    pub fn record_healthy(&mut self) {
        self.consecutive_failures = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> KioskRuntimeConfig {
        KioskRuntimeConfig {
            state_file: PathBuf::from("state.json"),
            client_program: "client".to_string(),
            stream_app: "Desktop".to_string(),
            poll_interval: Duration::from_millis(500),
            allow_exit_key: false,
        }
    }

    fn streaming(pin: Option<&str>) -> KioskState {
        KioskState::Streaming {
            paired_host_name: "host".to_string(),
            host_address: "host-address".to_string(),
            stream_pair_pin: pin.map(|pin| pin.to_string()),
        }
    }

    #[test]
    fn idle_and_unpaired_states_show_screens() {
        let supervisor = Supervisor::default();

        assert_eq!(
            supervisor.decide(&KioskState::Starting),
            KioskAction::ShowScreen
        );
        assert_eq!(
            supervisor.decide(&KioskState::Idle {
                node_name: "node".to_string(),
                paired_host_name: "host".to_string(),
            }),
            KioskAction::ShowScreen
        );
    }

    #[test]
    fn streaming_with_new_pin_pairs_first() {
        let supervisor = Supervisor::default();

        assert_eq!(
            supervisor.decide(&streaming(Some("0000"))),
            KioskAction::EnsureStreaming {
                host_address: "host-address".to_string(),
                pair_first: Some("0000".to_string()),
            }
        );
    }

    #[test]
    fn consumed_pin_is_not_paired_twice() {
        let mut supervisor = Supervisor::default();
        supervisor.mark_pair_completed("0000");

        assert_eq!(
            supervisor.decide(&streaming(Some("0000"))),
            KioskAction::EnsureStreaming {
                host_address: "host-address".to_string(),
                pair_first: None,
            }
        );
    }

    #[test]
    fn backoff_grows_and_caps() {
        let mut supervisor = Supervisor::default();
        assert_eq!(supervisor.restart_backoff(), Duration::ZERO);

        supervisor.record_failure();
        assert_eq!(supervisor.restart_backoff(), Duration::from_secs(2));

        for _ in 0..10 {
            supervisor.record_failure();
        }
        assert_eq!(supervisor.restart_backoff(), MAX_BACKOFF);

        supervisor.record_healthy();
        assert_eq!(supervisor.restart_backoff(), Duration::ZERO);
    }

    #[test]
    fn commands_carry_runtime_values_only() {
        let pair = pair_command(&config(), "host-address", "1234");
        let stream = stream_command(&config(), "host-address");

        assert_eq!(pair.program, "client");
        assert_eq!(pair.args, vec!["pair", "host-address", "--pin", "1234"]);
        assert_eq!(
            stream.args,
            vec!["stream", "host-address", "Desktop", "--quit-after"]
        );
    }
}
