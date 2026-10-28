//! Zsh bridge: completion capture through a zpty using
//! `Valodim/zsh-capture-completion` (vendored under `third_party/`),
//! with a custom `.zshrc` under `~/.config/carapace/bridge/zsh`.

use carapace_rs::Action;

const ZSH_SCRIPT: &str = include_str!("../third_party/zsh-capture-completion/capture.zsh");

/// Bridge completions registered in zsh.
pub fn action<I, S>(command: I) -> Action
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
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
            match context.execute("zsh", args.iter().copied()) {
                Ok(output) => {
                    let text = String::from_utf8_lossy(&output);
                    let mut lines: Vec<&str> = text.split("\r\n").collect();
                    if lines.last() == Some(&"") {
                        lines.pop();
                    }
                    let mut values = Vec::new();
                    for line in lines {
                        let line = unquote(line);
                        // strip ANSI from the value only; the description
                        // is emitted verbatim (Go `stripansi.Strip(splitted[0])`)
                        match line.split_once(" -- ") {
                            Some((value, description)) => values.push(
                                carapace_rs::RawValue::described(strip_ansi(value), description),
                            ),
                            None => {
                                values.push(carapace_rs::RawValue::described(strip_ansi(&line), ""))
                            }
                        }
                    }
                    Action::raw(values)
                        .style_f(|context, value| {
                            carapace_rs::style::for_path(context, &value.value)
                        })
                        .nospace(['/', '=', '@', ':', '.', ','])
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

/// Strip ANSI escape sequences (port of Go `stripansi.Strip`): CSI
/// sequences (`ESC [ params final-byte`) and OSC sequences (`ESC ] ...
/// BEL` or `ESC \`).
fn strip_ansi(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(pos) = rest.find('\x1b') {
        out.push_str(&rest[..pos]);
        rest = &rest[pos + 1..];
        let mut chars = rest.chars();
        match chars.next() {
            Some('[') => {
                let consumed = rest[1..]
                    .char_indices()
                    .find(|(_, c)| !c.is_ascii_digit() && *c != ';' && *c != ':');
                match consumed {
                    // the final byte terminates the sequence
                    Some((index, _)) => rest = &rest[1 + index + 1..],
                    None => {
                        out.push('\x1b');
                        rest = "";
                    }
                }
            }
            Some(']') => {
                // terminated by BEL or ST (ESC \)
                match rest.find('\x07') {
                    Some(end) => rest = &rest[end + 1..],
                    None => match rest.find("\x1b\\") {
                        Some(end) => rest = &rest[end + 2..],
                        None => {
                            out.push('\x1b');
                            rest = "";
                        }
                    },
                }
            }
            _ => {
                out.push('\x1b');
            }
        }
    }
    out.push_str(rest);
    out
}
