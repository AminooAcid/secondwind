use serde::{Deserialize, Serialize};

use crate::config::NodeUuid;

pub const PAIRING_QR_PAYLOAD_VERSION: u16 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairingPin(String);

impl PairingPin {
    pub fn new(value: impl Into<String>) -> Result<Self, PairingError> {
        let value = value.into();
        if value.len() != 6 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(PairingError::InvalidPin);
        }

        Ok(Self(value))
    }

    pub fn expose_for_pairing_display(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PairingError {
    InvalidPin,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairingOffer {
    pub node_uuid: NodeUuid,
    pub node_name: String,
    pub certificate_fingerprint: String,
    pub pin: PairingPin,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairingQrPayload {
    pub schema_version: u16,
    pub node_uuid: NodeUuid,
    pub node_name: String,
    pub certificate_fingerprint: String,
    pub pin: PairingPin,
}

impl PairingQrPayload {
    pub fn from_offer(offer: &PairingOffer) -> Self {
        Self {
            schema_version: PAIRING_QR_PAYLOAD_VERSION,
            node_uuid: offer.node_uuid,
            node_name: offer.node_name.clone(),
            certificate_fingerprint: offer.certificate_fingerprint.clone(),
            pin: offer.pin.clone(),
        }
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairingRequest {
    pub host_name: String,
    pub host_certificate_fingerprint: String,
    pub host_certificate_pem: String,
    pub pin: PairingPin,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairingResponse {
    pub accepted: bool,
    pub node_certificate_fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairingStatusResponse {
    pub status: PairingStatus,
    pub offer: Option<PairingOffer>,
    pub qr_payload: Option<PairingQrPayload>,
    pub paired_host_name: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PairingStatus {
    Unavailable,
    WaitingForHost,
    Paired,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_offer() -> PairingOffer {
        PairingOffer {
            node_uuid: NodeUuid::new("00000000-0000-4000-8000-000000000004").expect("valid uuid"),
            node_name: "node".to_string(),
            certificate_fingerprint: "sha256:fingerprint".to_string(),
            pin: PairingPin::new("123456").expect("valid pin"),
        }
    }

    #[test]
    fn pairing_pin_accepts_six_digits() {
        let pin = PairingPin::new("123456").expect("valid pin");

        assert_eq!(pin.expose_for_pairing_display(), "123456");
    }

    #[test]
    fn pairing_pin_rejects_non_six_digit_values() {
        assert_eq!(PairingPin::new("12345"), Err(PairingError::InvalidPin));
        assert_eq!(PairingPin::new("1234567"), Err(PairingError::InvalidPin));
        assert_eq!(PairingPin::new("12a456"), Err(PairingError::InvalidPin));
    }

    #[test]
    fn pairing_request_carries_host_certificate_material() {
        let request = PairingRequest {
            host_name: "host".to_string(),
            host_certificate_fingerprint: "sha256:host".to_string(),
            host_certificate_pem: "-----BEGIN CERTIFICATE-----\n-----END CERTIFICATE-----"
                .to_string(),
            pin: PairingPin::new("123456").expect("valid pin"),
        };

        assert_eq!(request.host_certificate_fingerprint, "sha256:host");
        assert!(request.host_certificate_pem.contains("BEGIN CERTIFICATE"));
    }

    #[test]
    fn qr_payload_is_derived_from_offer() {
        let offer = test_offer();
        let payload = PairingQrPayload::from_offer(&offer);

        assert_eq!(payload.schema_version, PAIRING_QR_PAYLOAD_VERSION);
        assert_eq!(payload.node_uuid, offer.node_uuid);
        assert_eq!(payload.node_name, offer.node_name);
        assert_eq!(
            payload.certificate_fingerprint,
            offer.certificate_fingerprint
        );
        assert_eq!(payload.pin, offer.pin);
    }

    #[test]
    fn qr_payload_serializes_to_json() {
        let payload = PairingQrPayload::from_offer(&test_offer());
        let json = payload.to_json().expect("serialize qr payload");

        assert!(json.contains("schema_version"));
        assert!(json.contains("00000000-0000-4000-8000-000000000004"));
        assert!(json.contains("sha256:fingerprint"));
    }
}
