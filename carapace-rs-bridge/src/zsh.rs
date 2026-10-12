//! Zsh bridge: completion capture through a zpty using
//! `Valodim/zsh-capture-completion` (vendored under `third_party/`),
//! with a custom `.zshrc` under `~/.config/carapace/bridge/zsh`.

use carapace_rs::Action;

const ZSH_SCRIPT: &str = include_str!("../third_party/zsh-capture-completion/capture.zsh");

/// Bridge completions registered in zsh.
pub fn action(command: &[&str]) -> Action {
    super::action_command(command, move |command| {
        let command: Vec<String> = command.to_vec();
        Action::callback(move |c| {
            let mut args = vec!["--no-rcs", "-c", ZSH_SCRIPT, "--"];
            args.extend(command.iter().map(String::as_str));
            args.extend(c.args.iter().map(String::as_str));
            args.push(&c.value);

            let Some(config_dir) = super::config_dir(c) else {
                return Action::message("failed to resolve config dir");
            };
            let rc_path = config_dir.join("carapace/bridge/zsh/.zshrc");
            if let Err(error) = super::ensure_exists(&rc_path) {
                return Action::message(error);
            }

            let mut context = c.clone();
            context.env.insert(
                "CARAPACE_BRIDGE_CONFIG_HOME".to_owned(),
                config_dir.to_string_lossy().into_owned(),
            );
            match context.execute("zsh", &args) {
                Ok(output) => {
                    let text = String::from_utf8_lossy(&output);
                    let mut lines: Vec<&str> = text.split("\r\n").collect();
                    if lines.last() == Some(&"") {
                        lines.pop();
                    }
                    let mut values = Vec::new();
                    for line in lines {
                        let line = strip_ansi(&unquote(line));
                        match line.split_once(" -- ") {
                            Some((value, description)) => {
                                values.push(carapace_rs::RawValue::described(value, description));
                            }
                            None => values.push(carapace_rs::RawValue::described(line, "")),
                        }
                    }
                    Action::raw(values).nospace(['/', '=', '@', ':', '.', ','])
                }
                Err(error) => Action::message(error),
            }
        })
    })
}

/// Resolve zsh's compadd backslash quoting (port of Go `unquoter`).
fn unquote(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some(
                    escaped @ ('\\' | '&' | '<' | '>' | '`' | '\'' | '"' | '{' | '}' | '$' | '#'
                    | '|' | '?' | '(' | ')' | ';' | ' ' | '[' | ']' | '*' | '~'),
                ) => out.push(escaped),
                Some(other) => {
                    out.push(c);
                    out.push(other);
                }
                None => out.push(c),
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Strip ANSI SGR sequences (port of Go `stripansi.Strip`).
fn strip_ansi(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(pos) = rest.find('\x1b') {
        out.push_str(&rest[..pos]);
        rest = &rest[pos + 1..];
        if let Some(sequence) = rest.strip_prefix('[').and_then(|s| s.split_once('m')) {
            rest = sequence.1;
        } else {
            out.push('\x1b');
        }
    }
    out.push_str(rest);
    out
}
