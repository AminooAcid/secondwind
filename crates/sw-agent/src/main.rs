use std::{env, error::Error, net::SocketAddr, path::PathBuf};

use sw_agent::{
    api::{AgentState, health_response, router},
    identity::load_or_create_identity,
};

const STATE_FILE_ENV: &str = "SECONDWIND_AGENT_STATE_FILE";
const BIND_ENV: &str = "SECONDWIND_AGENT_BIND";
const NODE_NAME_ENV: &str = "SECONDWIND_AGENT_NODE_NAME";
const DEFAULT_NODE_NAME: &str = "SecondWind node";

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let runtime = AgentRuntimeConfig::from_env()?;
    let identity = load_or_create_identity(&runtime.state_file, runtime.node_name)?;
    let state = AgentState::detect(identity.node_uuid, identity.node_name);

    if let Some(bind_addr) = runtime.bind_addr {
        let listener = tokio::net::TcpListener::bind(bind_addr).await?;
        axum::serve(listener, router(state)).await?;
    } else {
        let health = health_response(&state);
        println!(
            "{}",
            serde_json::to_string(&health).expect("health response should serialize")
        );
    }

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AgentRuntimeConfig {
    state_file: PathBuf,
    bind_addr: Option<SocketAddr>,
    node_name: String,
}

impl AgentRuntimeConfig {
    fn from_env() -> Result<Self, AgentRuntimeConfigError> {
        let state_file = env::var_os(STATE_FILE_ENV)
            .map(PathBuf::from)
            .filter(|path| !path.as_os_str().is_empty())
            .ok_or(AgentRuntimeConfigError::MissingStateFile)?;

        let bind_addr = match env::var(BIND_ENV) {
            Ok(value) if !value.trim().is_empty() => Some(
                value
                    .parse()
                    .map_err(|_| AgentRuntimeConfigError::InvalidBind)?,
            ),
            _ => None,
        };

        let node_name = env::var(NODE_NAME_ENV)
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_NODE_NAME.to_string());

        Ok(Self {
            state_file,
            bind_addr,
            node_name,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum AgentRuntimeConfigError {
    MissingStateFile,
    InvalidBind,
}

impl std::fmt::Display for AgentRuntimeConfigError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingStateFile => write!(
                formatter,
                "missing {STATE_FILE_ENV}; the node image or dev shell must provide an agent state file path"
            ),
            Self::InvalidBind => write!(
                formatter,
                "invalid {BIND_ENV}; expected a socket address supplied by configuration"
            ),
        }
    }
}

impl Error for AgentRuntimeConfigError {}
