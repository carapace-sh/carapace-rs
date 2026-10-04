//! Framework-independent completion engine core for carapace.
//!
//! This crate contains the Action API, the argument model, the traversal
//! engine and per-shell formatters. Framework adapters (e.g. clap) live in
//! their own crates and produce the [`model::Command`] tree consumed here.

pub mod action;
pub mod context;
pub mod meta;
pub mod model;
pub mod value;

pub use action::{Action, InvokedAction};
pub use context::Context;
pub use meta::Meta;
pub use model::{ArgumentStyle, Command, Flag, Nargs, Positional};
pub use value::{RawValue, RawValues};
