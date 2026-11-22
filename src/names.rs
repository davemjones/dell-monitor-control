//! Custom input names ("Work Mac", "Gaming PC"), kept per monitor model in a
//! small JSON file so the CLI and the menu bar app share them.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::vcp;

/// The longest name accepted, so names fit the panel and `input list`.
pub const MAX_LEN: usize = 20;

const VERSION: u32 = 1;

/// One monitor model's names, keyed by input code (`"0x11"`).
pub type Labels = BTreeMap<String, String>;

/// The whole names file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Names {
    pub version: u32,
    #[serde(default)]
    pub monitors: BTreeMap<String, MonitorNames>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonitorNames {
    #[serde(default)]
    pub inputs: Labels,
}

impl Default for Names {
    fn default() -> Self {
        Names {
            version: VERSION,
            monitors: BTreeMap::new(),
        }
    }
}

/// How the file and the output spell an input code.
pub fn code_key(code: u8) -> String {
    format!("{code:#04x}")
}

impl Names {
    /// The names for one monitor model (empty if it has none).
    pub fn labels(&self, model: &str) -> Labels {
        self.monitors
            .get(model)
            .map(|m| m.inputs.clone())
            .unwrap_or_default()
    }

    pub fn get(&self, model: &str, code: u8) -> Option<&str> {
        self.monitors
            .get(model)?
            .inputs
            .get(&code_key(code))
            .map(String::as_str)
    }

    pub fn set(&mut self, model: &str, code: u8, label: String) {
        self.monitors
            .entry(model.to_string())
            .or_default()
            .inputs
            .insert(code_key(code), label);
    }

    /// Removes a name; a monitor left with no names is dropped from the file.
    pub fn clear(&mut self, model: &str, code: u8) {
        if let Some(m) = self.monitors.get_mut(model) {
            m.inputs.remove(&code_key(code));
            if m.inputs.is_empty() {
                self.monitors.remove(model);
            }
        }
    }
}

/// The input code a custom name belongs to, ignoring case and surrounding spaces.
pub fn find(labels: &Labels, name: &str) -> Option<u8> {
    let name = name.trim().to_lowercase();
    labels
        .iter()
        .find(|(_, label)| label.to_lowercase() == name)
        .and_then(|(code, _)| u8::from_str_radix(code.trim_start_matches("0x"), 16).ok())
}

/// Checks a new name for input `code` and returns it trimmed.
pub fn validate(label: &str, code: u8, labels: &Labels) -> Result<String> {
    let label = label.trim();
    if label.is_empty() {
        bail!(
            "the name is empty\nhint: to remove a name, use `monitorctl input name <input> --clear`"
        );
    }
    if label.chars().count() > MAX_LEN {
        bail!("'{label}' is too long; names can be up to {MAX_LEN} characters");
    }
    if label.chars().any(char::is_control) {
        bail!("names can't contain tabs, line breaks, or other control characters");
    }
    if vcp::parse_input(label).is_some() {
        bail!(
            "'{label}' already means an input to `monitorctl input set`\n\
             hint: pick a name that isn't an input name, alias, or code"
        );
    }
    if let Some(other) = find(labels, label).filter(|&c| c != code) {
        bail!(
            "{} is already named '{label}'\nhint: pick another name, or rename {} first",
            vcp::input_name(other),
            vcp::input_name(other)
        );
    }
    Ok(label.to_string())
}

/// Where names are loaded from and saved to.
pub trait NameStore {
    fn load(&self) -> Result<Names>;
    fn save(&self, names: &Names) -> Result<()>;
}

/// The names file on disk.
pub struct FileStore {
    path: Option<PathBuf>,
}

impl FileStore {
    /// `$MONITORCTL_NAMES_FILE`, else `<config dir>/monitorctl/names.json`.
    pub fn from_env() -> Self {
        let path = std::env::var_os("MONITORCTL_NAMES_FILE")
            .filter(|p| !p.is_empty())
            .map(PathBuf::from)
            .or_else(|| dirs::config_dir().map(|d| d.join("monitorctl").join("names.json")));
        FileStore { path }
    }

    #[cfg(test)]
    pub fn at(path: impl Into<PathBuf>) -> Self {
        FileStore {
            path: Some(path.into()),
        }
    }

    fn path(&self) -> Result<&Path> {
        self.path.as_deref().context(
            "couldn't find a folder to keep input names in\n\
             hint: set MONITORCTL_NAMES_FILE to the file to use",
        )
    }
}

impl NameStore for FileStore {
    fn load(&self) -> Result<Names> {
        let path = self.path()?;
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Names::default()),
            Err(e) => {
                return Err(e).with_context(|| format!("couldn't read {}", path.display()));
            }
        };
        let names: Names = serde_json::from_str(&text).with_context(|| {
            format!(
                "{} isn't a valid names file\nhint: fix it, or delete it to start over",
                path.display()
            )
        })?;
        if names.version != VERSION {
            bail!(
                "{} is version {} of the names file; this monitorctl reads version {VERSION}\n\
                 hint: update monitorctl, or delete the file to start over",
                path.display(),
                names.version
            );
        }
        Ok(names)
    }

    fn save(&self, names: &Names) -> Result<()> {
        let path = self.path()?;
        let fail = || format!("couldn't save input names to {}", path.display());
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir).with_context(fail)?;
        }
        let mut text = serde_json::to_string_pretty(names).expect("names always serialize");
        text.push('\n');
        // Write then rename, so a crash never leaves half a file behind.
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, text).with_context(fail)?;
        std::fs::rename(&tmp, path).with_context(fail)?;
        Ok(())
    }
}

/// Names kept in memory, for tests.
#[cfg(test)]
pub struct MemoryStore {
    pub names: std::cell::RefCell<Names>,
    /// Make `load` fail, like an unreadable file.
    pub broken: bool,
}

#[cfg(test)]
impl MemoryStore {
    pub fn new() -> Self {
        MemoryStore {
            names: Default::default(),
            broken: false,
        }
    }

    pub fn with(model: &str, names: &[(u8, &str)]) -> Self {
        let store = MemoryStore::new();
        for (code, label) in names {
            store
                .names
                .borrow_mut()
                .set(model, *code, label.to_string());
        }
        store
    }
}

#[cfg(test)]
impl NameStore for MemoryStore {
    fn load(&self) -> Result<Names> {
        if self.broken {
            bail!("names.json isn't a valid names file");
        }
        Ok(self.names.borrow().clone())
    }

    fn save(&self, names: &Names) -> Result<()> {
        *self.names.borrow_mut() = names.clone();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MODEL: &str = "DELL U3425WE";

    #[test]
    fn set_get_clear() {
        let mut names = Names::default();
        names.set(MODEL, 0x11, "Gaming PC".into());
        assert_eq!(names.get(MODEL, 0x11), Some("Gaming PC"));
        assert_eq!(names.get("OTHER", 0x11), None);
        assert_eq!(names.labels(MODEL)["0x11"], "Gaming PC");
        names.clear(MODEL, 0x11);
        assert_eq!(names, Names::default());
    }

    #[test]
    fn finds_names_ignoring_case() {
        let labels = Labels::from([("0x11".into(), "Gaming PC".into())]);
        assert_eq!(find(&labels, " gaming pc "), Some(0x11));
        assert_eq!(find(&labels, "Gaming"), None);
    }

    #[test]
    fn validates_names() {
        let labels = Labels::from([("0x11".into(), "Gaming PC".into())]);
        assert_eq!(validate("  Work Mac ", 0x19, &labels).unwrap(), "Work Mac");
        // Re-using an input's own name is fine.
        assert_eq!(validate("gaming pc", 0x11, &labels).unwrap(), "gaming pc");
        let err = |l: &str| validate(l, 0x19, &labels).unwrap_err().to_string();
        assert!(err("  ").contains("empty"));
        assert_eq!(
            validate(&"é".repeat(20), 0x19, &labels)
                .unwrap()
                .chars()
                .count(),
            20
        );
        assert!(err(&"x".repeat(21)).contains("too long"));
        assert!(err("a\tb").contains("control"));
        for clash in ["hdmi", "TB", "usb-c", "0x11", "17", "fe"] {
            assert!(err(clash).contains("already means an input"), "{clash}");
        }
        assert!(err("Gaming PC").starts_with("hdmi is already named"));
    }

    #[test]
    fn file_store_round_trip() {
        let dir = std::env::temp_dir().join(format!("monitorctl-names-{}", std::process::id()));
        let store = FileStore::at(dir.join("sub").join("names.json"));
        assert_eq!(store.load().unwrap(), Names::default());

        let mut names = Names::default();
        names.set(MODEL, 0x19, "Work Mac".into());
        store.save(&names).unwrap();
        assert_eq!(store.load().unwrap(), names);
        let text = std::fs::read_to_string(dir.join("sub").join("names.json")).unwrap();
        assert!(text.contains("\"0x19\": \"Work Mac\""), "{text}");

        std::fs::write(dir.join("sub").join("names.json"), "{nope").unwrap();
        assert!(
            store
                .load()
                .unwrap_err()
                .to_string()
                .contains("isn't a valid names file")
        );
        std::fs::write(dir.join("sub").join("names.json"), r#"{"version":2}"#).unwrap();
        assert!(store.load().unwrap_err().to_string().contains("version 2"));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
