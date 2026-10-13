//! Custom in-process macros (port of Go `AddMacro`/`MacroMap`).
//!
//! A binary registers macros at startup with [`add_macro`]; specs invoke
//! them as `$<executable>.<name>(args)` (or `$carapace.<name>(args)` for
//! carapace-bin style), which routes through the `_carapace macro`
//! subcommand of the registering binary and comes back as export JSON.

use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

use carapace_rs::Action;

/// The macro function: receives the positional arguments and the value
/// being completed (the mini command's context, Go `ActionMacro`'s
/// callback context).
pub type MacroFn = Arc<dyn Fn(&[String], &str) -> Action + Send + Sync>;

/// A registered custom macro (Go `spec.Macro`).
#[derive(Clone)]
pub struct Macro {
    /// The macro name as invoked after the executable prefix
    /// (`$<exe>.<name>(...)`).
    pub name: String,
    /// Short description (for macro listing).
    pub description: String,
    /// Usage example (for macro listing).
    pub example: String,
    f: MacroFn,
}

impl Macro {
    /// Invoke the macro with the positional arguments and the current value.
    pub fn invoke(&self, args: &[String], value: &str) -> Action {
        (self.f)(args, value)
    }
}

/// Registered custom macros keyed by name (Go `spec.MacroMap`).
#[derive(Default)]
pub struct MacroMap {
    macros: BTreeMap<String, Macro>,
}

impl MacroMap {
    /// Create an empty map.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a macro (Go `AddMacro`; the name-inference of
    /// `AddMacroI`/`AddMacroV` is a Go reflection trick — Rust closures
    /// are unnamed, so the name is always explicit).
    pub fn add<F>(&mut self, name: impl Into<String>, description: &str, example: &str, f: F)
    where
        F: Fn(&[String], &str) -> Action + Send + Sync + 'static,
    {
        self.macros.insert(
            name.into(),
            Macro {
                name: String::new(),
                description: description.to_owned(),
                example: example.to_owned(),
                f: Arc::new(f),
            },
        );
    }

    /// The registered macro, if any.
    pub fn get(&self, name: &str) -> Option<&Macro> {
        self.macros.get(name)
    }

    /// The registered macro names in sorted order.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.macros.keys().map(String::as_str)
    }
}

static REGISTRY: std::sync::OnceLock<RwLock<MacroMap>> = std::sync::OnceLock::new();

fn registry() -> &'static RwLock<MacroMap> {
    REGISTRY.get_or_init(|| RwLock::new(MacroMap::new()))
}

/// Register a macro in the process-wide registry (Go `AddMacro`).
pub fn add_macro<F>(name: impl Into<String>, description: &str, example: &str, f: F)
where
    F: Fn(&[String], &str) -> Action + Send + Sync + 'static,
{
    let name = name.into();
    let mut guard = registry()
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.macros.insert(
        name.clone(),
        Macro {
            name,
            description: description.to_owned(),
            example: example.to_owned(),
            f: Arc::new(f),
        },
    );
}

/// Invoke a registered macro by name; `None` when unknown.
pub fn invoke_macro(name: &str, args: &[String], value: &str) -> Option<Action> {
    let guard = registry()
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.get(name).map(|macro_| macro_.invoke(args, value))
}

/// The registered macro names in sorted order.
pub fn macro_names() -> Vec<String> {
    let guard = registry()
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.names().map(str::to_owned).collect()
}

/// The registered macro's description (for the `_carapace macro` listing).
pub fn macro_description(name: &str) -> Option<String> {
    let guard = registry()
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.get(name).map(|m| m.description.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_and_invoke_macro() {
        add_macro(
            "test.echo",
            "echoes the args",
            "$test.echo(a, b)",
            |args, value| {
                let mut words: Vec<String> = args.to_vec();
                words.push(value.to_owned());
                Action::message(words.join(","))
            },
        );

        assert!(macro_names().contains(&"test.echo".to_owned()));
        let action = invoke_macro("test.echo", &["a".to_owned()], "v").unwrap();
        let invoked = action.invoke(&carapace_rs::Context::default());
        assert_eq!(invoked.meta.messages, vec!["a,v"]);
        assert!(invoke_macro("nosuch", &[], "").is_none());
    }
}
