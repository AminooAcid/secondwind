//! Shared self-signed certificate store used by both peers.
//!
//! The node agent and the host companion both persist a self-signed
//! certificate + private key and exchange fingerprints during pairing, so the
//! storage, generation, and fingerprinting logic lives in `sw-core`.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use base64::{Engine, engine::general_purpose::STANDARD};
use rcgen::generate_simple_self_signed;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CertificateMaterial {
    pub certificate_pem: String,
    pub private_key_pem: String,
    pub fingerprint: String,
}

pub fn load_or_create_certificate(
    certificate_file: impl AsRef<Path>,
    private_key_file: impl AsRef<Path>,
    subject_name: impl Into<String>,
) -> Result<CertificateMaterial, CertificateStoreError> {
    let certificate_file = certificate_file.as_ref();
    let private_key_file = private_key_file.as_ref();
    let subject_name = subject_name.into();

    match (
        fs::read_to_string(certificate_file),
        fs::read_to_string(private_key_file),
    ) {
        (Ok(certificate_pem), Ok(private_key_pem)) => {
            match certificate_fingerprint(&certificate_pem) {
                Ok(fingerprint) => Ok(CertificateMaterial {
                    certificate_pem,
                    private_key_pem,
                    fingerprint,
                }),
                Err(CertificateStoreError::InvalidPem) => {
                    regenerate_certificate_files(certificate_file, private_key_file, subject_name)
                }
                Err(error) => Err(error),
            }
        }
        (Err(cert_error), Err(key_error))
            if cert_error.kind() == io::ErrorKind::NotFound
                && key_error.kind() == io::ErrorKind::NotFound =>
        {
            regenerate_certificate_files(certificate_file, private_key_file, subject_name)
        }
        (Err(error), _) if error.kind() == io::ErrorKind::NotFound => {
            regenerate_certificate_files(certificate_file, private_key_file, subject_name)
        }
        (_, Err(error)) if error.kind() == io::ErrorKind::NotFound => {
            regenerate_certificate_files(certificate_file, private_key_file, subject_name)
        }
        (Err(source), _) => Err(CertificateStoreError::Read {
            path: certificate_file.to_path_buf(),
            source,
        }),
        (_, Err(source)) => Err(CertificateStoreError::Read {
            path: private_key_file.to_path_buf(),
            source,
        }),
    }
}

pub fn certificate_fingerprint(certificate_pem: &str) -> Result<String, CertificateStoreError> {
    let certificate_der = certificate_der_from_pem(certificate_pem)?;
    Ok(fingerprint_of_der(&certificate_der))
}

pub fn fingerprint_of_der(certificate_der: &[u8]) -> String {
    let digest = Sha256::digest(certificate_der);
    format!("sha256:{}", hex::encode_upper(digest))
}

fn regenerate_certificate_files(
    certificate_file: &Path,
    private_key_file: &Path,
    subject_name: String,
) -> Result<CertificateMaterial, CertificateStoreError> {
    let generated = generate_certificate(subject_name)?;
    write_certificate_files(certificate_file, private_key_file, &generated)?;
    Ok(generated)
}

fn generate_certificate(
    subject_name: String,
) -> Result<CertificateMaterial, CertificateStoreError> {
    let certified = generate_simple_self_signed(vec![subject_name]).map_err(|source| {
        CertificateStoreError::Generate {
            source: source.to_string(),
        }
    })?;
    let certificate_pem = certified.cert.pem();
    let private_key_pem = certified.key_pair.serialize_pem();
    let fingerprint = certificate_fingerprint(&certificate_pem)?;

    Ok(CertificateMaterial {
        certificate_pem,
        private_key_pem,
        fingerprint,
    })
}

fn write_certificate_files(
    certificate_file: &Path,
    private_key_file: &Path,
    certificate: &CertificateMaterial,
) -> Result<(), CertificateStoreError> {
    create_parent(certificate_file)?;
    create_parent(private_key_file)?;
    fs::write(certificate_file, &certificate.certificate_pem).map_err(|source| {
        CertificateStoreError::Write {
            path: certificate_file.to_path_buf(),
            source,
        }
    })?;
    fs::write(private_key_file, &certificate.private_key_pem).map_err(|source| {
        CertificateStoreError::Write {
            path: private_key_file.to_path_buf(),
            source,
        }
    })
}

fn create_parent(path: &Path) -> Result<(), CertificateStoreError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| CertificateStoreError::Write {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    Ok(())
}

pub fn certificate_der_from_pem(certificate_pem: &str) -> Result<Vec<u8>, CertificateStoreError> {
    let mut in_certificate = false;
    let mut encoded = String::new();

    for line in certificate_pem.lines() {
        match line.trim() {
            "-----BEGIN CERTIFICATE-----" => in_certificate = true,
            "-----END CERTIFICATE-----" => break,
            value if in_certificate => encoded.push_str(value),
            _ => {}
        }
    }

    if encoded.is_empty() {
        return Err(CertificateStoreError::InvalidPem);
    }

    STANDARD
        .decode(encoded)
        .map_err(|_| CertificateStoreError::InvalidPem)
}

#[derive(Debug)]
pub enum CertificateStoreError {
    Read { path: PathBuf, source: io::Error },
    Write { path: PathBuf, source: io::Error },
    Generate { source: String },
    InvalidPem,
}

impl std::fmt::Display for CertificateStoreError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read { path, .. } => {
                write!(
                    formatter,
                    "failed to read certificate file {}",
                    path.display()
                )
            }
            Self::Write { path, .. } => {
                write!(
                    formatter,
                    "failed to write certificate file {}",
                    path.display()
                )
            }
            Self::Generate { .. } => write!(formatter, "failed to generate certificate"),
            Self::InvalidPem => write!(formatter, "certificate file is not valid PEM"),
        }
    }
}

impl std::error::Error for CertificateStoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read { source, .. } | Self::Write { source, .. } => Some(source),
            Self::Generate { .. } | Self::InvalidPem => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempFileSet {
        certificate_file: PathBuf,
        private_key_file: PathBuf,
    }

    impl TempFileSet {
        fn new(name: &str) -> Self {
            let root = std::env::temp_dir().join(format!(
                "secondwind-certificate-{}-{name}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&root);

            Self {
                certificate_file: root.join("peer-cert.pem"),
                private_key_file: root.join("peer-key.pem"),
            }
        }
    }

    impl Drop for TempFileSet {
        fn drop(&mut self) {
            if let Some(root) = self.certificate_file.parent() {
                let _ = fs::remove_dir_all(root);
            }
        }
    }

    #[test]
    fn creates_certificate_files_when_missing() {
        let files = TempFileSet::new("creates");

        let certificate = load_or_create_certificate(
            &files.certificate_file,
            &files.private_key_file,
            "peer-uuid",
        )
        .expect("create certificate");

        assert!(files.certificate_file.exists());
        assert!(files.private_key_file.exists());
        assert!(certificate.fingerprint.starts_with("sha256:"));
        assert_eq!(certificate.fingerprint.len(), 71);
    }

    #[test]
    fn keeps_existing_certificate_fingerprint() {
        let files = TempFileSet::new("keeps");
        let original = load_or_create_certificate(
            &files.certificate_file,
            &files.private_key_file,
            "peer-uuid",
        )
        .expect("create certificate");

        let loaded = load_or_create_certificate(
            &files.certificate_file,
            &files.private_key_file,
            "different-peer-uuid",
        )
        .expect("load certificate");

        assert_eq!(loaded.fingerprint, original.fingerprint);
    }

    #[test]
    fn regenerates_incomplete_certificate_store() {
        let files = TempFileSet::new("incomplete");
        fs::create_dir_all(files.certificate_file.parent().expect("root"))
            .expect("create temp dir");
        fs::write(&files.certificate_file, "orphaned cert").expect("write cert");

        let certificate = load_or_create_certificate(
            &files.certificate_file,
            &files.private_key_file,
            "peer-uuid",
        )
        .expect("self-heal incomplete store");

        assert!(files.certificate_file.exists());
        assert!(files.private_key_file.exists());
        assert!(certificate.fingerprint.starts_with("sha256:"));
    }

    #[test]
    fn regenerates_invalid_certificate_pem() {
        let files = TempFileSet::new("invalid-pem");
        fs::create_dir_all(files.certificate_file.parent().expect("root"))
            .expect("create temp dir");
        fs::write(&files.certificate_file, "not pem").expect("write cert");
        fs::write(&files.private_key_file, "key exists").expect("write key");

        let certificate = load_or_create_certificate(
            &files.certificate_file,
            &files.private_key_file,
            "peer-uuid",
        )
        .expect("self-heal invalid cert");

        assert!(certificate.fingerprint.starts_with("sha256:"));
    }

    #[test]
    fn der_fingerprint_matches_pem_fingerprint() {
        let files = TempFileSet::new("der-fingerprint");
        let certificate = load_or_create_certificate(
            &files.certificate_file,
            &files.private_key_file,
            "peer-uuid",
        )
        .expect("create certificate");

        let der = certificate_der_from_pem(&certificate.certificate_pem).expect("der");

        assert_eq!(fingerprint_of_der(&der), certificate.fingerprint);
    }
}
