use serde::{Deserialize, Serialize};

use crate::config::NodeUuid;

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
pub struct PairingRequest {
    pub host_name: String,
    pub host_certificate_fingerprint: String,
    pub pin: PairingPin,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairingResponse {
    pub accepted: bool,
    pub node_certificate_fingerprint: String,
}

#[cfg(test)]
mod tests {
    use super::*;

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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairingStatusResponse {
    pub status: PairingStatus,
    pub offer: Option<PairingOffer>,
    pub paired_host_name: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PairingStatus {
    Unavailable,
    WaitingForHost,
    Paired,
}
