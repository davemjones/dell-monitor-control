//! Real monitor access through `ddc-hi`, which wraps the native DDC/CI
//! channel on macOS, Windows, and Linux.

use anyhow::{Result, anyhow};
use ddc_hi::{Ddc, Display};

use super::{Backend, Caps, Monitor, MonitorInfo, VcpValue};

pub struct DdcHiBackend {
    /// Look up serial numbers outside DDC/CI where the platform needs it.
    /// Only `list` shows serials, so other commands skip the extra lookup.
    pub with_serials: bool,
}

impl Backend for DdcHiBackend {
    fn enumerate(&self) -> Result<Vec<Box<dyn Monitor>>> {
        #[cfg(target_os = "macos")]
        let serials = if self.with_serials {
            super::macos_serial::lookup()
        } else {
            Default::default()
        };

        let monitors = Display::enumerate()
            .into_iter()
            .map(|display| {
                let mut info = MonitorInfo {
                    name: display
                        .info
                        .model_name
                        .clone()
                        .unwrap_or_else(|| display.info.id.clone()),
                    manufacturer: display.info.manufacturer_id.clone(),
                    serial: display.info.serial_number.clone(),
                };
                #[cfg(target_os = "macos")]
                if info.serial.is_none() {
                    info.serial = serials
                        .get(&info.name.to_ascii_uppercase())
                        .cloned()
                        .flatten();
                }
                Box::new(DdcHiMonitor { display, info }) as Box<dyn Monitor>
            })
            .collect();
        Ok(monitors)
    }
}

struct DdcHiMonitor {
    display: Display,
    info: MonitorInfo,
}

impl Monitor for DdcHiMonitor {
    fn info(&self) -> &MonitorInfo {
        &self.info
    }

    fn read_vcp(&mut self, code: u8) -> Result<VcpValue> {
        let v = self.display.handle.get_vcp_feature(code).map_err(|e| {
            anyhow!(
                "couldn't read setting {code:#04x} from {}: {e}",
                self.info.name
            )
        })?;
        Ok(VcpValue {
            value: v.value(),
            max: v.maximum(),
        })
    }

    fn capabilities(&mut self) -> Result<Caps> {
        let raw = self
            .display
            .handle
            .capabilities_string()
            .map_err(|e| anyhow!("couldn't read capabilities from {}: {e}", self.info.name))?;
        Caps::parse(&raw)
    }
}
