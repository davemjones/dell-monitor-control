//! Why no monitors were found on Linux.
//!
//! DDC/CI on Linux goes through `/dev/i2c-*`, which only exist once the
//! `i2c-dev` kernel module is loaded and are usually root-only. The device
//! enumerator silently skips anything it can't open, so without this check
//! both problems would look like "no monitors found".

/// What happened when we tried to open one `/dev/i2c-*` device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    Ok,
    Denied,
    Failed,
}

/// A specific explanation when the I²C devices themselves are the problem,
/// or `None` when they look fine and the generic hint applies.
pub fn diagnose(devices: &[(String, Access)]) -> Option<String> {
    if devices.is_empty() {
        return Some(
            "no monitors found: there are no I²C devices (/dev/i2c-*), so the i2c-dev kernel module probably isn't loaded\n\
             hint: load it with `sudo modprobe i2c-dev`, and load it at every boot with\n      \
             `echo i2c-dev | sudo tee /etc/modules-load.d/i2c-dev.conf`"
                .into(),
        );
    }
    let denied: Vec<&str> = devices
        .iter()
        .filter(|(_, access)| *access == Access::Denied)
        .map(|(path, _)| path.as_str())
        .collect();
    if denied.is_empty() {
        return None;
    }
    Some(format!(
        "no monitors found: no permission to open {}\n\
         hint: give your user access to the I²C devices, then log out and back in:\n      \
         sudo groupadd --system i2c\n      \
         sudo usermod -aG i2c $USER\n      \
         echo 'KERNEL==\"i2c-[0-9]*\", GROUP=\"i2c\", MODE=\"0660\"' | sudo tee /etc/udev/rules.d/45-i2c.rules",
        denied.join(", ")
    ))
}

/// Try opening every `/dev/i2c-*` the way the DDC/CI backend does.
#[cfg(target_os = "linux")]
pub fn probe() -> Vec<(String, Access)> {
    use std::fs::{OpenOptions, read_dir};

    // Only a plain file-permission failure is fixed by the group setup in
    // the hint. EPERM comes from sandboxes or security modules instead.
    const EACCES: i32 = 13;

    let mut devices: Vec<(u32, String)> = read_dir("/dev")
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            let bus = name.strip_prefix("i2c-")?.parse().ok()?;
            Some((bus, format!("/dev/{name}")))
        })
        .collect();
    devices.sort();
    devices
        .into_iter()
        .map(|(_, path)| {
            let access = match OpenOptions::new().read(true).write(true).open(&path) {
                Ok(_) => Access::Ok,
                Err(e) if e.raw_os_error() == Some(EACCES) => Access::Denied,
                Err(_) => Access::Failed,
            };
            (path, access)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn devs(list: &[(&str, Access)]) -> Vec<(String, Access)> {
        list.iter().map(|(p, a)| (p.to_string(), *a)).collect()
    }

    #[test]
    fn missing_devices_mean_module_not_loaded() {
        let msg = diagnose(&[]).unwrap();
        assert!(msg.contains("i2c-dev kernel module"), "{msg}");
        assert!(msg.contains("sudo modprobe i2c-dev"), "{msg}");
        assert!(msg.contains("/etc/modules-load.d/i2c-dev.conf"), "{msg}");
    }

    #[test]
    fn denied_devices_mean_permissions() {
        let msg = diagnose(&devs(&[
            ("/dev/i2c-0", Access::Ok),
            ("/dev/i2c-3", Access::Denied),
            ("/dev/i2c-4", Access::Denied),
        ]))
        .unwrap();
        assert!(
            msg.starts_with("no monitors found: no permission to open /dev/i2c-3, /dev/i2c-4"),
            "{msg}"
        );
        assert!(msg.contains("usermod -aG i2c"), "{msg}");
        assert!(msg.contains("45-i2c.rules"), "{msg}");
    }

    #[test]
    fn accessible_devices_fall_back_to_the_generic_hint() {
        assert_eq!(
            diagnose(&devs(&[
                ("/dev/i2c-0", Access::Ok),
                ("/dev/i2c-1", Access::Failed),
            ])),
            None
        );
    }
}
