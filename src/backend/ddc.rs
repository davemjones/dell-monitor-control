//! Real monitor access through `ddc-hi`, which wraps the native DDC/CI
//! channel on macOS, Windows, and Linux.

use std::thread::sleep;
use std::time::Duration;

use anyhow::{Result, anyhow};
use ddc_hi::{Ddc, Display, DisplayInfo};

use super::{Backend, Caps, Monitor, MonitorInfo, VcpValue};

pub struct DdcHiBackend {
    /// Look up serial numbers outside DDC/CI where the platform needs it.
    /// Only `list` shows serials, so other commands skip the extra lookup.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
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

        let displays = Display::enumerate();

        #[cfg(target_os = "linux")]
        if displays.is_empty()
            && let Some(problem) = super::linux_i2c::diagnose(&super::linux_i2c::probe())
        {
            anyhow::bail!(problem);
        }

        #[allow(unused_mut)] // only Windows replaces entries
        let mut infos: Vec<MonitorInfo> = displays.iter().map(|d| info_from(&d.info)).collect();

        // The Windows API only names monitors "Generic PnP Monitor"; the
        // registry's EDID copy has the real name and serial. Only trust the
        // pairing when both sides see the same number of monitors.
        #[cfg(windows)]
        {
            let edid = super::windows_edid::lookup();
            if edid.len() == infos.len() {
                for (info, found) in infos.iter_mut().zip(edid) {
                    let Some(found) = found else { continue };
                    if !found.name.is_empty() {
                        info.name = found.name;
                    }
                    info.manufacturer = found.manufacturer.or(info.manufacturer.take());
                    info.serial = found.serial.or(info.serial.take());
                }
            }
        }

        #[cfg(target_os = "macos")]
        for info in &mut infos {
            if info.serial.is_none() {
                info.serial = serials
                    .get(&info.name.to_ascii_uppercase())
                    .cloned()
                    .flatten();
            }
        }

        Ok(displays
            .into_iter()
            .zip(infos)
            .map(|(display, info)| Box::new(DdcHiMonitor { display, info }) as Box<dyn Monitor>)
            .collect())
    }
}

fn info_from(info: &DisplayInfo) -> MonitorInfo {
    MonitorInfo {
        name: info.model_name.clone().unwrap_or_else(|| info.id.clone()),
        manufacturer: info.manufacturer_id.clone(),
        serial: info.serial_number.clone(),
    }
}

/// DDC/CI transactions fail now and then (bad length, checksum mismatch),
/// especially right after another command, so each one gets a few tries
/// with a growing pause in between.
const ATTEMPTS: u32 = 4;

fn retry<T, E>(mut op: impl FnMut() -> Result<T, E>) -> Result<T, E> {
    let mut attempt = 1;
    loop {
        match op() {
            Err(_) if attempt < ATTEMPTS => {
                sleep(Duration::from_millis(50 * u64::from(attempt)));
                attempt += 1;
            }
            result => return result,
        }
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
        let handle = &mut self.display.handle;
        let v = retry(|| handle.get_vcp_feature(code)).map_err(|e| {
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

    fn write_vcp(&mut self, code: u8, value: u16) -> Result<()> {
        self.display
            .handle
            .set_vcp_feature(code, value)
            .map_err(|e| {
                anyhow!(
                    "couldn't change setting {code:#04x} on {}: {e}",
                    self.info.name
                )
            })
    }

    fn capabilities(&mut self) -> Result<Caps> {
        let handle = &mut self.display.handle;
        let raw = retry(|| handle.capabilities_string())
            .map_err(|e| anyhow!("couldn't read capabilities from {}: {e}", self.info.name))?;
        Caps::parse(&raw)
    }
}
