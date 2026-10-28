/// Resolution of `$`-prefixed macros and tab-separated values.
use carapace_rs::Action;
use carapace_rs::RawValue;

/// Resolve a single completion entry into an action.
pub fn resolve(entry: &str) -> Action {
    let Some(_) = entry.strip_prefix('$') else {
        return Action::raw(vec![parse_value(entry)]);
    };

    // the own-executable prefix (`$myexe.files(...)`) is the internal
    // macro namespace (Go `ActionMacro`'s `prefix` handling)
    let mut entry = entry.to_owned();
    if let Some(exe) = std::env::current_exe().ok().and_then(|path| {
        path.file_name()
            .map(|name| name.to_string_lossy().into_owned())
    }) {
        let prefix = format!("${exe}.");
        if let Some(rest) = entry.strip_prefix(&prefix) {
            entry = format!("$_{rest}");
        }
    }

    let macro_entry = &entry[1..];
    let (name, args) = split_macro(macro_entry);

    if name.starts_with('_') && !name.starts_with("_.") {
        return Action::message(format!(
            "\"$_\" deprecated: replace ${name} with $carapace.{}",
            name.strip_prefix('_').unwrap_or(name)
        ));
    }

    // a dotted macro belongs to another binary (Go's external macro
    // protocol): `<binary> _carapace macro <macro> <args...> <value>`
    if !name.starts_with("_.") && name.contains('.') {
        let (binary, macro_name) = macro_entry.split_once('.').unwrap();
        let (binary, macro_name) = (binary.to_owned(), macro_name.to_owned());
        return Action::callback(move |c| {
            let mut remote_args = vec![
                "_carapace".to_owned(),
                "macro".to_owned(),
                macro_name.clone(),
            ];
            remote_args.extend(c.args.iter().cloned());
            remote_args.push(c.value.clone());
            match c.execute(&binary, &remote_args) {
                Ok(output) => Action::import(&String::from_utf8_lossy(&output)),
                Err(error) => Action::message(error),
            }
        });
    }

    // internal macros accept the `_.` namespace prefix (Go `macros.Lookup`)
    let name = name.strip_prefix("_.").unwrap_or(name);
    match name {
        "files" => Action::files_with_suffixes(parse_args(args)),
        "directories" => Action::directories(),
        "executables" => Action::executables(),
        "message" => Action::message(full_arg(args)),
        "spec" => spec_action(&full_arg(args)),
        "" | "bash" | "cmd" | "elvish" | "fish" | "nu" | "osh" | "pwsh" | "sh" | "xonsh"
        | "zsh" => {
            let shell = if name.is_empty() && cfg!(windows) {
                "cmd"
            } else if name.is_empty() {
                "sh"
            } else {
                name
            };
            shell_macro(shell, &full_arg(args))
        }
        // registered as dummies in Go's core.go (`for completeness`):
        // bare usage completes nothing instead of erroring; the real
        // modifier forms are applied through `modifier`/batch handling
        "chdir" | "list" | "multiparts" | "nospace" | "noprefix" | "uniquelist" => {
            Action::values([])
        }
        other => Action::message(format!("unsupported macro: {other}")),
    }
}

/// The full macro argument (Go `MacroI` with a string parameter receives
/// everything between the outer parentheses).
fn full_arg(args: &str) -> String {
    args.trim_start_matches('(')
        .trim_end_matches(')')
        .to_owned()
}

/// `$spec(path)`: load and execute a spec file (Go `ActionSpec`).
fn spec_action(path: &str) -> Action {
    let path = path.to_owned();
    Action::callback(move |c| {
        let abs = match c.abs(&path) {
            Ok(abs) => abs,
            Err(error) => return Action::message(error),
        };
        match std::fs::read_to_string(&abs) {
            Ok(content) => match crate::from_yaml(&content) {
                Ok(command) => Action::execute(std::sync::Arc::new(command)),
                Err(error) => Action::message(error),
            },
            Err(error) => Action::message(error.to_string()),
        }
    })
}

/// Shell macros (`$sh(...)`, `$bash(...)`, ...): run the command with the
/// positional arguments appended and complete each output line as a value
/// (Go `core.go`'s `shell` helper).
fn shell_macro(shell: &str, command: &str) -> Action {
    let shell = shell.to_owned();
    let command = command.to_owned();
    Action::callback(move |c| {
        let arguments: Vec<String> = c.args.clone();
        let args = match shell_args(&shell, &command, &arguments) {
            Ok(args) => args,
            Err(error) => return Action::message(error),
        };
        match c.execute(&shell, &args) {
            Ok(output) => {
                let text = String::from_utf8_lossy(&output);
                let values: Vec<RawValue> = text
                    .lines()
                    .filter(|line| !line.is_empty())
                    .map(parse_value)
                    .collect();
                Action::raw(values)
            }
            Err(error) => Action::message(error),
        }
    })
}

/// Build the interpreter invocation for a shell macro (Go `shellArgs`).
fn shell_args(shell: &str, command: &str, arguments: &[String]) -> Result<Vec<String>, String> {
    #[cfg(windows)]
    if !matches!(shell, "cmd" | "elvish" | "nu" | "pwsh" | "xonsh") {
        return Err(format!("unsupported shell [windows]: {shell}"));
    }
    let mut args = vec!["-c".to_owned()];
    match shell {
        "cmd" => {
            args[0] = "/c".to_owned();
            args.push(command.to_owned());
            args.extend(arguments.iter().cloned());
        }
        "nu" => {
            args.push(format!(
                "def --wrapped main [...args] {{ {command} }}; main {}",
                carapace_rs_shlex::join(arguments, carapace_rs_shlex::Format::Nushell)
            ));
        }
        "pwsh" => {
            args.push(command.to_owned());
            args.extend(arguments.iter().cloned());
        }
        _ => {
            args.push(command.to_owned());
            args.push("--".to_owned());
            args.extend(arguments.iter().cloned());
        }
    }
    Ok(args)
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
                action.filter_with(values.iter().cloned())
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
                action.retain_with(values.iter().cloned())
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
                action.suppress_with(exprs.iter().cloned())
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
