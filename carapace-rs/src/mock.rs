//! Sandbox mock state (port of Go `pkg/mock`), carried on the `Context`.
//!
//! Go injects the mock via the `CARAPACE_SANDBOX` environment variable
//! because its sandbox runs in a subprocess; the Rust testkit is in-process,
//! so the mock travels on the context instead.

use std::collections::HashMap;
use std::path::PathBuf;

/// Mock context for sandbox tests: work directory root and command replies.
#[derive(Clone, Debug, Default)]
pub struct Mock {
    /// Sandbox root directory (the cache lives in `<dir>/cache`).
    pub dir: PathBuf,
    /// Mocked command output keyed by [`Mock::key`].
    pub replies: HashMap<String, String>,
}

impl Mock {
    /// Key for a command invocation: JSON-encoded `[program, args...]`.
    pub fn key(program: &str, args: &[String]) -> String {
        let mut parts = vec![program.to_owned()];
        parts.extend(args.iter().cloned());
        serde_json::to_string(&parts).expect("mock key serialization cannot fail")
    }

    /// Mocked output for the command, if registered.
    pub fn reply(&self, program: &str, args: &[String]) -> Option<String> {
        self.replies.get(&Self::key(program, args)).cloned()
    }
}
