use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub trait CartridgeSaveTimeSource {
    fn now_unix_seconds(&self) -> u64;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SystemCartridgeSaveTimeSource;

impl CartridgeSaveTimeSource for SystemCartridgeSaveTimeSource {
    fn now_unix_seconds(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_secs()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedCartridgeSaveTimeSource {
    unix_seconds: u64,
}

impl FixedCartridgeSaveTimeSource {
    pub const fn new(unix_seconds: u64) -> Self {
        Self { unix_seconds }
    }
}

impl CartridgeSaveTimeSource for FixedCartridgeSaveTimeSource {
    fn now_unix_seconds(&self) -> u64 {
        self.unix_seconds
    }
}
