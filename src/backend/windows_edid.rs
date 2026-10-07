//! Monitor names on Windows.
//!
//! The Windows monitor-control API only gives a description, usually
//! "Generic PnP Monitor". The real model name and serial live in the EDID,
//! which Windows keeps a copy of in the registry under the monitor's device.

/// The registry key holding the EDID, from a monitor's device interface
/// name, e.g.
/// `\\?\DISPLAY#DELA1B3#5&1a2b&0&UID4352#{e6f07b5f-ee97-4a90-b076-33f57bf4eaa7}`
/// → `SYSTEM\CurrentControlSet\Enum\DISPLAY\DELA1B3\5&1a2b&0&UID4352\Device Parameters`.
pub fn parse_device_id(device_id: &str) -> Option<String> {
    let mut parts = device_id.split('#');
    let _prefix = parts.next()?;
    let hardware = parts.next().filter(|h| h.len() >= 3)?;
    let instance = parts.next().filter(|i| !i.is_empty())?;
    Some(format!(
        r"SYSTEM\CurrentControlSet\Enum\DISPLAY\{hardware}\{instance}\Device Parameters"
    ))
}

#[cfg(windows)]
pub use win32::lookup;

#[cfg(windows)]
mod win32 {
    use std::mem::size_of;
    use std::ptr;

    use windows_sys::Win32::Devices::Display::GetNumberOfPhysicalMonitorsFromHMONITOR;
    use windows_sys::Win32::Foundation::{ERROR_SUCCESS, LPARAM, RECT};
    use windows_sys::Win32::Graphics::Gdi::{
        DISPLAY_DEVICE_ACTIVE, DISPLAY_DEVICEW, EnumDisplayDevicesW, EnumDisplayMonitors,
        GetMonitorInfoW, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW,
    };
    use windows_sys::Win32::System::Registry::{
        HKEY_LOCAL_MACHINE, RRF_RT_REG_BINARY, RegGetValueW,
    };
    use windows_sys::core::BOOL;

    use super::parse_device_id;
    use crate::backend::MonitorInfo;

    /// Ask `EnumDisplayDevicesW` for device interface names rather than
    /// hardware IDs (from WinUser.h).
    const EDD_GET_DEVICE_INTERFACE_NAME: u32 = 1;

    /// What the EDID says about each physical monitor, in the same order the
    /// `ddc-winapi` backend enumerates them: monitors in `EnumDisplayMonitors`
    /// order, then each one's physical monitors. `None` where it couldn't be
    /// worked out.
    pub fn lookup() -> Vec<Option<MonitorInfo>> {
        hmonitors()
            .into_iter()
            .flat_map(|hmonitor| {
                let mut count = 0;
                if unsafe { GetNumberOfPhysicalMonitorsFromHMONITOR(hmonitor, &mut count) } == 0 {
                    count = 0;
                }
                let mut ids = device_ids(hmonitor).into_iter();
                (0..count)
                    .map(|_| ids.next().and_then(|id| info_for(&id)))
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    fn hmonitors() -> Vec<HMONITOR> {
        unsafe extern "system" fn push(
            hmonitor: HMONITOR,
            _: HDC,
            _: *mut RECT,
            data: LPARAM,
        ) -> BOOL {
            let list = unsafe { &mut *(data as *mut Vec<HMONITOR>) };
            list.push(hmonitor);
            1
        }
        let mut list: Vec<HMONITOR> = Vec::new();
        unsafe {
            EnumDisplayMonitors(
                ptr::null_mut(),
                ptr::null(),
                Some(push),
                &mut list as *mut _ as LPARAM,
            );
        }
        list
    }

    /// Device interface names of the active monitors behind one HMONITOR.
    fn device_ids(hmonitor: HMONITOR) -> Vec<String> {
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
        if unsafe { GetMonitorInfoW(hmonitor, &mut info as *mut _ as *mut MONITORINFO) } == 0 {
            return vec![];
        }
        let mut ids = Vec::new();
        for i in 0.. {
            let mut device = DISPLAY_DEVICEW {
                cb: size_of::<DISPLAY_DEVICEW>() as u32,
                ..Default::default()
            };
            let found = unsafe {
                EnumDisplayDevicesW(
                    info.szDevice.as_ptr(),
                    i,
                    &mut device,
                    EDD_GET_DEVICE_INTERFACE_NAME,
                )
            };
            if found == 0 {
                break;
            }
            if device.StateFlags & DISPLAY_DEVICE_ACTIVE != 0 {
                ids.push(from_wide(&device.DeviceID));
            }
        }
        ids
    }

    fn info_for(device_id: &str) -> Option<MonitorInfo> {
        let edid = read_edid(&parse_device_id(device_id)?)?;
        let edid =
            ddc_hi::DisplayInfo::from_edid(ddc_hi::Backend::WinApi, device_id.into(), edid).ok()?;
        Some(MonitorInfo {
            name: edid.model_name.unwrap_or_default(),
            serial: edid.serial_number,
        })
    }

    fn read_edid(key: &str) -> Option<Vec<u8>> {
        let key = to_wide(key);
        let value = to_wide("EDID");
        let mut edid = vec![0u8; 512];
        let mut len = edid.len() as u32;
        let status = unsafe {
            RegGetValueW(
                HKEY_LOCAL_MACHINE,
                key.as_ptr(),
                value.as_ptr(),
                RRF_RT_REG_BINARY,
                ptr::null_mut(),
                edid.as_mut_ptr().cast(),
                &mut len,
            )
        };
        if status != ERROR_SUCCESS || len < 128 {
            return None;
        }
        edid.truncate(len as usize);
        Some(edid)
    }

    fn to_wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    fn from_wide(s: &[u16]) -> String {
        let end = s.iter().position(|&c| c == 0).unwrap_or(s.len());
        String::from_utf16_lossy(&s[..end])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_device_interface_names() {
        assert_eq!(
            parse_device_id(
                r"\\?\DISPLAY#DELA1B3#5&1a2b3c&0&UID4352#{e6f07b5f-ee97-4a90-b076-33f57bf4eaa7}",
            )
            .unwrap(),
            r"SYSTEM\CurrentControlSet\Enum\DISPLAY\DELA1B3\5&1a2b3c&0&UID4352\Device Parameters"
        );
    }

    #[test]
    fn rejects_other_device_id_shapes() {
        // Without EDD_GET_DEVICE_INTERFACE_NAME Windows returns this form.
        assert!(
            parse_device_id(r"MONITOR\DELA1B3\{4d36e96e-e325-11ce-bfc1-08002be10318}\0001")
                .is_none()
        );
        assert!(parse_device_id("").is_none());
        assert!(parse_device_id(r"\\?\DISPLAY#DE").is_none());
        assert!(parse_device_id(r"\\?\DISPLAY#DELA1B3#").is_none());
    }
}
