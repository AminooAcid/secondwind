pub const CRATE_PURPOSE: &str = "host-side launch helper scaffold";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LauncherStatus {
    pub ready: bool,
}

pub fn status() -> LauncherStatus {
    LauncherStatus { ready: true }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launcher_scaffold_reports_ready() {
        assert!(status().ready);
    }
}
