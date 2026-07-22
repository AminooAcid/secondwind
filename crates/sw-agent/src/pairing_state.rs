use sw_core::{
    PairingOffer, PairingPin, PairingRequest, PairingResponse, PairingStatus, PairingStatusResponse,
};

#[derive(Debug, Clone)]
pub enum PairingState {
    Unavailable { reason: String },
    Waiting { offer: PairingOffer },
    Paired { host_name: String },
}

impl PairingState {
    pub fn status_response(&self) -> PairingStatusResponse {
        match self {
            Self::Unavailable { reason } => PairingStatusResponse {
                status: PairingStatus::Unavailable,
                offer: None,
                paired_host_name: None,
                message: Some(reason.clone()),
            },
            Self::Waiting { offer } => PairingStatusResponse {
                status: PairingStatus::WaitingForHost,
                offer: Some(offer.clone()),
                paired_host_name: None,
                message: None,
            },
            Self::Paired { host_name } => PairingStatusResponse {
                status: PairingStatus::Paired,
                offer: None,
                paired_host_name: Some(host_name.clone()),
                message: None,
            },
        }
    }

    pub fn submit_request(&mut self, request: PairingRequest) -> PairingResponse {
        let Self::Waiting { offer } = self else {
            return PairingResponse {
                accepted: false,
                node_certificate_fingerprint: String::new(),
            };
        };

        if request.pin != offer.pin {
            return PairingResponse {
                accepted: false,
                node_certificate_fingerprint: offer.certificate_fingerprint.clone(),
            };
        }

        let node_certificate_fingerprint = offer.certificate_fingerprint.clone();
        *self = Self::Paired {
            host_name: request.host_name,
        };

        PairingResponse {
            accepted: true,
            node_certificate_fingerprint,
        }
    }
}

pub fn unavailable_pairing() -> PairingState {
    PairingState::Unavailable {
        reason: "node certificate is not configured yet".to_string(),
    }
}

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
    fn waiting_status_exposes_offer() {
        let status = waiting_state().status_response();

        assert_eq!(status.status, PairingStatus::WaitingForHost);
        assert!(status.offer.is_some());
    }

    #[test]
    fn matching_pin_accepts_and_marks_paired() {
        let mut state = waiting_state();

        let response = state.submit_request(PairingRequest {
            host_name: "host".to_string(),
            host_certificate_fingerprint: "host-fingerprint".to_string(),
            pin: fixed_pin_for_tests(),
        });

        assert!(response.accepted);
        assert!(matches!(state, PairingState::Paired { .. }));
        assert_eq!(
            state.status_response().paired_host_name.as_deref(),
            Some("host")
        );
    }

    #[test]
    fn mismatched_pin_rejects() {
        let mut state = waiting_state();

        let response = state.submit_request(PairingRequest {
            host_name: "host".to_string(),
            host_certificate_fingerprint: "host-fingerprint".to_string(),
            pin: PairingPin::new("654321").expect("valid pin"),
        });

        assert!(!response.accepted);
        assert!(matches!(state, PairingState::Waiting { .. }));
    }
}
