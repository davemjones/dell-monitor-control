//! Errors that scripts and the menu bar app need to tell apart.

use std::fmt;

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureKind {
    NoMonitor,
    MultipleMonitors,
    Unsupported,
    Failed,
}

/// An error with a kind attached, reported as `{"error": {"kind", "message"}}`
/// under `--json`. Errors without one are reported as `failed`.
#[derive(Debug)]
pub struct Failure {
    pub kind: FailureKind,
    pub message: String,
}

impl Failure {
    pub fn new(kind: FailureKind, message: impl Into<String>) -> Self {
        Failure {
            kind,
            message: message.into(),
        }
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Failure {}

/// The kind of any error, looking through added context.
pub fn kind_of(err: &anyhow::Error) -> FailureKind {
    err.chain()
        .find_map(|e| e.downcast_ref::<Failure>())
        .map_or(FailureKind::Failed, |f| f.kind)
}

/// The `--json` form of an error.
pub fn json(err: &anyhow::Error) -> String {
    serde_json::json!({
        "error": { "kind": kind_of(err), "message": format!("{err:#}") }
    })
    .to_string()
}
