//! Sandbox mock state (port of Go `pkg/mock`), injected via `CARAPACE_SANDBOX`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Mock context for sandbox tests: work directory and command replies.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Mock {
    #[serde(rename = "Dir", default)]
    pub dir: PathBuf,
    #[serde(rename = "Replies", default)]
    pub replies: HashMap<String, String>,
}

impl Mock {
    /// Read the active mock from `CARAPACE_SANDBOX` (None when unset/invalid).
    pub fn current() -> Option<Self> {
        let sandbox = std::env::var("CARAPACE_SANDBOX").ok()?;
        if sandbox.is_empty() {
            return None;
        }
        serde_json::from_str(&sandbox).ok()
    }

    /// Key for a command invocation: JSON-encoded `[program, args...]`.
    pub fn key(program: &str, args: &[&str]) -> String {
        let mut parts = vec![program.to_owned()];
        parts.extend(args.iter().map(|arg| arg.to_string()));
        serde_json::to_string(&parts).expect("mock key serialization cannot fail")
    }

    /// Mocked output for the command, if registered.
    pub fn reply(&self, program: &str, args: &[&str]) -> Option<String> {
        self.replies.get(&Self::key(program, args)).cloned()
    }
}
