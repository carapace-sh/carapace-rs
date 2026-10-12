//! Resolution of `$`-prefixed macros and tab-separated values.

use carapace_rs::Action;
use carapace_rs::RawValue;

/// Resolve a single completion entry into an action.
pub fn resolve(entry: &str) -> Action {
    if let Some(macro_entry) = entry.strip_prefix('$') {
        let (name, args) = split_macro(macro_entry);
        return match name {
            "files" => Action::files(parse_args(args)),
            "directories" => Action::directories(),
            "executables" => Action::executables(),
            other => Action::message(format!("unsupported macro: {other}")),
        };
    }
    Action::raw(vec![parse_value(entry)])
}

/// Parse a tab-separated value entry: `value\tdescription\tstyle`.
pub fn parse_value(entry: &str) -> RawValue {
    let mut fields = entry.split('\t');
    let value = fields.next().unwrap_or_default();
    let mut raw = RawValue::new(value.to_owned());
    if let Some(description) = fields.next() {
        raw = raw.description(description);
    }
    if let Some(style) = fields.next() {
        raw = raw.style(style);
    }
    raw
}

/// Apply a generic modifier entry to an action; unsupported modifiers pass through.
pub fn modifier(entry: &str) -> Option<Box<dyn Fn(Action) -> Action + Send + Sync>> {
    let macro_entry = entry.strip_prefix('$')?;
    let (name, args) = split_macro(macro_entry);
    match name {
        "chdir" => {
            let dir = args.trim_matches('(').trim_end_matches(')').to_owned();
            Some(Box::new(move |action| action.chdir(dir.clone())))
        }
        _ => None,
    }
}

/// Split `$name(args)` into name and args (empty parens for bare macros).
fn split_macro(entry: &str) -> (&str, &str) {
    match entry.find('(') {
        Some(index) => (&entry[..index], &entry[index..]),
        None => (entry, ""),
    }
}

/// Parse macro arguments like `[.go, .mod]` or `.go, .mod` into a list.
fn parse_args(args: &str) -> Vec<String> {
    let args = args
        .trim_start_matches('[')
        .trim_end_matches(']')
        .trim()
        .trim_start_matches('(')
        .trim_end_matches(')');
    if args.is_empty() {
        return Vec::new();
    }
    args.split(", ").map(|arg| arg.trim().to_owned()).collect()
}
