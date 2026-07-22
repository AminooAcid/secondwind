//! Apollo (the host streaming server) install/detect/configure layer.
//!
//! Product boundary: users never see Apollo. The companion detects the
//! installation, keeps a SecondWind-managed block in its config, owns the
//! dashboard credentials (random, stored in companion state), arms one-shot
//! stream pairing PINs through Apollo's localhost API, and controls the
//! Windows service. Nothing here is surfaced in product terms other than
//! "Screen".
//!
//! No install paths, ports, or service names are hardcoded as *the* value:
//! everything has a detection list or an env/config override, with upstream
//! defaults as the fallback.

use std::{
    fs, io,
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
    time::Duration,
};

use serde::{Deserialize, Serialize};

pub const APOLLO_DIR_ENV: &str = "SECONDWIND_APOLLO_DIR";
pub const APOLLO_SERVICE_ENV: &str = "SECONDWIND_APOLLO_SERVICE";
pub const APOLLO_API_ENV: &str = "SECONDWIND_APOLLO_API";

/// Upstream defaults, used only as detection fallbacks. `sc.exe` matches
/// the *service name*, not the display name — the real Apollo installs as
/// `ApolloService` (display "Apollo Service"), verified on hardware.
const DEFAULT_SERVICE_NAMES: [&str; 3] = ["ApolloService", "SunshineService", "Apollo Service"];
const DEFAULT_EXECUTABLE_NAMES: [&str; 2] = ["sunshine.exe", "apollo.exe"];
const DEFAULT_CONFIG_RELATIVE: &str = "config/sunshine.conf";
const DEFAULT_API_BASE: &str = "https://localhost:47990";

const MANAGED_BLOCK_BEGIN: &str = "# --- SecondWind managed (do not edit between markers) ---";
const MANAGED_BLOCK_END: &str = "# --- end SecondWind managed ---";

const PIN_ARM_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApolloInstallation {
    pub install_dir: PathBuf,
    pub executable: PathBuf,
    pub config_file: PathBuf,
}

/// Directories worth probing, most specific first. The env override wins,
/// then per-machine program dirs; nothing here is required to exist.
pub fn default_install_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(dir) = std::env::var_os(APOLLO_DIR_ENV).filter(|dir| !dir.is_empty()) {
        candidates.push(PathBuf::from(dir));
    }

    for env_name in ["ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(programs) = std::env::var_os(env_name).filter(|dir| !dir.is_empty()) {
            let programs = PathBuf::from(programs);
            candidates.push(programs.join("Apollo"));
            candidates.push(programs.join("Sunshine"));
        }
    }

    candidates
}

/// Finds an Apollo installation among candidate directories.
pub fn detect_installation_in(candidates: &[PathBuf]) -> Option<ApolloInstallation> {
    for candidate in candidates {
        for executable_name in DEFAULT_EXECUTABLE_NAMES {
            let executable = candidate.join(executable_name);
            if executable.is_file() {
                return Some(ApolloInstallation {
                    install_dir: candidate.clone(),
                    executable,
                    config_file: candidate.join(DEFAULT_CONFIG_RELATIVE),
                });
            }
        }
    }

    None
}

pub fn detect_installation() -> Option<ApolloInstallation> {
    detect_installation_in(&default_install_candidates())
}

/// The SecondWind-managed configuration for Apollo. Only keys SecondWind
/// owns; everything else in the user's config file is preserved verbatim.
pub fn managed_config_entries(host_display_name: &str) -> Vec<(String, String)> {
    vec![
        // The name Moonlight clients see; branded so the node kiosk logs
        // stay in SecondWind terms.
        (
            "sunshine_name".to_string(),
            format!("SecondWind ({host_display_name})"),
        ),
        // The link is local; never poke holes in the router.
        ("upnp".to_string(), "off".to_string()),
    ]
}

/// Merges the managed entries into an existing config, replacing a previous
/// managed block if present and preserving all foreign lines.
pub fn merged_config(existing: &str, managed: &[(String, String)]) -> String {
    let mut kept_lines: Vec<&str> = Vec::new();
    let mut inside_managed = false;
    for line in existing.lines() {
        match line.trim() {
            trimmed if trimmed == MANAGED_BLOCK_BEGIN => inside_managed = true,
            trimmed if trimmed == MANAGED_BLOCK_END => inside_managed = false,
            _ if !inside_managed => kept_lines.push(line),
            _ => {}
        }
    }

    while kept_lines.last().is_some_and(|line| line.trim().is_empty()) {
        kept_lines.pop();
    }

    let mut result = kept_lines.join("\n");
    if !result.is_empty() {
        result.push('\n');
    }
    result.push_str(MANAGED_BLOCK_BEGIN);
    result.push('\n');
    for (key, value) in managed {
        result.push_str(&format!("{key} = {value}\n"));
    }
    result.push_str(MANAGED_BLOCK_END);
    result.push('\n');
    result
}

/// Applies the managed config block. Returns `true` when the file actually
/// changed (so the caller knows a service restart is needed for Apollo to
/// pick it up — Apollo only reads its config at startup).
pub fn apply_managed_config(
    installation: &ApolloInstallation,
    host_display_name: &str,
) -> Result<bool, ApolloError> {
    let existing = match fs::read_to_string(&installation.config_file) {
        Ok(contents) => contents,
        Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
        Err(source) => {
            return Err(ApolloError::ConfigRead {
                path: installation.config_file.clone(),
                source,
            });
        }
    };

    let merged = merged_config(&existing, &managed_config_entries(host_display_name));
    if merged == existing {
        return Ok(false);
    }
    if let Some(parent) = installation.config_file.parent() {
        fs::create_dir_all(parent).map_err(|source| ApolloError::ConfigWrite {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    fs::write(&installation.config_file, merged).map_err(|source| ApolloError::ConfigWrite {
        path: installation.config_file.clone(),
        source,
    })?;
    Ok(true)
}

/// Dashboard credentials owned by SecondWind. Random, generated once, stored
/// in the companion state dir; the user never sees Apollo's web UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApolloCredentials {
    pub username: String,
    pub password: String,
}

pub const CREDENTIALS_FILE_NAME: &str = "apollo-credentials.json";

/// Loads our dashboard credentials, creating and registering them with
/// Apollo on first use. The `bool` is `true` when they were just created
/// (the running service still holds the old ones and must be restarted
/// before the API accepts them — verified on hardware: a 401 otherwise).
pub fn load_or_create_credentials(
    state_root: &Path,
    installation: &ApolloInstallation,
) -> Result<(ApolloCredentials, bool), ApolloError> {
    let credentials_file = state_root.join(CREDENTIALS_FILE_NAME);
    match fs::read_to_string(&credentials_file) {
        Ok(contents) => serde_json::from_str(&contents)
            .map(|credentials| (credentials, false))
            .map_err(|source| ApolloError::Parse { source }),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let credentials = ApolloCredentials {
                username: format!("secondwind-{}", random_token(8)?),
                password: random_token(24)?,
            };
            // Register with Apollo first; only persist ours once accepted.
            set_apollo_credentials(installation, &credentials)?;
            let contents = serde_json::to_string_pretty(&credentials)
                .map_err(|source| ApolloError::Serialize { source })?;
            fs::create_dir_all(state_root).map_err(|source| ApolloError::ConfigWrite {
                path: state_root.to_path_buf(),
                source,
            })?;
            fs::write(&credentials_file, contents).map_err(|source| ApolloError::ConfigWrite {
                path: credentials_file.clone(),
                source,
            })?;
            Ok((credentials, true))
        }
        Err(source) => Err(ApolloError::ConfigRead {
            path: credentials_file,
            source,
        }),
    }
}

fn set_apollo_credentials(
    installation: &ApolloInstallation,
    credentials: &ApolloCredentials,
) -> Result<(), ApolloError> {
    let status = Command::new(&installation.executable)
        .arg("--creds")
        .arg(&credentials.username)
        .arg(&credentials.password)
        .status()
        .map_err(|source| ApolloError::Process { source })?;

    if status.success() {
        Ok(())
    } else {
        Err(ApolloError::CredentialSetupFailed)
    }
}

fn random_token(bytes: usize) -> Result<String, ApolloError> {
    let mut buffer = vec![0_u8; bytes];
    getrandom::getrandom(&mut buffer).map_err(|_| ApolloError::Randomness)?;
    Ok(buffer.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// Generates the one-shot PIN used for the node client ↔ Apollo pairing.
pub fn generate_stream_pair_pin() -> Result<String, ApolloError> {
    let mut buffer = [0_u8; 4];
    getrandom::getrandom(&mut buffer).map_err(|_| ApolloError::Randomness)?;
    let value = u32::from_le_bytes(buffer) % 10_000;
    Ok(format!("{value:04}"))
}

pub fn api_base() -> String {
    std::env::var(APOLLO_API_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_API_BASE.to_string())
}

/// Arms `pin` on Apollo so the node's incoming stream-pairing attempt is
/// accepted without any dashboard interaction.
pub fn arm_stream_pair_pin(
    api_base: &str,
    credentials: &ApolloCredentials,
    pin: &str,
    client_name: &str,
) -> Result<(), ApolloError> {
    let payload = serde_json::json!({ "pin": pin, "name": client_name });
    let agent = localhost_agent()?;
    let authorization = format!(
        "Basic {}",
        base64_encode(format!("{}:{}", credentials.username, credentials.password).as_bytes())
    );

    match agent
        .post(&format!("{api_base}/api/pin"))
        .set("authorization", &authorization)
        .set("content-type", "application/json")
        .send_string(&payload.to_string())
    {
        Ok(_) => Ok(()),
        // Stale credentials in the running service — the caller restarts.
        Err(ureq::Error::Status(401, _)) => Err(ApolloError::Unauthorized),
        Err(ureq::Error::Status(_, _)) => Err(ApolloError::PinRejected),
        Err(source) => Err(ApolloError::Api(Box::new(source))),
    }
}

/// Apollo serves its localhost API with a self-signed certificate. This
/// agent is used exclusively for loopback requests, so certificate identity
/// adds nothing — the trust boundary is the local machine.
fn localhost_agent() -> Result<ureq::Agent, ApolloError> {
    #[derive(Debug)]
    struct AcceptLocalhost {
        provider: Arc<rustls::crypto::CryptoProvider>,
    }

    impl rustls::client::danger::ServerCertVerifier for AcceptLocalhost {
        fn verify_server_cert(
            &self,
            _end_entity: &rustls::pki_types::CertificateDer<'_>,
            _intermediates: &[rustls::pki_types::CertificateDer<'_>],
            _server_name: &rustls::pki_types::ServerName<'_>,
            _ocsp_response: &[u8],
            _now: rustls::pki_types::UnixTime,
        ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
            Ok(rustls::client::danger::ServerCertVerified::assertion())
        }

        fn verify_tls12_signature(
            &self,
            message: &[u8],
            cert: &rustls::pki_types::CertificateDer<'_>,
            dss: &rustls::DigitallySignedStruct,
        ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
            rustls::crypto::verify_tls12_signature(
                message,
                cert,
                dss,
                &self.provider.signature_verification_algorithms,
            )
        }

        fn verify_tls13_signature(
            &self,
            message: &[u8],
            cert: &rustls::pki_types::CertificateDer<'_>,
            dss: &rustls::DigitallySignedStruct,
        ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
            rustls::crypto::verify_tls13_signature(
                message,
                cert,
                dss,
                &self.provider.signature_verification_algorithms,
            )
        }

        fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
            self.provider
                .signature_verification_algorithms
                .supported_schemes()
        }
    }

    let provider = rustls::crypto::CryptoProvider::get_default()
        .cloned()
        .unwrap_or_else(|| Arc::new(rustls::crypto::aws_lc_rs::default_provider()));
    let config = rustls::ClientConfig::builder_with_provider(provider.clone())
        .with_safe_default_protocol_versions()
        .map_err(ApolloError::Tls)?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(AcceptLocalhost { provider }))
        .with_no_client_auth();

    Ok(ureq::builder()
        .tls_config(Arc::new(config))
        .timeout(PIN_ARM_TIMEOUT)
        .build())
}

fn base64_encode(input: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(input)
}

pub fn service_name() -> String {
    std::env::var(APOLLO_SERVICE_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_SERVICE_NAMES[0].to_string())
}

/// Ensures the Apollo Windows service is running. Tries the configured or
/// known service names; success is "some Apollo service is running".
pub fn ensure_service_running() -> Result<(), ApolloError> {
    let mut names = vec![service_name()];
    for default in DEFAULT_SERVICE_NAMES {
        if !names.iter().any(|name| name == default) {
            names.push(default.to_string());
        }
    }

    for name in &names {
        if service_state(name) == ServiceState::Running {
            return Ok(());
        }
    }

    for name in &names {
        if service_state(name) == ServiceState::Unknown {
            continue; // not this service name
        }
        let _ = Command::new("sc.exe").args(["start", name]).status();
        // Wait for it to actually reach RUNNING before we use the API
        // (a start returns during START_PENDING).
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        while std::time::Instant::now() < deadline {
            match service_state(name) {
                ServiceState::Running => return Ok(()),
                ServiceState::Stopped => {
                    let _ = Command::new("sc.exe").args(["start", name]).status();
                }
                _ => {}
            }
            std::thread::sleep(Duration::from_millis(500));
        }
    }

    Err(ApolloError::ServiceUnavailable)
}

/// Coarse service state parsed from `sc query`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ServiceState {
    Running,
    Stopped,
    /// STOP_PENDING / START_PENDING / anything transitional.
    Transitional,
    Unknown,
}

fn service_state(name: &str) -> ServiceState {
    let Ok(output) = Command::new("sc.exe").args(["query", name]).output() else {
        return ServiceState::Unknown;
    };
    let text = String::from_utf8_lossy(&output.stdout);
    if text.contains("RUNNING") {
        ServiceState::Running
    } else if text.contains("STOPPED") {
        ServiceState::Stopped
    } else if text.contains("PENDING") {
        ServiceState::Transitional
    } else {
        ServiceState::Unknown
    }
}

fn service_query_running(name: &str) -> Result<bool, ApolloError> {
    Ok(service_state(name) == ServiceState::Running)
}

/// Which service name is actually installed (first one that responds to a
/// query), so restart targets the right one.
fn installed_service_name() -> Option<String> {
    let mut names = vec![service_name()];
    for default in DEFAULT_SERVICE_NAMES {
        if !names.iter().any(|name| name == default) {
            names.push(default.to_string());
        }
    }
    names.into_iter().find(|name| {
        Command::new("sc.exe")
            .args(["query", name])
            .output()
            .map(|output| {
                let text = String::from_utf8_lossy(&output.stdout);
                text.contains("STATE") // any state line = the service exists
            })
            .unwrap_or(false)
    })
}

/// Restarts Apollo so it reloads freshly written config + credentials, then
/// waits for the dashboard API to answer (needs admin, like the rest of
/// the Apollo control path).
pub fn restart_service_and_wait(credentials: &ApolloCredentials) -> Result<(), ApolloError> {
    let name = installed_service_name().ok_or(ApolloError::ServiceUnavailable)?;

    // Stop and wait for the *fully STOPPED* state — Apollo takes up to
    // ~30 s and passes through STOP_PENDING, during which a start fails
    // with 1056 "already running" (found on hardware).
    let _ = Command::new("sc.exe").args(["stop", &name]).status();
    let stop_deadline = std::time::Instant::now() + Duration::from_secs(45);
    while service_state(&name) != ServiceState::Stopped {
        if std::time::Instant::now() >= stop_deadline {
            return Err(ApolloError::ServiceUnavailable);
        }
        std::thread::sleep(Duration::from_millis(500));
    }

    // Start and wait for RUNNING; retry the start while it is still
    // transitioning.
    let start_deadline = std::time::Instant::now() + Duration::from_secs(30);
    loop {
        match service_state(&name) {
            ServiceState::Running => break,
            ServiceState::Transitional => {}
            _ => {
                let _ = Command::new("sc.exe").args(["start", &name]).status();
            }
        }
        if std::time::Instant::now() >= start_deadline {
            return Err(ApolloError::ServiceUnavailable);
        }
        std::thread::sleep(Duration::from_millis(500));
    }

    wait_for_api_ready(&api_base(), credentials)
}

/// Polls the dashboard API until an authenticated request succeeds (the
/// service has finished loading our credentials), or times out.
fn wait_for_api_ready(api_base: &str, credentials: &ApolloCredentials) -> Result<(), ApolloError> {
    let agent = localhost_agent()?;
    let authorization = format!(
        "Basic {}",
        base64_encode(format!("{}:{}", credentials.username, credentials.password).as_bytes())
    );
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    let url = format!("{api_base}/api/apps");

    loop {
        // `/api/apps` needs auth: 2xx means our creds are live; a transport
        // error means the service is still coming up; 401 means it hasn't
        // reloaded creds yet — keep waiting on both.
        match agent.get(&url).set("authorization", &authorization).call() {
            Ok(_) => return Ok(()),
            Err(ureq::Error::Status(code, _)) if (200..300).contains(&code) => return Ok(()),
            _ => {}
        }
        if std::time::Instant::now() >= deadline {
            return Err(ApolloError::ServiceUnavailable);
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

#[derive(Debug)]
pub enum ApolloError {
    NotInstalled,
    ConfigRead { path: PathBuf, source: io::Error },
    ConfigWrite { path: PathBuf, source: io::Error },
    Parse { source: serde_json::Error },
    Serialize { source: serde_json::Error },
    Process { source: io::Error },
    CredentialSetupFailed,
    Randomness,
    Unauthorized,
    Tls(rustls::Error),
    Api(Box<ureq::Error>),
    PinRejected,
    ServiceUnavailable,
}

impl std::fmt::Display for ApolloError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotInstalled => write!(
                formatter,
                "SecondWind's screen engine is not installed on this PC yet"
            ),
            Self::ConfigRead { .. } | Self::ConfigWrite { .. } => {
                write!(formatter, "could not update the screen engine settings")
            }
            Self::Parse { .. } | Self::Serialize { .. } => {
                write!(formatter, "could not read the screen engine credentials")
            }
            Self::Process { .. } | Self::CredentialSetupFailed => {
                write!(formatter, "could not prepare the screen engine")
            }
            Self::Randomness => write!(formatter, "could not generate a secure code"),
            Self::Tls(_) | Self::Api(_) => {
                write!(formatter, "could not talk to the screen engine")
            }
            Self::PinRejected => {
                write!(formatter, "the screen engine rejected the pairing code")
            }
            Self::Unauthorized => {
                write!(
                    formatter,
                    "the screen engine rejected SecondWind's credentials"
                )
            }
            Self::ServiceUnavailable => {
                write!(formatter, "the screen engine service could not be started")
            }
        }
    }
}

impl std::error::Error for ApolloError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ConfigRead { source, .. } | Self::ConfigWrite { source, .. } => Some(source),
            Self::Parse { source } | Self::Serialize { source } => Some(source),
            Self::Process { source } => Some(source),
            Self::Tls(source) => Some(source),
            Self::Api(source) => Some(source),
            Self::NotInstalled
            | Self::CredentialSetupFailed
            | Self::Randomness
            | Self::PinRejected
            | Self::Unauthorized
            | Self::ServiceUnavailable => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_installation_from_candidate_dirs() {
        let root =
            std::env::temp_dir().join(format!("secondwind-apollo-detect-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let install = root.join("Apollo");
        fs::create_dir_all(&install).expect("create install dir");
        fs::write(install.join("sunshine.exe"), "stub").expect("write stub exe");

        let detected =
            detect_installation_in(&[root.join("missing"), install.clone()]).expect("detected");

        assert_eq!(detected.install_dir, install);
        assert_eq!(detected.executable, install.join("sunshine.exe"));
        assert_eq!(detected.config_file, install.join(DEFAULT_CONFIG_RELATIVE));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn detection_returns_none_without_installation() {
        let root =
            std::env::temp_dir().join(format!("secondwind-apollo-none-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);

        assert!(detect_installation_in(&[root]).is_none());
    }

    #[test]
    fn managed_block_is_added_and_preserves_foreign_lines() {
        let existing = "min_log_level = 2\ncustom_key = keep-me\n";

        let merged = merged_config(existing, &managed_config_entries("Host"));

        assert!(merged.contains("min_log_level = 2"));
        assert!(merged.contains("custom_key = keep-me"));
        assert!(merged.contains("sunshine_name = SecondWind (Host)"));
        assert!(merged.contains("upnp = off"));
        assert!(merged.contains(MANAGED_BLOCK_BEGIN));
        assert!(merged.contains(MANAGED_BLOCK_END));
    }

    #[test]
    fn managed_block_is_replaced_not_duplicated() {
        let first = merged_config("keep = yes\n", &managed_config_entries("First"));

        let second = merged_config(&first, &managed_config_entries("Second"));

        assert_eq!(second.matches(MANAGED_BLOCK_BEGIN).count(), 1);
        assert_eq!(second.matches(MANAGED_BLOCK_END).count(), 1);
        assert!(second.contains("SecondWind (Second)"));
        assert!(!second.contains("SecondWind (First)"));
        assert!(second.contains("keep = yes"));
    }

    #[test]
    fn stream_pair_pin_is_four_digits() {
        let pin = generate_stream_pair_pin().expect("pin");

        assert_eq!(pin.len(), 4);
        assert!(pin.bytes().all(|byte| byte.is_ascii_digit()));
    }

    #[test]
    fn managed_config_never_bakes_in_machine_values() {
        let merged = merged_config("", &managed_config_entries("AnyHost"));

        // Guard: no IPs, resolutions, or codec names in the managed block.
        for forbidden in ["192.168", "1920", "hevc", "h264", "adapter"] {
            assert!(
                !merged.to_lowercase().contains(forbidden),
                "managed config must not contain {forbidden}"
            );
        }
    }
}
