//! Pure screen rendering: kiosk state → centered lines of text.
//!
//! Rendering is terminal-agnostic (a `Vec<String>`) so every screen is unit
//! testable; the binary paints the lines with crossterm.

use qrcode::{QrCode, render::unicode};
use sw_core::KioskState;

/// A rendered screen: lines to center vertically and horizontally.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Screen {
    pub lines: Vec<String>,
}

pub fn render(state: &KioskState) -> Screen {
    match state {
        KioskState::Starting => Screen {
            lines: vec![
                "SecondWind".to_string(),
                String::new(),
                "Starting up…".to_string(),
            ],
        },
        KioskState::Unpaired {
            node_name,
            pin,
            qr_payload,
            certificate_fingerprint,
        } => {
            let mut lines = vec![
                "SecondWind".to_string(),
                String::new(),
                format!("This node: {node_name}"),
                String::new(),
                "Open the SecondWind app on your PC and".to_string(),
                "enter this PIN when asked:".to_string(),
                String::new(),
                spaced_pin(pin),
                String::new(),
            ];

            if let Some(qr_lines) = qr_lines(qr_payload) {
                lines.push("Or scan:".to_string());
                lines.push(String::new());
                lines.extend(qr_lines);
                lines.push(String::new());
            }

            lines.push(format!("Security code: {}", short_fingerprint(certificate_fingerprint)));
            Screen { lines }
        }
        KioskState::Idle {
            node_name,
            paired_host_name,
        } => Screen {
            lines: vec![
                "SecondWind".to_string(),
                String::new(),
                format!("This node: {node_name}"),
                format!("Paired with: {paired_host_name}"),
                String::new(),
                "Waiting for your PC…".to_string(),
                "The screen will start automatically.".to_string(),
            ],
        },
        KioskState::Streaming {
            paired_host_name, ..
        } => Screen {
            lines: vec![
                "SecondWind".to_string(),
                String::new(),
                format!("Connecting your extra screen to {paired_host_name}…"),
            ],
        },
    }
}

/// "123456" → "1 2 3 4 5 6" so the PIN is readable across the room.
fn spaced_pin(pin: &str) -> String {
    pin.chars()
        .map(|digit| digit.to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

fn short_fingerprint(fingerprint: &str) -> String {
    let hex = fingerprint.strip_prefix("sha256:").unwrap_or(fingerprint);
    if hex.len() >= 8 {
        format!("{}…{}", &hex[..4], &hex[hex.len() - 4..])
    } else {
        hex.to_string()
    }
}

fn qr_lines(payload: &str) -> Option<Vec<String>> {
    let code = QrCode::new(payload.as_bytes()).ok()?;
    let rendered = code
        .render::<unicode::Dense1x2>()
        .quiet_zone(true)
        .build();
    Some(rendered.lines().map(|line| line.to_string()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unpaired_screen_shows_pin_and_qr_without_upstream_names() {
        let screen = render(&KioskState::Unpaired {
            node_name: "Living-room laptop".to_string(),
            pin: "123456".to_string(),
            qr_payload: "{\"schema_version\":1}".to_string(),
            certificate_fingerprint: "sha256:AABBCCDDEEFF0011".to_string(),
        });

        let text = screen.lines.join("\n");
        assert!(text.contains("1 2 3 4 5 6"));
        assert!(text.contains("Living-room laptop"));
        assert!(text.contains("AABB…0011"));
        // QR block characters present.
        assert!(text.contains('█') || text.contains('▀') || text.contains('▄'));
        // Product boundary: no upstream tool names on user screens.
        for upstream in ["moonlight", "apollo", "sunshine", "debian", "cage"] {
            assert!(
                !text.to_lowercase().contains(upstream),
                "screen must not mention {upstream}"
            );
        }
    }

    #[test]
    fn idle_screen_names_both_sides() {
        let screen = render(&KioskState::Idle {
            node_name: "node".to_string(),
            paired_host_name: "My PC".to_string(),
        });

        let text = screen.lines.join("\n");
        assert!(text.contains("node"));
        assert!(text.contains("My PC"));
        assert!(text.contains("Waiting for your PC"));
    }

    #[test]
    fn streaming_screen_is_a_brief_transition_notice() {
        let screen = render(&KioskState::Streaming {
            paired_host_name: "My PC".to_string(),
            host_address: "peer".to_string(),
            stream_pair_pin: None,
        });

        assert!(screen.lines.join("\n").contains("My PC"));
    }

    #[test]
    fn broken_qr_payload_still_renders_pin_screen() {
        let screen = render(&KioskState::Unpaired {
            node_name: "node".to_string(),
            pin: "123456".to_string(),
            qr_payload: String::new(),
            certificate_fingerprint: "sha256:AB".to_string(),
        });

        assert!(screen.lines.join("\n").contains("1 2 3 4 5 6"));
    }
}
