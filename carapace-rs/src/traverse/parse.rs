//! Lenient argument parsing over the normalized model.
//!
//! Replaces cobra's `ParseFlags` during traversal: unknown flags are ignored,
//! flags consume values per their [`Nargs`](crate::model::Nargs), and
//! positional arguments (including everything after `--`) are collected.

use crate::model::Command;
use crate::traverse::lookup::lookup_arg;

/// Result of leniently parsing a word list against a command.
#[derive(Clone, Debug, Default)]
pub struct Parsed {
    /// Positional arguments in order.
    pub positionals: Vec<String>,
    /// Consumed flag values keyed by flag id, in order of occurrence.
    pub flags: Vec<(String, Vec<String>)>,
    /// Number of positionals encountered before `--`.
    pub args_len_at_dash: usize,
    /// Whether `--` was encountered.
    pub dash: bool,
}

impl Parsed {
    /// Positionals after `--` (empty when no dash occurred).
    pub fn dash_positionals(&self) -> &[String] {
        if self.dash {
            &self.positionals[self.args_len_at_dash..]
        } else {
            &[]
        }
    }
}

/// Parse `args` leniently against `cmd`.
pub fn parse(cmd: &Command, args: &[String]) -> Parsed {
    let mut parsed = Parsed::default();
    let mut iter = args.iter().enumerate().peekable();
    while let Some((_, arg)) = iter.next() {
        if parsed.dash {
            parsed.positionals.push(arg.clone());
            continue;
        }
        if arg.as_str() == "--" {
            parsed.dash = true;
            parsed.args_len_at_dash = parsed.positionals.len();
            continue;
        }
        if arg.starts_with(cmd.flag_prefix) {
            if let Some(matched) = lookup_arg(cmd, arg) {
                let mut values = matched.args.clone();
                let missing = missing_values(&matched.flag, &values);
                if missing > 0 {
                    // fixed nargs consume unconditionally (pflag behavior);
                    // variadic nargs stop at anything that looks like a flag
                    let unconditional = matches!(matched.flag.nargs, crate::model::Nargs::Exact(_));
                    for _ in 0..missing {
                        match iter.peek().map(|(_, next)| next.to_string()) {
                            Some(next)
                                if unconditional
                                    || matched.flag.allow_hyphen_values
                                    || !next.starts_with(cmd.flag_prefix) =>
                            {
                                values.push(next);
                                iter.next();
                            }
                            _ => break,
                        }
                    }
                }
                // a shorthand series (`-abc`) registers every member as
                // given (pflag `Changed` semantics), not just the terminal
                // flag; longhands are not series (the value `--output=dev`
                // must not register a shorthand for its last rune)
                let double_prefix: String = [cmd.flag_prefix, cmd.flag_prefix].iter().collect();
                let is_series = arg.starts_with(cmd.flag_prefix)
                    && !arg.starts_with(&double_prefix)
                    && arg.chars().nth(1).is_some();
                if is_series {
                    let series: Vec<char> = arg.chars().skip(cmd.flag_prefix.len_utf8()).collect();
                    for shorthand in &series {
                        if let Some(member) = cmd.lookup_short(*shorthand) {
                            if member.id != matched.flag.id {
                                parsed.flags.push((member.id.clone(), Vec::new()));
                            }
                        }
                    }
                }
                parsed.flags.push((matched.flag.id.clone(), values));
                continue;
            }
            // unknown flag: leniently skip
            continue;
        }
        if !cmd.interspersed {
            // pflag's non-interspersed mode: the first positional ends
            // flag parsing, everything after (including flag-lookalikes
            // and `--`) is a positional argument
            parsed.positionals.push(arg.clone());
            for rest in iter {
                parsed.positionals.push(rest.1.clone());
            }
            break;
        }
        parsed.positionals.push(arg.clone());
    }
    parsed
}

fn missing_values(flag: &crate::model::Flag, values: &[String]) -> usize {
    use crate::model::Nargs;
    if !flag.takes_value || flag.is_optarg() || !values.is_empty() {
        return 0;
    }
    match flag.nargs {
        Nargs::Exact(n) => n,
        Nargs::AtLeastOne | Nargs::Any => 1,
    }
}
