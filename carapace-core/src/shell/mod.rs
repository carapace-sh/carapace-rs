//! Per-shell output formatting and snippet generation.
//!
//! Ports Go carapace's `internal/shell` packages. Currently implemented:
//! bash (full value formatting and snippet) and the export JSON format.

pub mod bash;

use crate::meta::Meta;
use crate::value::RawValues;

/// A target shell for completion output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shell {
    Bash,
    Export,
}

impl Shell {
    /// Parse a shell name as used in `_carapace <shell>` invocations.
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "bash" => Some(Self::Bash),
            "export" => Some(Self::Export),
            _ => None,
        }
    }

    /// Format invoked values for the shell.
    pub fn value(&self, current_word: &str, meta: &Meta, values: &RawValues) -> String {
        match self {
            Self::Bash => {
                bash::action_raw_values(current_word, meta, values, &bash::Context::default())
            }
            Self::Export => crate::export::Export::from_parts(meta, values).to_json(),
        }
    }

    /// Generate the shell integration snippet for a binary.
    pub fn snippet(&self, binary: &str) -> String {
        match self {
            Self::Bash => bash::snippet(binary),
            Self::Export => String::new(),
        }
    }
}
