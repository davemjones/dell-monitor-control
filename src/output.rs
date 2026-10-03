//! What each command reports, and how it renders as text or JSON.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InputRef {
    pub name: String,
    pub code: String,
}

impl InputRef {
    pub fn new(code: u8) -> Self {
        InputRef {
            name: crate::vcp::input_name(code),
            code: format!("{code:#04x}"),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Status {
    pub monitor: usize,
    pub name: String,
    pub input: Option<InputRef>,
    /// Supported inputs; only read for `--json`, and `null` when unreadable.
    pub inputs: Option<Vec<InputEntry>>,
    pub brightness: Option<u16>,
    pub volume: Option<u16>,
    pub speakers: Option<&'static str>,
}

#[derive(Debug, Serialize)]
pub struct ListEntry {
    pub monitor: usize,
    pub name: String,
    pub serial: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub controls: Option<Controls>,
}

#[derive(Debug, Serialize)]
pub struct Controls {
    pub input: bool,
    pub brightness: bool,
    pub volume: bool,
    pub mute: bool,
}

#[derive(Debug, Serialize)]
pub struct InputEntry {
    #[serde(flatten)]
    pub input: InputRef,
    pub active: bool,
}

pub fn json<T: Serialize + ?Sized>(value: &T) -> String {
    serde_json::to_string_pretty(value).expect("output types always serialize")
}

fn or_na<T: ToString>(value: Option<T>) -> String {
    value.map(|v| v.to_string()).unwrap_or_else(|| "n/a".into())
}

pub fn status_text(statuses: &[Status]) -> String {
    statuses
        .iter()
        .map(|s| {
            format!(
                "{} (monitor {})\n  Input:      {}\n  Brightness: {}\n  Volume:     {}\n  Speakers:   {}",
                s.name,
                s.monitor,
                or_na(s.input.as_ref().map(|i| &i.name)),
                or_na(s.brightness),
                or_na(s.volume),
                or_na(s.speakers),
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub fn list_text(entries: &[ListEntry], verbose: bool) -> String {
    let width = entries.iter().map(|e| e.name.len()).max().unwrap_or(0);
    entries
        .iter()
        .map(|e| {
            let mut line = format!("{}  {:width$}", e.monitor, e.name);
            if let Some(serial) = &e.serial {
                line.push_str(&format!("  (serial {serial})"));
            }
            let mut line = line.trim_end().to_string();
            match &e.controls {
                Some(c) => {
                    let all = [
                        ("input", c.input),
                        ("brightness", c.brightness),
                        ("volume", c.volume),
                        ("speaker mute", c.mute),
                    ];
                    let pick = |want: bool| {
                        all.iter()
                            .filter(|(_, s)| *s == want)
                            .map(|(n, _)| *n)
                            .collect::<Vec<_>>()
                    };
                    let (yes, no) = (pick(true), pick(false));
                    line.push_str(&format!(
                        "\n   supports:      {}",
                        if yes.is_empty() {
                            "none".into()
                        } else {
                            yes.join(", ")
                        }
                    ));
                    if !no.is_empty() {
                        line.push_str(&format!("\n   not supported: {}", no.join(", ")));
                    }
                }
                None if verbose => {
                    line.push_str("\n   supports:      unknown (couldn't read its capabilities)")
                }
                None => {}
            }
            line
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn inputs_text(entries: &[InputEntry]) -> String {
    let width = entries
        .iter()
        .map(|e| e.input.name.len())
        .max()
        .unwrap_or(0);
    entries
        .iter()
        .map(|e| {
            format!(
                "{} {:width$}  [{}]",
                if e.active { '*' } else { ' ' },
                e.input.name,
                e.input.code
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}
