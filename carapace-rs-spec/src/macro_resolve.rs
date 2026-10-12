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

/// Whether the entry names a generic modifier applied to the whole batch
/// (Go `action.Parse`'s modifier switch).
pub fn is_batch_modifier(entry: &str) -> bool {
    matches!(
        split_macro(entry.strip_prefix('$').unwrap_or(entry)).0,
        "chdir"
            | "filter"
            | "filterargs"
            | "list"
            | "multiparts"
            | "nospace"
            | "noprefix"
            | "prefix"
            | "retain"
            | "shift"
            | "split"
            | "splitp"
            | "suffix"
            | "suppress"
            | "style"
            | "tag"
            | "uniquelist"
            | "usage"
    )
}

/// Apply a generic modifier entry to an action; unsupported modifiers pass through.
pub fn modifier(entry: &str) -> Option<Box<dyn Fn(Action) -> Action + Send + Sync>> {
    let macro_entry = entry.strip_prefix('$')?;
    let (name, args) = split_macro(macro_entry);
    match name {
        "chdir" => {
            let dir = args.trim_matches(|c| c == '(' || c == ')').to_owned();
            Some(Box::new(move |action| match chdir_macro(&dir) {
                Some(chdir) => chdir(action),
                None => Action::message(format!("unknown macro: ${dir}")),
            }))
        }
        "filter" => {
            let values = parse_args(args);
            Some(Box::new(move |action| {
                action.filter(values.iter().cloned())
            }))
        }
        "filterargs" => Some(Box::new(move |action| action.filter_args())),
        "list" => {
            let divider = unparen(args);
            Some(Box::new(move |action| action.list(&divider)))
        }
        "multiparts" => {
            let dividers = std::sync::Arc::new(parse_args(args));
            Some(Box::new(move |action| {
                let dividers = dividers.clone();
                Action::callback(move |c| {
                    action
                        .clone()
                        .invoke(c)
                        .to_multiparts_a(&dividers.iter().map(String::as_str).collect::<Vec<_>>())
                })
            }))
        }
        "nospace" => {
            let suffixes: Vec<char> = unparen(args).chars().collect();
            Some(Box::new(move |action| {
                action.nospace(suffixes.iter().copied())
            }))
        }
        "noprefix" => {
            let prefixes: Vec<char> = unparen(args).chars().collect();
            Some(Box::new(move |action| {
                action.no_prefix(prefixes.iter().copied())
            }))
        }
        "prefix" => {
            let prefix = unparen(args);
            Some(Box::new(move |action| action.prefix(prefix.clone())))
        }
        "retain" => {
            let values = parse_args(args);
            Some(Box::new(move |action| {
                action.retain(values.iter().cloned())
            }))
        }
        "shift" => {
            let n = unparen(args).trim().parse::<usize>().unwrap_or_default();
            Some(Box::new(move |action| action.shift(n)))
        }
        "split" => {
            let format = unparen(args);
            Some(Box::new(move |action| match format_name(&format) {
                Some(format) => action.split(format),
                None => Action::message(format!("unknown format: {format}")),
            }))
        }
        "splitp" => {
            let format = unparen(args);
            Some(Box::new(move |action| match format_name(&format) {
                Some(format) => action.split_p(format),
                None => Action::message(format!("unknown format: {format}")),
            }))
        }
        "suffix" => {
            let suffix = unparen(args);
            Some(Box::new(move |action| action.suffix(suffix.clone())))
        }
        "suppress" => {
            let exprs = parse_args(args);
            Some(Box::new(move |action| {
                action.suppress(exprs.iter().cloned())
            }))
        }
        "style" => {
            let style = unparen(args);
            Some(Box::new(move |action| action.style(style.clone())))
        }
        "tag" => {
            let tag = unparen(args);
            Some(Box::new(move |action| action.tag(tag.clone())))
        }
        "uniquelist" => {
            let divider = unparen(args);
            Some(Box::new(move |action| action.unique_list(&divider)))
        }
        "usage" => {
            let usage = unparen(args);
            Some(Box::new(move |action| action.usage(usage.clone())))
        }
        _ => None,
    }
}

/// `$chdir` accepts a literal directory or a traverse macro (Go
/// `modifier.chdir`); only the engine-supported lookups are available.
fn chdir_macro(arg: &str) -> Option<Box<dyn Fn(Action) -> Action + Send + Sync>> {
    if !arg.starts_with('$') {
        let dir = arg.to_owned();
        return Some(Box::new(move |action| action.chdir(dir.clone())));
    }
    let (name, args) = split_macro(arg.trim_start_matches('$'));
    match name {
        "gitdir" => Some(Box::new(move |action| {
            action.chdir_f(|c| carapace_rs::traverse::git::git_dir(c).unwrap_or_default())
        })),
        "gitworktree" => Some(Box::new(move |action| {
            action.chdir_f(|c| carapace_rs::traverse::git::git_work_tree(c).unwrap_or_default())
        })),
        "parent" => {
            let names: Vec<String> = parse_args(args);
            Some(Box::new(move |action| {
                let names = names.clone();
                action.chdir_f(move |c| {
                    carapace_rs::traverse::git::parent(
                        c,
                        &names.iter().map(String::as_str).collect::<Vec<_>>(),
                    )
                    .unwrap_or_default()
                })
            }))
        }
        _ => None,
    }
}

fn format_name(name: &str) -> Option<carapace_rs::Format> {
    carapace_rs::Format::from_name(name.trim())
}

fn unparen(args: &str) -> String {
    args.trim_matches(|c| c == '(' || c == ')').to_owned()
}

/// Split `$name(args)` into name and args (empty parens for bare macros).
fn split_macro(entry: &str) -> (&str, &str) {
    match entry.find('(') {
        Some(index) => (&entry[..index], &entry[index..]),
        None => (entry, ""),
    }
}

/// Parse macro arguments like `[.go, .mod]`, `(a, b)` or `a` into a list.
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
