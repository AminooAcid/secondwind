//! Node USB export (v0.4).
//!
//! `usbipd` runs on the node (image unit); the agent lists local devices
//! (`usbip list -l`), reports which are bound to the export driver, and
//! binds/unbinds via a root wrapper script reachable through a sudoers
//! rule scoped to exactly that script. Bus ids are validated before they
//! go anywhere near a privileged command.

use std::{fs, path::Path, process::Command, sync::Arc};

use sw_core::UsbDeviceInfo;

const USB_WRAPPER: &str = "/usr/local/lib/secondwind/secondwind-usb.sh";
const USBIP_HOST_DRIVER_DIR: &str = "/sys/bus/usb/drivers/usbip-host";

pub trait UsbController: Send + Sync + std::fmt::Debug {
    fn available(&self) -> bool;
    fn devices(&self) -> Vec<UsbDeviceInfo>;
    fn bind(&self, bus_id: &str) -> Result<(), UsbControlError>;
    fn unbind(&self, bus_id: &str) -> Result<(), UsbControlError>;
}

pub type SharedUsbController = Arc<dyn UsbController>;

/// A bus id like `1-1.4` or `usb3-2`. Conservative charset so it is safe
/// to hand to the privileged wrapper.
pub fn is_valid_bus_id(bus_id: &str) -> bool {
    !bus_id.is_empty()
        && bus_id.len() <= 32
        && bus_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
}

/// Parses `usbip list -l` output:
/// ```text
///  - busid 1-1 (0951:1666)
///    Kingston Technology DataTraveler 100 G3
/// ```
pub fn parse_usbip_list(output: &str, bound: &[String]) -> Vec<UsbDeviceInfo> {
    let mut devices = Vec::new();
    let mut current: Option<UsbDeviceInfo> = None;

    for line in output.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("- busid ") {
            if let Some(device) = current.take() {
                devices.push(device);
            }
            // "1-1 (0951:1666)"
            let mut parts = rest.split_whitespace();
            let bus_id = parts.next().unwrap_or_default().to_string();
            let ids = parts
                .next()
                .unwrap_or_default()
                .trim_matches(['(', ')'])
                .to_lowercase();
            let (vendor_id, product_id) = ids.split_once(':').unwrap_or(("", ""));
            if !is_valid_bus_id(&bus_id) {
                continue;
            }
            current = Some(UsbDeviceInfo {
                bound: bound.iter().any(|b| b == &bus_id),
                bus_id,
                vendor_id: vendor_id.to_string(),
                product_id: product_id.to_string(),
                description: String::new(),
            });
        } else if let Some(device) = current.as_mut() {
            if !trimmed.is_empty() && device.description.is_empty() {
                device.description = trimmed.to_string();
            }
        }
    }
    if let Some(device) = current.take() {
        devices.push(device);
    }
    devices
}

/// Reads which bus ids are bound to the export driver.
pub fn bound_bus_ids_in(driver_dir: impl AsRef<Path>) -> Vec<String> {
    let Ok(entries) = fs::read_dir(driver_dir) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().to_str().map(|name| name.to_string()))
        .filter(|name| is_valid_bus_id(name))
        .collect()
}

#[derive(Debug, Default)]
pub struct SysUsbController;

impl UsbController for SysUsbController {
    fn available(&self) -> bool {
        // usbip tooling present and the wrapper installed.
        Path::new(USB_WRAPPER).exists()
    }

    fn devices(&self) -> Vec<UsbDeviceInfo> {
        let output = Command::new("usbip")
            .args(["list", "-l"])
            .output()
            .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
            .unwrap_or_default();
        let bound = bound_bus_ids_in(USBIP_HOST_DRIVER_DIR);
        parse_usbip_list(&output, &bound)
    }

    fn bind(&self, bus_id: &str) -> Result<(), UsbControlError> {
        run_wrapper("bind", bus_id)
    }

    fn unbind(&self, bus_id: &str) -> Result<(), UsbControlError> {
        run_wrapper("unbind", bus_id)
    }
}

fn run_wrapper(verb: &str, bus_id: &str) -> Result<(), UsbControlError> {
    if !is_valid_bus_id(bus_id) {
        return Err(UsbControlError::InvalidBusId);
    }
    let status = Command::new("sudo")
        .args(["-n", USB_WRAPPER, verb, bus_id])
        .status()
        .map_err(|source| UsbControlError::Spawn { source })?;
    if status.success() {
        Ok(())
    } else {
        Err(UsbControlError::CommandFailed)
    }
}

#[derive(Debug)]
pub enum UsbControlError {
    InvalidBusId,
    Spawn { source: std::io::Error },
    CommandFailed,
}

impl std::fmt::Display for UsbControlError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidBusId => write!(formatter, "that USB device id is not valid"),
            Self::Spawn { .. } => write!(formatter, "could not run the USB helper"),
            Self::CommandFailed => write!(formatter, "the USB helper refused the request"),
        }
    }
}

impl std::error::Error for UsbControlError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Spawn { source } => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
pub mod test_support {
    use std::sync::Mutex;

    use super::*;

    #[derive(Debug, Default)]
    pub struct FakeUsbController {
        pub devices: Vec<UsbDeviceInfo>,
        pub bound: Mutex<Vec<String>>,
    }

    impl UsbController for FakeUsbController {
        fn available(&self) -> bool {
            true
        }

        fn devices(&self) -> Vec<UsbDeviceInfo> {
            let bound = self.bound.lock().expect("bound lock");
            self.devices
                .iter()
                .cloned()
                .map(|mut device| {
                    device.bound = bound.contains(&device.bus_id);
                    device
                })
                .collect()
        }

        fn bind(&self, bus_id: &str) -> Result<(), UsbControlError> {
            if !is_valid_bus_id(bus_id) {
                return Err(UsbControlError::InvalidBusId);
            }
            self.bound
                .lock()
                .expect("bound lock")
                .push(bus_id.to_string());
            Ok(())
        }

        fn unbind(&self, bus_id: &str) -> Result<(), UsbControlError> {
            self.bound
                .lock()
                .expect("bound lock")
                .retain(|bound| bound != bus_id);
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
 - busid 1-1 (0951:1666)
   Kingston Technology DataTraveler 100 G3

 - busid 1-1.4 (046d:c31c)
   Logitech, Inc. Keyboard K120
"#;

    #[test]
    fn parses_usbip_local_list() {
        let devices = parse_usbip_list(SAMPLE, &["1-1.4".to_string()]);

        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].bus_id, "1-1");
        assert_eq!(devices[0].vendor_id, "0951");
        assert_eq!(devices[0].product_id, "1666");
        assert!(devices[0].description.contains("Kingston"));
        assert!(!devices[0].bound);
        assert!(devices[1].bound);
    }

    #[test]
    fn bus_id_validation_rejects_shell_metacharacters() {
        assert!(is_valid_bus_id("1-1.4"));
        assert!(is_valid_bus_id("usb3-2"));
        assert!(!is_valid_bus_id(""));
        assert!(!is_valid_bus_id("1-1; rm -rf /"));
        assert!(!is_valid_bus_id("$(evil)"));
    }

    #[test]
    fn wrapper_rejects_invalid_bus_id_before_privilege() {
        let error = run_wrapper("bind", "1-1; true").expect_err("invalid bus id");

        assert!(matches!(error, UsbControlError::InvalidBusId));
    }
}
