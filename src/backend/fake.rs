//! In-memory monitors for tests.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use anyhow::{Result, bail};

use super::{Backend, Caps, Monitor, MonitorInfo, VcpValue};

/// The capabilities string read from a real U3425WE.
pub const U3425WE_CAPS: &str = "(prot(monitor)type(LCD)model(U3425WE)cmds(01 02 03 07 0C E3 F3)vcp(02 04 05 08 10 12 14(01 04 05 06 08 09 0B 0C) 16 18 1A 52 60(19 0F 11 ) 62 66(00F2) 67 68 87 AC AE B2 B6 C6 C8 C9 CA CC(02 0A 03 04 08 09 0D 06 ) D6(01 04 05) DC(00 03 05 ) DF E0(03) E2(00 02 04 0C 0D 0F 10 11 13 0B 1A 1B 3D 14 ) E4(00 01) E5 E7(02 03) E8 E9(00 01 02 21 22 24 2B 2C ) EA(FC F8 ) F0(09 0A A1 ) EE EF(00 01 0F) F1 F2 FD)mswhql(1)asset_eep(40)mccs_ver(2.1))";

/// Every write a fake monitor received, as (VCP code, raw value).
pub type WriteLog = Rc<RefCell<Vec<(u8, u16)>>>;

#[derive(Clone, Default)]
pub struct FakeMonitor {
    pub info: MonitorInfo,
    pub values: HashMap<u8, VcpValue>,
    pub caps: Option<&'static str>,
    /// Every write sent to this monitor, shared across clones so tests can
    /// inspect it after `FakeBackend` hands out copies.
    pub writes: WriteLog,
    /// Make every write fail, like a monitor that ignores DDC/CI writes.
    pub read_only: bool,
}

impl FakeMonitor {
    /// A U3425WE reading like the real one did: Thunderbolt active, mute unsupported.
    pub fn u3425we() -> Self {
        FakeMonitor {
            info: MonitorInfo {
                name: "DELL U3425WE".into(),
                manufacturer: None,
                serial: Some("JK4HV84".into()),
            },
            values: HashMap::from([
                (
                    0x10,
                    VcpValue {
                        value: 78,
                        max: 100,
                    },
                ),
                (
                    0x60,
                    VcpValue {
                        value: 0x1919,
                        max: 0x1919,
                    },
                ),
                (
                    0x62,
                    VcpValue {
                        value: 88,
                        max: 100,
                    },
                ),
                (
                    0x8D,
                    VcpValue {
                        value: 0,
                        max: 0xFFFF,
                    },
                ),
            ]),
            caps: Some(U3425WE_CAPS),
            ..Default::default()
        }
    }

    pub fn named(name: &str) -> Self {
        FakeMonitor {
            info: MonitorInfo {
                name: name.into(),
                ..Default::default()
            },
            ..Default::default()
        }
    }
}

impl Monitor for FakeMonitor {
    fn info(&self) -> &MonitorInfo {
        &self.info
    }

    fn read_vcp(&mut self, code: u8) -> Result<VcpValue> {
        match self.values.get(&code) {
            Some(v) => Ok(*v),
            None => bail!(
                "couldn't read {} from {}",
                crate::vcp::feature_name(code),
                self.info.name
            ),
        }
    }

    fn write_vcp(&mut self, code: u8, value: u16) -> Result<()> {
        if self.read_only {
            bail!(
                "couldn't change {} on {}",
                crate::vcp::feature_name(code),
                self.info.name
            );
        }
        self.writes.borrow_mut().push((code, value));
        let max = self.values.get(&code).map_or(100, |v| v.max);
        self.values.insert(code, VcpValue { value, max });
        Ok(())
    }

    fn capabilities(&mut self) -> Result<Caps> {
        match self.caps {
            Some(raw) => Caps::parse(raw.as_bytes()),
            None => bail!(
                "couldn't read the list of supported settings from {}",
                self.info.name
            ),
        }
    }
}

pub struct FakeBackend(pub Vec<FakeMonitor>);

impl Backend for FakeBackend {
    fn enumerate(&self) -> Result<Vec<Box<dyn Monitor>>> {
        Ok(self
            .0
            .iter()
            .cloned()
            .map(|m| Box::new(m) as Box<dyn Monitor>)
            .collect())
    }
}
