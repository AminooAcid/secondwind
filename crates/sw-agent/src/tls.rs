use std::{io::Cursor, sync::Arc};

use axum_server::tls_rustls::RustlsConfig;
use rustls::{RootCertStore, ServerConfig, server::WebPkiClientVerifier};
use rustls_pemfile::private_key;

use sw_core::certificates::{
    CertificateMaterial, CertificateStoreError, certificate_der_from_pem,
};

use crate::identity::PairedHostTrust;

pub fn agent_tls_config(
    certificate: &CertificateMaterial,
    paired_host: Option<&PairedHostTrust>,
) -> Result<RustlsConfig, TlsConfigError> {
    let node_cert = rustls::pki_types::CertificateDer::from(certificate_der_from_pem(
        &certificate.certificate_pem,
    )?);
    let node_key = private_key_from_pem(&certificate.private_key_pem)?;

    let mut config = match paired_host {
        Some(host) => ServerConfig::builder()
            .with_client_cert_verifier(host_client_verifier(host)?)
            .with_single_cert(vec![node_cert], node_key)
            .map_err(TlsConfigError::Rustls)?,
        None => ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![node_cert], node_key)
            .map_err(TlsConfigError::Rustls)?,
    };
    config.alpn_protocols = vec![b"http/1.1".to_vec()];

    Ok(RustlsConfig::from_config(Arc::new(config)))
}

fn host_client_verifier(
    paired_host: &PairedHostTrust,
) -> Result<Arc<dyn rustls::server::danger::ClientCertVerifier>, TlsConfigError> {
    let host_certificate_pem = paired_host
        .host_certificate_pem
        .as_ref()
        .ok_or(TlsConfigError::MissingTrustedHostCertificate)?;
    let host_cert =
        rustls::pki_types::CertificateDer::from(certificate_der_from_pem(host_certificate_pem)?);
    let mut roots = RootCertStore::empty();
    roots.add(host_cert).map_err(TlsConfigError::TrustAnchor)?;

    WebPkiClientVerifier::builder(Arc::new(roots))
        .build()
        .map_err(TlsConfigError::ClientVerifier)
}

fn private_key_from_pem(
    private_key_pem: &str,
) -> Result<rustls::pki_types::PrivateKeyDer<'static>, TlsConfigError> {
    private_key(&mut Cursor::new(private_key_pem.as_bytes()))
        .map_err(TlsConfigError::PrivateKeyRead)?
        .ok_or(TlsConfigError::MissingPrivateKey)
}

#[derive(Debug)]
pub enum TlsConfigError {
    Certificate(CertificateStoreError),
    PrivateKeyRead(std::io::Error),
    MissingPrivateKey,
    MissingTrustedHostCertificate,
    TrustAnchor(rustls::Error),
    ClientVerifier(rustls::server::VerifierBuilderError),
    Rustls(rustls::Error),
}

impl From<CertificateStoreError> for TlsConfigError {
    fn from(source: CertificateStoreError) -> Self {
        Self::Certificate(source)
    }
}

impl std::fmt::Display for TlsConfigError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Certificate(_) => write!(formatter, "failed to parse TLS certificate"),
            Self::PrivateKeyRead(_) => write!(formatter, "failed to read TLS private key"),
            Self::MissingPrivateKey => write!(formatter, "missing TLS private key"),
            Self::MissingTrustedHostCertificate => {
                write!(
                    formatter,
                    "paired host trust is missing certificate material"
                )
            }
            Self::TrustAnchor(_) => write!(formatter, "failed to trust paired host certificate"),
            Self::ClientVerifier(_) => {
                write!(formatter, "failed to build client certificate verifier")
            }
            Self::Rustls(_) => write!(formatter, "failed to build TLS server configuration"),
        }
    }
}

impl std::error::Error for TlsConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Certificate(source) => Some(source),
            Self::PrivateKeyRead(source) => Some(source),
            Self::TrustAnchor(source) => Some(source),
            Self::ClientVerifier(source) => Some(source),
            Self::Rustls(source) => Some(source),
            Self::MissingPrivateKey | Self::MissingTrustedHostCertificate => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use super::*;
    use sw_core::certificates::load_or_create_certificate;

    struct TempCertificateFiles {
        root: PathBuf,
    }

    impl TempCertificateFiles {
        fn new(name: &str) -> Self {
            let root =
                std::env::temp_dir().join(format!("secondwind-tls-{}-{name}", std::process::id()));
            let _ = fs::remove_dir_all(&root);
            Self { root }
        }

        fn certificate(&self, name: &str) -> CertificateMaterial {
            load_or_create_certificate(
                self.root.join(format!("{name}.pem")),
                self.root.join(format!("{name}.key")),
                name,
            )
            .expect("certificate")
        }
    }

    impl Drop for TempCertificateFiles {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn builds_tls_config_without_client_auth_before_pairing() {
        let files = TempCertificateFiles::new("unpaired");
        let node = files.certificate("node");

        let _config = agent_tls_config(&node, None).expect("tls config");
    }

    #[test]
    fn builds_mtls_config_with_paired_host_certificate() {
        let files = TempCertificateFiles::new("paired");
        let node = files.certificate("node");
        let host = files.certificate("host");

        let _config = agent_tls_config(
            &node,
            Some(&PairedHostTrust {
                host_name: "host".to_string(),
                host_certificate_fingerprint: host.fingerprint,
                host_certificate_pem: Some(host.certificate_pem),
            }),
        )
        .expect("mtls config");
    }

    #[test]
    fn paired_mtls_config_requires_host_certificate_material() {
        let files = TempCertificateFiles::new("missing-host-cert");
        let node = files.certificate("node");

        let error = agent_tls_config(
            &node,
            Some(&PairedHostTrust {
                host_name: "host".to_string(),
                host_certificate_fingerprint: "sha256:host".to_string(),
                host_certificate_pem: None,
            }),
        )
        .expect_err("missing host cert should fail");

        assert!(matches!(
            error,
            TlsConfigError::MissingTrustedHostCertificate
        ));
    }
}
