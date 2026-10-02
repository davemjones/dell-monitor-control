//! macOS's DDC/CI interface doesn't expose serial numbers, but the IORegistry
//! does: each display's `AppleCLCD2` node carries
//! `DisplayAttributes.ProductAttributes` with the product name and serial.
//! We match them to monitors by name.

use std::collections::HashMap;
use std::process::Command;

use plist::Value;

/// Map of upper-cased product name → serial. A name seen more than once maps
/// to `None`, since we can't tell which identical monitor owns which serial.
pub type SerialMap = HashMap<String, Option<String>>;

/// Best effort: any failure yields an empty map rather than an error.
pub fn lookup() -> SerialMap {
    Command::new("ioreg")
        .args(["-a", "-r", "-c", "AppleCLCD2", "-d", "1"])
        .output()
        .ok()
        .filter(|out| out.status.success())
        .map(|out| parse(&out.stdout))
        .unwrap_or_default()
}

fn parse(plist_xml: &[u8]) -> SerialMap {
    let mut map = SerialMap::new();
    let Ok(Value::Array(nodes)) = Value::from_reader_xml(plist_xml) else {
        return map;
    };
    for attrs in nodes
        .iter()
        .filter_map(|n| n.as_dictionary()?.get("DisplayAttributes")?.as_dictionary())
        .filter_map(|d| d.get("ProductAttributes")?.as_dictionary())
    {
        let Some(name) = attrs.get("ProductName").and_then(Value::as_string) else {
            continue;
        };
        let serial = attrs
            .get("AlphanumericSerialNumber")
            .and_then(Value::as_string)
            .map(str::to_owned)
            .or_else(|| {
                attrs
                    .get("SerialNumber")
                    .and_then(Value::as_unsigned_integer)
                    .map(|n| n.to_string())
            });
        map.entry(name.to_ascii_uppercase())
            .and_modify(|existing| *existing = None)
            .or_insert(serial);
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(name: &str, serial: &str) -> String {
        format!(
            "<dict><key>IOObjectClass</key><string>AppleCLCD2</string><key>DisplayAttributes</key><dict><key>ProductAttributes</key><dict>\
             <key>AlphanumericSerialNumber</key><string>{serial}</string>\
             <key>ManufacturerID</key><string>DEL</string>\
             <key>ProductName</key><string>{name}</string>\
             <key>SerialNumber</key><integer>811158348</integer></dict></dict></dict>"
        )
    }

    fn doc(nodes: &[String]) -> Vec<u8> {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><plist version="1.0"><array>{}<dict><key>IOObjectClass</key><string>AppleCLCD2</string></dict></array></plist>"#,
            nodes.concat()
        )
        .into_bytes()
    }

    #[test]
    fn maps_name_to_serial() {
        let map = parse(&doc(&[node("DELL U3425WE", "JK4HV84")]));
        assert_eq!(map.get("DELL U3425WE"), Some(&Some("JK4HV84".to_string())));
    }

    #[test]
    fn identical_models_get_no_serial() {
        let map = parse(&doc(&[
            node("DELL U3425WE", "AAA"),
            node("DELL U3425WE", "BBB"),
        ]));
        assert_eq!(map.get("DELL U3425WE"), Some(&None));
    }

    #[test]
    fn garbage_is_empty() {
        assert!(parse(b"not a plist").is_empty());
    }
}
