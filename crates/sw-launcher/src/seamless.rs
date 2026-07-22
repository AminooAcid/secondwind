//! Seamless-window client invocation (xpra attach) — pure builders.
//!
//! The host attaches an xpra client to the node's app session so node-side
//! windows appear as native windows on the host desktop. The client program
//! name and every connection value are runtime inputs; nothing is baked in.

use std::net::IpAddr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeamlessAttachSpec {
    pub program: String,
    pub args: Vec<String>,
}

/// Builds the attach invocation for the node's app session. The password
/// travels via env-style argument file semantics of xpra's `--password-file`;
/// callers write it to a private temp file and pass its path.
pub fn attach_spec(
    client_program: &str,
    address: &IpAddr,
    port: u16,
    password_file: &str,
) -> SeamlessAttachSpec {
    let host = match address {
        IpAddr::V4(v4) => v4.to_string(),
        IpAddr::V6(v6) => format!("[{v6}]"),
    };

    SeamlessAttachSpec {
        program: client_program.to_string(),
        args: vec![
            "attach".to_string(),
            format!("tcp://{host}:{port}/"),
            format!("--password-file={password_file}"),
            // Reconnect quietly if the link blips; never show upstream UI.
            "--reconnect=yes".to_string(),
            "--notifications=no".to_string(),
            "--splash=no".to_string(),
        ],
    }
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use super::*;

    #[test]
    fn attach_spec_uses_runtime_values_only() {
        let spec = attach_spec(
            "xpra",
            &IpAddr::V4(Ipv4Addr::new(10, 0, 0, 7)),
            14500,
            "C:/tmp/pass",
        );

        assert_eq!(spec.program, "xpra");
        assert_eq!(spec.args[0], "attach");
        assert_eq!(spec.args[1], "tcp://10.0.0.7:14500/");
        assert!(
            spec.args
                .contains(&"--password-file=C:/tmp/pass".to_string())
        );
    }

    #[test]
    fn ipv6_addresses_are_bracketed() {
        let spec = attach_spec("xpra", &"::1".parse().expect("ipv6"), 14500, "/tmp/pass");

        assert_eq!(spec.args[1], "tcp://[::1]:14500/");
    }
}
