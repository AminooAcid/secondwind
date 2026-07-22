//! HTTPS client for talking to a node's agent API.
//!
//! Trust model (matches the locked pairing decision): node certificates are
//! self-signed, so the companion never uses WebPKI for nodes. Instead every
//! connection pins the node's certificate by SHA-256 fingerprint — learned
//! from mDNS/QR before pairing and from persisted trust afterwards. After
//! pairing, the companion also presents its host certificate for mutual TLS.

use std::{io::Cursor, net::IpAddr, sync::Arc, time::Duration};

use rustls::{
    ClientConfig, DigitallySignedStruct, SignatureScheme,
    client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
    crypto::CryptoProvider,
    pki_types::{CertificateDer, PrivateKeyDer, ServerName, UnixTime},
};
use sw_core::{
    CertificateMaterial, DiskCommandRequest, DiskStatusResponse, PairingRequest, PairingResponse,
    ScreenCommandRequest, ScreenStatusResponse,
    agent_api::{DISK_PATH, PAIRING_PATH, SCREEN_PATH},
    certificate_der_from_pem, fingerprint_of_der,
};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Server-certificate verifier that accepts exactly one pinned fingerprint.
#[derive(Debug)]
struct PinnedFingerprintVerifier {
    expected_fingerprint: String,
    provider: Arc<CryptoProvider>,
}

impl PinnedFingerprintVerifier {
    fn new(expected_fingerprint: &str, provider: Arc<CryptoProvider>) -> Arc<Self> {
        Arc::new(Self {
            expected_fingerprint: expected_fingerprint.trim().to_ascii_uppercase(),
            provider,
        })
    }

    fn matches(&self, certificate: &CertificateDer<'_>) -> bool {
        fingerprint_of_der(certificate.as_ref()).to_ascii_uppercase()
            == self.expected_fingerprint
    }
}

impl ServerCertVerifier for PinnedFingerprintVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        if self.matches(end_entity) {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::InvalidCertificate(
                rustls::CertificateError::ApplicationVerificationFailure,
            ))
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
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
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

fn crypto_provider() -> Result<Arc<CryptoProvider>, NodeClientError> {
    if let Some(provider) = CryptoProvider::get_default() {
        return Ok(provider.clone());
    }

    Ok(Arc::new(rustls::crypto::aws_lc_rs::default_provider()))
}

/// Builds a client TLS config pinned to `node_fingerprint`. When
/// `client_identity` is provided the host certificate is presented for mTLS
/// (required by paired nodes).
pub fn pinned_client_config(
    node_fingerprint: &str,
    client_identity: Option<&CertificateMaterial>,
) -> Result<ClientConfig, NodeClientError> {
    let provider = crypto_provider()?;
    let verifier = PinnedFingerprintVerifier::new(node_fingerprint, provider.clone());
    let builder = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(NodeClientError::Tls)?
        .dangerous()
        .with_custom_certificate_verifier(verifier);

    let config = match client_identity {
        Some(identity) => {
            let certificate = CertificateDer::from(
                certificate_der_from_pem(&identity.certificate_pem)
                    .map_err(|_| NodeClientError::InvalidHostCertificate)?,
            );
            let private_key = private_key_from_pem(&identity.private_key_pem)?;
            builder
                .with_client_auth_cert(vec![certificate], private_key)
                .map_err(NodeClientError::Tls)?
        }
        None => builder.with_no_client_auth(),
    };

    Ok(config)
}

fn private_key_from_pem(private_key_pem: &str) -> Result<PrivateKeyDer<'static>, NodeClientError> {
    rustls_pemfile::private_key(&mut Cursor::new(private_key_pem.as_bytes()))
        .map_err(|_| NodeClientError::InvalidHostCertificate)?
        .ok_or(NodeClientError::InvalidHostCertificate)
}

/// A reachable node API endpoint: candidate addresses plus pinned trust.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeEndpoint {
    pub addresses: Vec<IpAddr>,
    pub api_port: u16,
    pub certificate_fingerprint: String,
}

impl NodeEndpoint {
    fn candidate_urls(&self, path: &str) -> Vec<String> {
        self.addresses
            .iter()
            .map(|address| match address {
                IpAddr::V4(v4) => format!("https://{v4}:{}{path}", self.api_port),
                IpAddr::V6(v6) => format!("https://[{v6}]:{}{path}", self.api_port),
            })
            .collect()
    }
}

fn http_agent(
    endpoint: &NodeEndpoint,
    client_identity: Option<&CertificateMaterial>,
) -> Result<ureq::Agent, NodeClientError> {
    let tls = pinned_client_config(&endpoint.certificate_fingerprint, client_identity)?;
    Ok(ureq::builder()
        .tls_config(Arc::new(tls))
        .timeout(REQUEST_TIMEOUT)
        .build())
}

fn post_json<Response: serde::de::DeserializeOwned>(
    endpoint: &NodeEndpoint,
    client_identity: Option<&CertificateMaterial>,
    path: &str,
    body: &impl serde::Serialize,
) -> Result<Response, NodeClientError> {
    let agent = http_agent(endpoint, client_identity)?;
    let payload =
        serde_json::to_string(body).map_err(|source| NodeClientError::Serialize { source })?;
    let mut last_error = NodeClientError::NoAddresses;

    for url in endpoint.candidate_urls(path) {
        match agent
            .post(&url)
            .set("content-type", "application/json")
            .send_string(&payload)
        {
            Ok(response) => {
                let text = response
                    .into_string()
                    .map_err(|source| NodeClientError::Io { source })?;
                return serde_json::from_str(&text)
                    .map_err(|source| NodeClientError::Parse { source });
            }
            Err(error) => last_error = NodeClientError::Request(Box::new(error)),
        }
    }

    Err(last_error)
}

pub fn get_json<Response: serde::de::DeserializeOwned>(
    endpoint: &NodeEndpoint,
    client_identity: Option<&CertificateMaterial>,
    path: &str,
) -> Result<Response, NodeClientError> {
    let agent = http_agent(endpoint, client_identity)?;
    let mut last_error = NodeClientError::NoAddresses;

    for url in endpoint.candidate_urls(path) {
        match agent.get(&url).call() {
            Ok(response) => {
                let text = response
                    .into_string()
                    .map_err(|source| NodeClientError::Io { source })?;
                return serde_json::from_str(&text)
                    .map_err(|source| NodeClientError::Parse { source });
            }
            Err(error) => last_error = NodeClientError::Request(Box::new(error)),
        }
    }

    Err(last_error)
}

/// Sends a screen command to a paired node over mTLS (host identity
/// required by the node after pairing).
pub fn post_screen_command(
    endpoint: &NodeEndpoint,
    client_identity: Option<&CertificateMaterial>,
    request: &ScreenCommandRequest,
) -> Result<ScreenStatusResponse, NodeClientError> {
    post_json(endpoint, client_identity, SCREEN_PATH, request)
}

/// Sends a disk command to a paired node over mTLS.
pub fn post_disk_command(
    endpoint: &NodeEndpoint,
    client_identity: Option<&CertificateMaterial>,
    request: &DiskCommandRequest,
) -> Result<DiskStatusResponse, NodeClientError> {
    post_json(endpoint, client_identity, DISK_PATH, request)
}

pub fn get_disk_status(
    endpoint: &NodeEndpoint,
    client_identity: Option<&CertificateMaterial>,
) -> Result<DiskStatusResponse, NodeClientError> {
    get_json(endpoint, client_identity, DISK_PATH)
}

/// Submits the pairing request to the node. The connection is pinned to the
/// fingerprint shown in the node's advertisement/QR, and no client cert is
/// presented (the node only requires mTLS after pairing).
pub fn submit_pairing(
    endpoint: &NodeEndpoint,
    request: &PairingRequest,
) -> Result<PairingResponse, NodeClientError> {
    let response: PairingResponse = post_json(endpoint, None, PAIRING_PATH, request)?;

    if response.accepted
        && !response.node_certificate_fingerprint.trim().is_empty()
        && response.node_certificate_fingerprint.to_ascii_uppercase()
            != endpoint.certificate_fingerprint.trim().to_ascii_uppercase()
    {
        return Err(NodeClientError::FingerprintMismatch);
    }

    Ok(response)
}

#[derive(Debug)]
pub enum NodeClientError {
    Tls(rustls::Error),
    InvalidHostCertificate,
    NoAddresses,
    Request(Box<ureq::Error>),
    Io { source: std::io::Error },
    Serialize { source: serde_json::Error },
    Parse { source: serde_json::Error },
    FingerprintMismatch,
}

impl std::fmt::Display for NodeClientError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Tls(_) => write!(formatter, "could not prepare a secure node connection"),
            Self::InvalidHostCertificate => {
                write!(formatter, "the host certificate could not be used")
            }
            Self::NoAddresses => {
                write!(formatter, "the node did not advertise a reachable address")
            }
            Self::Request(_) => write!(formatter, "could not reach the node"),
            Self::Io { .. } => write!(formatter, "the node connection was interrupted"),
            Self::Serialize { .. } => write!(formatter, "could not encode the node request"),
            Self::Parse { .. } => write!(formatter, "the node sent an unexpected response"),
            Self::FingerprintMismatch => write!(
                formatter,
                "the node's certificate did not match its advertised identity"
            ),
        }
    }
}

impl std::error::Error for NodeClientError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Tls(source) => Some(source),
            Self::Request(source) => Some(source),
            Self::Io { source } => Some(source),
            Self::Serialize { source } | Self::Parse { source } => Some(source),
            Self::InvalidHostCertificate | Self::NoAddresses | Self::FingerprintMismatch => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use super::*;

    fn test_certificate(name: &str) -> CertificateMaterial {
        let root = std::env::temp_dir().join(format!(
            "secondwind-node-client-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let certificate = sw_core::load_or_create_certificate(
            root.join("cert.pem"),
            root.join("key.pem"),
            name,
        )
        .expect("certificate");
        let _ = std::fs::remove_dir_all(&root);
        certificate
    }

    #[test]
    fn pinned_config_builds_without_client_identity() {
        let node = test_certificate("node");

        let config = pinned_client_config(&node.fingerprint, None).expect("pinned config");

        assert!(config.client_auth_cert_resolver.has_certs() == false);
    }

    #[test]
    fn pinned_config_builds_with_client_identity_for_mtls() {
        let node = test_certificate("node");
        let host = test_certificate("host");

        let config = pinned_client_config(&node.fingerprint, Some(&host)).expect("mtls config");

        assert!(config.client_auth_cert_resolver.has_certs());
    }

    #[test]
    fn verifier_accepts_only_the_pinned_fingerprint() {
        let node = test_certificate("node");
        let other = test_certificate("other");
        let provider = crypto_provider().expect("provider");
        let verifier = PinnedFingerprintVerifier::new(&node.fingerprint, provider);

        let node_der =
            CertificateDer::from(certificate_der_from_pem(&node.certificate_pem).expect("der"));
        let other_der =
            CertificateDer::from(certificate_der_from_pem(&other.certificate_pem).expect("der"));

        assert!(verifier.matches(&node_der));
        assert!(!verifier.matches(&other_der));
    }

    #[test]
    fn candidate_urls_format_ipv4_and_ipv6() {
        let endpoint = NodeEndpoint {
            addresses: vec![
                IpAddr::V4(Ipv4Addr::LOCALHOST),
                IpAddr::V6(std::net::Ipv6Addr::LOCALHOST),
            ],
            api_port: 49152,
            certificate_fingerprint: "sha256:ABC".to_string(),
        };

        assert_eq!(
            endpoint.candidate_urls("/v1/pairing"),
            vec![
                "https://127.0.0.1:49152/v1/pairing".to_string(),
                "https://[::1]:49152/v1/pairing".to_string(),
            ]
        );
    }

    #[test]
    fn no_addresses_is_a_clear_error() {
        let endpoint = NodeEndpoint {
            addresses: Vec::new(),
            api_port: 49152,
            certificate_fingerprint: "sha256:ABC".to_string(),
        };

        let error = get_json::<serde_json::Value>(&endpoint, None, "/v1/health")
            .expect_err("no addresses");

        assert!(matches!(error, NodeClientError::NoAddresses));
    }
}
