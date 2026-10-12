//! `compat` command (Go `example/cmd/compat.go`).
//!
//! The Go source wires `RegisterFlagCompletionFunc` and
//! `ValidArgsFunction` — cobra's completion protocol. In carapace-rs those
//! directive semantics live in the cobra bridge (`carapace-rs-bridge`)
//! and are tested there; the command itself only contributes its name to
//! the root's subcommand completion.

use clap::Command;

pub fn cmd() -> Command {
    Command::new("compat")
}
