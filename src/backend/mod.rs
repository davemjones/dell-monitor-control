//! The boundary between command logic and the OS-specific DDC/CI channel.
//!
//! Commands only see the `Backend` and `Monitor` traits, so tests can swap in
//! a fake and new platforms never touch the command code.

mod ddc;
#[cfg(test)]
pub mod fake;
#[cfg(any(target_os = "linux", test))]
mod linux_i2c;
#[cfg(target_os = "macos")]
mod macos_serial;
#[cfg(any(windows, test))]
mod windows_edid;

use std::collections::BTreeMap;

use anyhow::{Context, Result};

pub use ddc::DdcHiBackend;

/// What we know about a monitor without talking DDC/CI to it.
#[derive(Debug, Clone, Default)]
pub struct MonitorInfo {
    /// Human-readable name, e.g. "DELL U3425WE".
    pub name: String,
    pub serial: Option<String>,
}

/// A raw VCP reading: current value and the monitor's reported maximum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VcpValue {
    pub value: u16,
    pub max: u16,
}

/// The parsed DDC/CI capabilities string.
#[derive(Debug, Clone, Default)]
pub struct Caps {
    /// Supported VCP codes and, for non-continuous ones, their allowed values.
    pub features: BTreeMap<u8, Vec<u8>>,
}

impl Caps {
    pub fn parse(raw: &[u8]) -> Result<Self> {
        let caps = mccs_caps::parse_capabilities(raw)
            .context("monitor sent an unreadable capabilities string")?;
        let features = caps
            .vcp_features
            .iter()
            .map(|(code, desc)| (*code, desc.values().copied().collect()))
            .collect();
        Ok(Caps { features })
    }

    pub fn supports(&self, code: u8) -> bool {
        self.features.contains_key(&code)
    }
}

pub trait Monitor {
    fn info(&self) -> &MonitorInfo;
    fn read_vcp(&mut self, code: u8) -> Result<VcpValue>;
    fn write_vcp(&mut self, code: u8, value: u16) -> Result<()>;
    fn capabilities(&mut self) -> Result<Caps>;
}

pub trait Backend {
    fn enumerate(&self) -> Result<Vec<Box<dyn Monitor>>>;
}

#[cfg(test)]
mod tests {
    use super::fake::U3425WE_CAPS;
    use super::*;

    #[test]
    fn parses_real_u3425we_caps() {
        let caps = Caps::parse(U3425WE_CAPS.as_bytes()).unwrap();
        assert_eq!(caps.features[&0x60], vec![0x0F, 0x11, 0x19]);
        assert!(caps.supports(0x10));
        assert!(caps.supports(0x62));
        assert!(!caps.supports(0x8D));
    }
}
