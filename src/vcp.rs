//! VCP feature codes and how their raw values map to what users see.

pub const BRIGHTNESS: u8 = 0x10;
pub const INPUT: u8 = 0x60;
pub const VOLUME: u8 = 0x62;
pub const MUTE: u8 = 0x8D;

/// Known input codes. 0x19 is what Dell uses for its Thunderbolt/USB-C port
/// (confirmed on the U3425WE); the rest follow the MCCS standard.
const INPUTS: &[(u8, &str)] = &[
    (0x01, "vga"),
    (0x03, "dvi"),
    (0x0F, "dp"),
    (0x10, "dp2"),
    (0x11, "hdmi"),
    (0x12, "hdmi2"),
    (0x19, "thunderbolt"),
];

/// Extra names accepted by `input set`, mapped to the canonical name.
#[allow(dead_code)] // used by `input set` in milestone 2
pub const INPUT_ALIASES: &[(&str, &str)] = &[
    ("displayport", "dp"),
    ("dp1", "dp"),
    ("hdmi1", "hdmi"),
    ("tb", "thunderbolt"),
    ("usbc", "thunderbolt"),
    ("usb-c", "thunderbolt"),
];

pub fn input_name(code: u8) -> String {
    INPUTS
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(_, name)| name.to_string())
        .unwrap_or_else(|| format!("input-{code:#04x}"))
}

/// Some monitors (the U3425WE included) repeat the input code in the high
/// byte; only the low byte identifies the input.
pub fn current_input(raw: u16) -> u8 {
    (raw & 0xFF) as u8
}

/// Whether the speakers are muted, or `None` when the reply isn't a valid
/// mute value (1 = muted, 2 = unmuted), which means mute isn't supported.
pub fn muted(raw: u16) -> Option<bool> {
    match raw {
        1 => Some(true),
        2 => Some(false),
        _ => None,
    }
}

/// Express a reading as 0–100 regardless of the monitor's own scale.
pub fn percent(value: u16, max: u16) -> u16 {
    match max {
        0 => value,
        100 => value.min(100),
        _ => ((u32::from(value.min(max)) * 100 + u32::from(max) / 2) / u32::from(max)) as u16,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_known_and_unknown_inputs() {
        assert_eq!(input_name(0x19), "thunderbolt");
        assert_eq!(input_name(0x0F), "dp");
        assert_eq!(input_name(0x11), "hdmi");
        assert_eq!(input_name(0x42), "input-0x42");
    }

    #[test]
    fn masks_current_input_to_low_byte() {
        assert_eq!(current_input(0x1919), 0x19);
        assert_eq!(current_input(0x0011), 0x11);
    }

    #[test]
    fn validates_mute() {
        assert_eq!(muted(1), Some(true));
        assert_eq!(muted(2), Some(false));
        assert_eq!(muted(0), None);
        assert_eq!(muted(0xFFFF), None);
    }

    #[test]
    fn scales_to_percent() {
        assert_eq!(percent(78, 100), 78);
        assert_eq!(percent(50, 255), 20);
        assert_eq!(percent(255, 255), 100);
        assert_eq!(percent(7, 0), 7);
    }
}
