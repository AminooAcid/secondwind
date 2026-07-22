pub fn startup_status() -> &'static str {
    "sw-kiosk scaffold ready"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kiosk_scaffold_reports_ready() {
        assert_eq!(startup_status(), "sw-kiosk scaffold ready");
    }
}
