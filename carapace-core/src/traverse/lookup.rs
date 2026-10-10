//! Lenient flag lookup and argument classification (port of pflagfork `LookupArg`).
//!
//! Only the POSIX lookup path is ported: clap flags are POSIX (`-f`, `--flag`,
//! `-fvalue`, `-f=value`, `--flag=value`). The flag-prefix character comes from
//! the command so custom prefixes keep working.

use crate::model::{Command, Flag};

/// A flag matched against a command-line argument.
#[derive(Clone, Debug)]
pub struct MatchedFlag {
    /// The matched flag definition.
    pub flag: Flag,
    /// The matched prefix including delimiters, e.g. `--verbose=` or `-v`.
    pub arg_prefix: String,
    /// Values already attached to this occurrence.
    pub args: Vec<String>,
}

impl MatchedFlag {
    /// Whether this flag still consumes `arg` as an additional value.
    pub fn consumes(&self, arg: &str) -> bool {
        self.flag.consumes(self.args.len(), arg)
    }

    pub fn accepts_next(&self) -> bool {
        self.flag.argument_style.accepts_next
    }

    pub fn accepts_attached(&self) -> bool {
        self.flag.argument_style.accepts_attached
    }

    pub fn is_optarg(&self) -> bool {
        self.flag.is_optarg()
    }

    /// A control-character delimiter disables it, enabling directly attached values.
    pub fn delimiter_disabled(&self) -> bool {
        !self.flag.delimiter_enabled()
    }
}

/// Look up the flag referenced by `arg` (e.g. `--verbose=tr`, `-vf`, `-v=tr`).
pub fn lookup_arg(cmd: &Command, arg: &str) -> Option<MatchedFlag> {
    let prefix = cmd.flag_prefix;
    let double_prefix: String = [prefix, prefix].iter().collect();
    if arg.starts_with(&double_prefix) {
        return lookup_longhand(cmd, arg, prefix);
    }
    if arg.starts_with(prefix) {
        return lookup_shorthand_series(cmd, arg, prefix);
    }
    None
}

fn lookup_longhand(cmd: &Command, arg: &str, prefix: char) -> Option<MatchedFlag> {
    let body = &arg[prefix.len_utf8() * 2..];
    let (name, attached) = match body.find('=') {
        Some(index) => (&body[..index], Some(body[index + 1..].to_owned())),
        None => (body, None),
    };
    let flag = cmd.lookup_long(name)?;
    match attached {
        Some(value) => {
            if !flag.takes_value || flag.argument_style.accepts_delimited {
                // the prefix is the flag name including the delimiter
                // (Go SplitAfterN), not the full argument with its value
                Some(MatchedFlag {
                    flag: flag.clone(),
                    arg_prefix: format!("{}{}{}=", prefix, prefix, name),
                    args: vec![value],
                })
            } else {
                None
            }
        }
        None => {
            if flag.takes_value && !flag.is_optarg() && !flag.argument_style.accepts_next {
                return None;
            }
            Some(MatchedFlag {
                flag: flag.clone(),
                arg_prefix: arg.to_owned(),
                args: Vec::new(),
            })
        }
    }
}

fn lookup_shorthand_series(cmd: &Command, arg: &str, prefix: char) -> Option<MatchedFlag> {
    let series: Vec<char> = arg.chars().skip(prefix.len_utf8()).collect();
    if series.is_empty() {
        return None;
    }
    let mut consumed: String = prefix.to_string();
    for (position, shorthand) in series.iter().copied().enumerate() {
        let flag = cmd.lookup_short(shorthand)?;
        consumed.push(shorthand);
        let rest: String = series[position + 1..].iter().collect();
        if !flag.takes_value {
            if rest.is_empty() {
                return Some(MatchedFlag {
                    flag: flag.clone(),
                    arg_prefix: arg.to_owned(),
                    args: Vec::new(),
                });
            }
            if rest.starts_with('=') && flag.argument_style.accepts_delimited {
                return Some(MatchedFlag {
                    flag: flag.clone(),
                    arg_prefix: format!("{consumed}="),
                    args: vec![rest.strip_prefix('=').unwrap_or(&rest).to_owned()],
                });
            }
            continue;
        }
        if rest.is_empty() {
            if !flag.argument_style.accepts_next {
                return None;
            }
            return Some(MatchedFlag {
                flag: flag.clone(),
                arg_prefix: consumed,
                args: Vec::new(),
            });
        }
        if rest.starts_with('=') {
            if !flag.argument_style.accepts_delimited {
                return None;
            }
            return Some(MatchedFlag {
                flag: flag.clone(),
                arg_prefix: format!("{consumed}="),
                args: vec![rest.strip_prefix('=').unwrap_or(&rest).to_owned()],
            });
        }
        let attached_allowed =
            flag.argument_style.accepts_attached || (flag.is_optarg() && !flag.delimiter_enabled());
        if attached_allowed {
            return Some(MatchedFlag {
                flag: flag.clone(),
                arg_prefix: consumed,
                args: vec![rest],
            });
        }
        return None;
    }
    unreachable!("loop returns for every position")
}
