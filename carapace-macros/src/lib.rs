//! Expression wrappers for wiring carapace completers through clap derive.
//!
//! clap derive forwards unknown `#[arg(...)]` / `#[command(...)]` entries to
//! builder methods, so `add = <expr>` attaches an extension object to the
//! generated arg. These macros wrap a carapace [`Action`] expression in the
//! matching extension type:
//!
//! ```ignore
//! #[derive(Parser)]
//! struct Cli {
//!     #[arg(long, add = carapace_macros::completer!(Action::values(["dev", "prod"])))]
//!     profile: Option<String>,
//! }
//! ```
//!
//! The generated code references `carapace_clap::` directly, so the crate
//! using the macro must depend on `carapace-clap` as well.

use proc_macro::TokenStream;
use quote::quote;

/// Wrap an action expression in a `carapace_clap::ArgCompleter`.
#[proc_macro]
pub fn completer(input: TokenStream) -> TokenStream {
    let action = proc_macro2::TokenStream::from(input);
    quote! { carapace_clap::ArgCompleter::new(#action) }.into()
}

/// Wrap an action expression in a `carapace_clap::CommandCompleter`.
#[proc_macro]
pub fn command_completer(input: TokenStream) -> TokenStream {
    let action = proc_macro2::TokenStream::from(input);
    quote! { carapace_clap::CommandCompleter::new(#action) }.into()
}
