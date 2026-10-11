//! Framework-independent completion engine core for carapace.
//!
//! This crate contains the Action API, the argument model, the traversal
//! engine and per-shell formatters. Framework adapters (e.g. clap) live in
//! their own crates and produce the [`model::Command`] tree consumed here.

pub mod action;
// Internal machinery (Go's `internal/` packages): reachable for adapters and
// the test harness, but not part of the stable API.
#[doc(hidden)]
pub mod cache;
#[doc(hidden)]
pub mod config;
pub mod context;
pub mod export;
#[doc(hidden)]
pub mod log;
#[doc(hidden)]
pub mod meta;
#[doc(hidden)]
pub mod mock;
pub mod model;
pub mod shell;
pub mod traverse;
#[doc(hidden)]
pub mod uid;
pub mod value;

pub use action::{Action, InvokedAction, batch};
pub use carapace_shlex::Format;
pub use context::Context;
pub use meta::Meta;
pub use model::{ArgumentStyle, Command, Flag, Nargs, Positional};
pub use traverse::{Completion, traverse};
pub use value::{RawValue, RawValues};
