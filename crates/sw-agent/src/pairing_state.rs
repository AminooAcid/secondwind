use crate::identity::PairedHostTrust;
use sw_core::{
    PairingOffer, PairingPin, PairingQrPayload, PairingRequest, PairingResponse, PairingStatus,
    PairingStatusResponse,
};

#[derive(Debug, Clone)]
pub enum PairingState {
    Unavailable { reason: String },
    Waiting { offer: PairingOffer },
    Paired { host_name: String },
}

impl PairingState {
    pub fn waiting(offer: PairingOffer) -> Self {
        Self::Waiting { offer }
    }

    pub fn paired(host_name: String) -> Self {
        Self::Paired { host_name }
    }

    pub fn status_response(&self) -> PairingStatusResponse {
        match self {
            Self::Unavailable { reason } => PairingStatusResponse {
                status: PairingStatus::Unavailable,
                offer: None,
                qr_payload: None,
                paired_host_name: None,
                message: Some(reason.clone()),
            },
            Self::Waiting { offer } => PairingStatusResponse {
                status: PairingStatus::WaitingForHost,
                offer: Some(offer.clone()),
                qr_payload: Some(PairingQrPayload::from_offer(offer)),
                paired_host_name: None,
                message: None,
            },
            Self::Paired { host_name } => PairingStatusResponse {
                status: PairingStatus::Paired,
                offer: None,
                qr_payload: None,
                paired_host_name: Some(host_name.clone()),
                message: None,
            },
        }
    }

    pub fn prepare_submit_request(&self, request: PairingRequest) -> PairingSubmitResult {
        let Self::Waiting { offer } = self else {
            return PairingSubmitResult {
                response: PairingResponse {
                    accepted: false,
                    node_certificate_fingerprint: String::new(),
                },
                paired_host: None,
            };
        };

        if request.pin != offer.pin {
            return PairingSubmitResult {
                response: PairingResponse {
                    accepted: false,
                    node_certificate_fingerprint: offer.certificate_fingerprint.clone(),
                },
                paired_host: None,
            };
        }

        PairingSubmitResult {
            response: PairingResponse {
                accepted: true,
                node_certificate_fingerprint: offer.certificate_fingerprint.clone(),
            },
            paired_host: Some(PairedHostTrust {
                host_name: request.host_name,
                host_certificate_fingerprint: request.host_certificate_fingerprint,
                host_certificate_pem: Some(request.host_certificate_pem),
            }),
        }
    }

    pub fn mark_paired(&mut self, host_name: String) {
        *self = Self::Paired { host_name };
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairingSubmitResult {
    pub response: PairingResponse,
    pub paired_host: Option<PairedHostTrust>,
}

pub fn unavailable_pairing() -> PairingState {
    PairingState::Unavailable {
        reason: "node certificate is not configured yet".to_string(),
    }
}

pub fn runtime_pairing_offer(
    node_uuid: sw_core::NodeUuid,
    node_name: String,
    certificate_fingerprint: String,
) -> Result<PairingState, PairingPinGenerationError> {
    Ok(PairingState::waiting(PairingOffer {
        node_uuid,
        node_name,
        certificate_fingerprint,
        pin: generate_pairing_pin()?,
    }))
}

pub fn generate_pairing_pin() -> Result<PairingPin, PairingPinGenerationError> {
    const PIN_SPACE: u64 = 1_000_000;
    let random_range = u32::MAX as u64 + 1;
    let unbiased_limit = random_range - (random_range % PIN_SPACE);

    loop {
        let mut bytes = [0_u8; 4];
        getrandom::getrandom(&mut bytes).map_err(PairingPinGenerationError::Random)?;
        let value = u32::from_le_bytes(bytes) as u64;
        if value < unbiased_limit {
            return PairingPin::new(format!("{:06}", value % PIN_SPACE))
                .map_err(|_| PairingPinGenerationError::InvalidGeneratedPin);
        }
    }
}

#[derive(Debug)]
pub enum PairingPinGenerationError {
    Random(getrandom::Error),
    InvalidGeneratedPin,
}

impl std::fmt::Display for PairingPinGenerationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Random(_) => write!(formatter, "failed to generate pairing PIN randomness"),
            Self::InvalidGeneratedPin => write!(formatter, "generated pairing PIN was invalid"),
        }
    }
}

impl std::error::Error for PairingPinGenerationError {}

pub fn fixed_pin_for_tests() -> PairingPin {
    PairingPin::new("123456").expect("fixed test pin is valid")
}

#[cfg(test)]
mod tests {
    use sw_core::NodeUuid;

    use super::*;

    fn waiting_state() -> PairingState {
        PairingState::Waiting {
            offer: PairingOffer {
                node_uuid: NodeUuid::new("00000000-0000-4000-8000-000000000004")
                    .expect("valid uuid"),
                node_name: "node".to_string(),
                certificate_fingerprint: "fingerprint".to_string(),
                pin: fixed_pin_for_tests(),
            },
        }
    }

    #[test]
    fn generated_pin_is_six_digits() {
        let pin = generate_pairing_pin().expect("generate pin");

        assert_eq!(pin.expose_for_pairing_display().len(), 6);
        assert!(
            pin.expose_for_pairing_display()
                .bytes()
                .all(|byte| byte.is_ascii_digit())
        );
    }

    #[test]
    fn runtime_pairing_offer_uses_runtime_values() {
        let node_uuid = NodeUuid::new("00000000-0000-4000-8000-000000000004").expect("valid uuid");
        let state = runtime_pairing_offer(
            node_uuid,
            "node".to_string(),
            "sha256:fingerprint".to_string(),
        )
        .expect("runtime offer");

        let PairingState::Waiting { offer } = state else {
            panic!("expected waiting state");
        };
        assert_eq!(offer.node_uuid, node_uuid);
        assert_eq!(offer.node_name, "node");
        assert_eq!(offer.certificate_fingerprint, "sha256:fingerprint");
    }

    #[test]
    fn waiting_status_exposes_offer() {
        let status = waiting_state().status_response();

        assert_eq!(status.status, PairingStatus::WaitingForHost);
        assert!(status.offer.is_some());
        assert!(status.qr_payload.is_some());
    }

    #[test]
    fn matching_pin_prepares_paired_host_trust_without_mutating() {
        let state = waiting_state();

        let result = state.prepare_submit_request(PairingRequest {
            host_name: "host".to_string(),
            host_certificate_fingerprint: "host-fingerprint".to_string(),
            host_certificate_pem: "host certificate".to_string(),
            pin: fixed_pin_for_tests(),
        });

        assert!(result.response.accepted);
        assert_eq!(
            result
                .paired_host
                .expect("paired host")
                .host_certificate_fingerprint,
            "host-fingerprint"
        );
        assert!(matches!(state, PairingState::Waiting { .. }));
    }

    #[test]
    fn mark_paired_updates_state_after_trust_is_persisted() {
        let mut state = waiting_state();

        state.mark_paired("host".to_string());

        assert!(matches!(state, PairingState::Paired { .. }));
        assert_eq!(
            state.status_response().paired_host_name.as_deref(),
            Some("host")
        );
    }

    #[test]
    fn mismatched_pin_rejects() {
        let state = waiting_state();

        let result = state.prepare_submit_request(PairingRequest {
            host_name: "host".to_string(),
            host_certificate_fingerprint: "host-fingerprint".to_string(),
            host_certificate_pem: "host certificate".to_string(),
            pin: PairingPin::new("654321").expect("valid pin"),
        });

        assert!(!result.response.accepted);
        assert!(result.paired_host.is_none());
        assert!(matches!(state, PairingState::Waiting { .. }));
    }
}
