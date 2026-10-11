//! carapace shell completion for Rust CLIs.
//!
//! This crate is a facade over the workspace components: the framework-independent
//! engine ([`carapace-core`]), the [clap](https://github.com/clap-rs/clap) adapter
//! ([`carapace-clap`]) and the derive macros ([`carapace-macros`]). Depend on it
//! alone for the common case; the component crates remain available for direct use.
//!
//! [`carapace-core`]: https://docs.rs/carapace-core
//! [`carapace-clap`]: https://docs.rs/carapace-clap
//! [`carapace-macros`]: https://docs.rs/carapace-macros
//!
//! # Example
//!
//! ```
//! use carapace_rs::Action;
//! use clap::{CommandFactory, Parser};
//!
//! #[derive(Parser)]
//! #[command(name = "demo")]
//! struct Cli {
//!     /// the deployment profile
//!     #[arg(long, add = carapace_rs::completer!(Action::values(["dev", "prod"])))]
//!     profile: Option<String>,
//! }
//!
//! let _cmd = Cli::command();
//! ```

pub use carapace_clap::{
    ArgCompleter, Carapace, CommandCompleter, DashAnyCompleter, DashCompleter, FlagCompleter,
    PositionalCompleter, complete, complete_output, dash_completion, execute_embedded,
    flag_completion, positional_completion, snippet, to_model,
};
pub use carapace_core::{Action, Context, InvokedAction};
pub use carapace_macros::{command_completer, completer};
