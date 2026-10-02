//! VCP feature codes and how their raw values map to what users see.

use crate::cli::Level;

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

/// Turn what the user typed after `input set` into an input code: an alias,
/// a canonical name, or a raw code (`0x0f`, `0F`, or decimal `15`).
/// Whether the monitor actually has that input is checked by the caller.
pub fn parse_input(arg: &str) -> Option<u8> {
    let arg = arg.trim().to_ascii_lowercase();
    let name = INPUT_ALIASES
        .iter()
        .find(|(alias, _)| *alias == arg)
        .map_or(arg.as_str(), |(_, name)| name);
    if let Some((code, _)) = INPUTS.iter().find(|(_, n)| *n == name) {
        return Some(*code);
    }
    if let Some(hex) = name.strip_prefix("0x") {
        return u8::from_str_radix(hex, 16).ok();
    }
    // Bare digits are decimal; anything else hex-looking (e.g. "0f") is hex.
    name.parse()
        .ok()
        .or_else(|| u8::from_str_radix(name, 16).ok())
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

/// The raw 0x8D value that mutes (1) or unmutes (2) the speakers.
pub fn mute_value(muted: bool) -> u16 {
    if muted { 1 } else { 2 }
}

/// The 0–100 level a `brightness`/`volume` argument asks for, given the
/// current level. Nudges clamp at the edges instead of failing.
pub fn apply_level(current: u16, level: Level) -> u16 {
    match level {
        Level::Set(n) => n.min(100),
        Level::Delta(d) => (i32::from(current) + i32::from(d)).clamp(0, 100) as u16,
    }
}

/// The inverse of `percent`: a 0–100 level on the monitor's own scale.
pub fn from_percent(pct: u16, max: u16) -> u16 {
    match max {
        0 | 100 => pct,
        _ => ((u32::from(pct.min(100)) * u32::from(max) + 50) / 100) as u16,
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
    fn parses_input_names_aliases_and_codes() {
        assert_eq!(parse_input("hdmi"), Some(0x11));
        assert_eq!(parse_input("HDMI1"), Some(0x11));
        assert_eq!(parse_input(" DisplayPort "), Some(0x0F));
        assert_eq!(parse_input("usb-c"), Some(0x19));
        assert_eq!(parse_input("usbc"), Some(0x19));
        assert_eq!(parse_input("TB"), Some(0x19));
        assert_eq!(parse_input("0x0f"), Some(0x0F));
        assert_eq!(parse_input("0x1B"), Some(0x1B));
        assert_eq!(parse_input("0f"), Some(0x0F));
        assert_eq!(parse_input("17"), Some(17));
        assert_eq!(parse_input("thunder"), None);
        assert_eq!(parse_input("0x100"), None);
        assert_eq!(parse_input(""), None);
    }

    #[test]
    fn applies_levels_with_clamping() {
        assert_eq!(apply_level(78, Level::Set(40)), 40);
        assert_eq!(apply_level(78, Level::Delta(10)), 88);
        assert_eq!(apply_level(78, Level::Delta(50)), 100);
        assert_eq!(apply_level(5, Level::Delta(-10)), 0);
    }

    #[test]
    fn scales_from_percent() {
        assert_eq!(from_percent(40, 100), 40);
        assert_eq!(from_percent(20, 255), 51);
        assert_eq!(from_percent(100, 255), 255);
        assert_eq!(from_percent(7, 0), 7);
        for pct in 0..=100 {
            assert_eq!(percent(from_percent(pct, 255), 255), pct);
        }
    }

    #[test]
    fn scales_to_percent() {
        assert_eq!(percent(78, 100), 78);
        assert_eq!(percent(50, 255), 20);
        assert_eq!(percent(255, 255), 100);
        assert_eq!(percent(7, 0), 7);
    }
}
