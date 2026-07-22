//! Wake-on-LAN: build and send magic packets.

use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};

/// Standard discard-port broadcast target for magic packets.
const WOL_PORT: u16 = 9;

/// Parses "aa:bb:cc:dd:ee:ff" (case-insensitive, `:` or `-` separated).
pub fn parse_mac(mac: &str) -> Option<[u8; 6]> {
    let parts: Vec<&str> = mac.split([':', '-']).collect();
    if parts.len() != 6 {
        return None;
    }

    let mut bytes = [0_u8; 6];
    for (index, part) in parts.iter().enumerate() {
        bytes[index] = u8::from_str_radix(part, 16).ok()?;
    }
    Some(bytes)
}

/// 6 × 0xFF then the MAC sixteen times.
pub fn magic_packet(mac: [u8; 6]) -> Vec<u8> {
    let mut packet = vec![0xFF_u8; 6];
    for _ in 0..16 {
        packet.extend_from_slice(&mac);
    }
    packet
}

/// Sends magic packets for every MAC to the broadcast address. Best-effort:
/// reports how many packets were sent.
pub fn send_wake(mac_addresses: &[String]) -> usize {
    let Ok(socket) = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)) else {
        return 0;
    };
    if socket.set_broadcast(true).is_err() {
        return 0;
    }
    let target = SocketAddrV4::new(Ipv4Addr::BROADCAST, WOL_PORT);

    let mut sent = 0;
    for mac in mac_addresses {
        if let Some(bytes) = parse_mac(mac) {
            if socket.send_to(&magic_packet(bytes), target).is_ok() {
                sent += 1;
            }
        }
    }
    sent
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_common_mac_formats() {
        assert_eq!(
            parse_mac("aa:bb:cc:dd:ee:ff"),
            Some([0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF])
        );
        assert_eq!(
            parse_mac("AA-BB-CC-DD-EE-01"),
            Some([0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0x01])
        );
        assert_eq!(parse_mac("not-a-mac"), None);
        assert_eq!(parse_mac("aa:bb:cc:dd:ee"), None);
    }

    #[test]
    fn magic_packet_is_ff_header_plus_sixteen_macs() {
        let mac = [1, 2, 3, 4, 5, 6];
        let packet = magic_packet(mac);

        assert_eq!(packet.len(), 6 + 16 * 6);
        assert!(packet[..6].iter().all(|byte| *byte == 0xFF));
        assert_eq!(&packet[6..12], &mac);
        assert_eq!(&packet[packet.len() - 6..], &mac);
    }
}
