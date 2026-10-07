//! The completion engine: argument traversal and completion resolution.
//!
//! Direct port of Go carapace's `traverse.go` state machine over the
//! normalized [`Command`](crate::model::Command) model.

mod lookup;
pub mod parse;

use crate::action::merge_all;
use crate::context::Context;
use crate::model::Command;
use crate::value::RawValue;
use crate::{Action, RawValues};

pub use lookup::MatchedFlag;

/// The result of traversing a command line: the resolved action and its context.
#[derive(Clone, Debug)]
pub struct Completion {
    pub action: Action,
    pub context: Context,
}

/// Traverse the command line (everything after the binary name) and resolve
/// the action completing the current value.
pub fn traverse(root: &Command, args: Vec<String>) -> Completion {
    traverse_command(root, args, Vec::new())
}

fn traverse_command(cmd: &Command, args: Vec<String>, _path: Vec<String>) -> Completion {
    let mut context = Context::new(args.clone());

    let mut in_args: Vec<String> = Vec::new();
    let mut in_positionals: Vec<String> = Vec::new();
    let mut in_flag: Option<MatchedFlag> = None;
    let mut dash = false;

    for (index, arg) in context.args.clone().into_iter().enumerate() {
        if let Some(flag) = in_flag.as_ref() {
            if flag.consumes(&arg) {
                in_args.push(arg.clone());
                let flag = in_flag.as_mut().unwrap();
                flag.args.push(arg);
                if !flag.consumes("") {
                    in_flag = None;
                }
                continue;
            }
        }

        let double_prefix: String = [cmd.flag_prefix, cmd.flag_prefix].iter().collect();
        if arg == double_prefix {
            in_args.extend(context.args[index..].to_vec());
            dash = true;
            break;
        }

        let starts_with_prefix = arg.starts_with(cmd.flag_prefix);
        if !cmd.disable_flag_parsing
            && starts_with_prefix
            && (cmd.interspersed || in_positionals.is_empty())
        {
            in_args.push(arg.clone());
            in_flag = lookup::lookup_arg(cmd, &arg);
            continue;
        }

        if let Some(subcommand) = cmd.lookup_subcommand(&arg) {
            // recurse with everything after the subcommand, including the
            // value being completed as the last word (Go passes args[i+1:])
            if !cmd.disable_flag_parsing {
                let parsed = parse::parse(cmd, &in_args);
                context.args = parsed.positionals;
            }
            return traverse_command(subcommand, args[index + 1..].to_vec(), Vec::new());
        }

        in_args.push(arg.clone());
        in_positionals.push(arg);
    }

    let parsed = if cmd.disable_flag_parsing {
        if dash {
            // everything after the dash belongs to the parent's args
        } else {
            in_args.clear();
        }
        parse::Parsed::default()
    } else {
        let mut to_parse = in_args.clone();
        if !dash {
            if let Some(flag) = in_flag.as_ref() {
                if flag.args.is_empty() && flag.consumes("") {
                    to_parse.pop();
                }
            }
        }
        let parsed = parse::parse(cmd, &to_parse);
        context.args = parsed.positionals.clone();
        parsed
    };

    // dash argument
    if dash {
        context.args = parsed.dash_positionals().to_vec();
        let action = cmd
            .dash_positionals
            .get(context.args.len())
            .and_then(|positional| positional.completion.clone())
            .or_else(|| cmd.dash_any.clone())
            .unwrap_or_else(|| get_positional(cmd, context.args.len()));
        return Completion { action, context };
    }

    // flag argument
    if let Some(flag) = in_flag.as_ref() {
        if flag.consumes(&context.value) && flag.accepts_next() {
            context.parts = flag.args.clone();
            return Completion {
                action: get_flag(cmd, &flag.flag.name),
                context,
            };
        }
    }

    // flag
    let starts_with_prefix = context.value.starts_with(cmd.flag_prefix);
    if !cmd.disable_flag_parsing
        && starts_with_prefix
        && (cmd.interspersed || in_positionals.is_empty())
    {
        if let Some(flag) = lookup::lookup_arg(cmd, &context.value) {
            if !flag.args.is_empty() {
                if !flag.flag.takes_value {
                    return Completion {
                        action: Action::values(["true", "false"]).prefix(&flag.arg_prefix),
                        context,
                    };
                }
                return Completion {
                    action: get_flag(cmd, &flag.flag.name).prefix(&flag.arg_prefix),
                    context,
                };
            }
            let double_prefix: String = [cmd.flag_prefix, cmd.flag_prefix].iter().collect();
            let not_double_prefixed = !context.value.starts_with(&double_prefix);
            if not_double_prefixed
                && flag.accepts_attached()
                && !flag.is_optarg()
                && flag.arg_prefix == context.value
            {
                return Completion {
                    action: get_flag(cmd, &flag.flag.name).prefix(&flag.arg_prefix),
                    context,
                };
            }
            if !not_double_prefixed
                && flag.is_optarg()
                && flag.delimiter_disabled()
                && flag.accepts_attached()
                && flag.arg_prefix == context.value
            {
                return Completion {
                    action: get_flag(cmd, &flag.flag.name).prefix(&flag.arg_prefix),
                    context,
                };
            }
        }
        return Completion {
            action: action_flags(cmd),
            context,
        };
    }

    // positional or subcommand
    let positional = get_positional(cmd, context.args.len());
    if !cmd.subcommands.is_empty() && context.args.is_empty() {
        let action = merge_all([
            positional.invoke(&context),
            action_commands(cmd).invoke(&context),
        ]);
        return Completion {
            action: action.to_action(),
            context,
        };
    }
    Completion {
        action: positional,
        context,
    }
}

/// Completion for the flag value registered on the command (empty when unset).
pub fn get_flag(cmd: &Command, name: &str) -> Action {
    cmd.lookup_long(name)
        .and_then(|flag| flag.completion.clone())
        .unwrap_or_default()
}

/// Completion for the positional at `index` (falling back to any-positional).
pub fn get_positional(cmd: &Command, index: usize) -> Action {
    cmd.positional_at(index)
        .and_then(|positional| positional.completion.clone())
        .or_else(|| cmd.positional_any.clone())
        .unwrap_or_default()
}

/// Completion of all visible flags of the command.
pub fn action_flags(cmd: &Command) -> Action {
    let cmd = cmd.clone();
    let prefix = cmd.flag_prefix;
    let double_prefix: String = [prefix, prefix].iter().collect();
    Action::callback(move |c| {
        let value = &c.value;
        let is_shorthand_series =
            value.starts_with(prefix) && !value.starts_with(&double_prefix) && value.len() > 2;

        let mut values: RawValues = Vec::new();
        let include_hidden = crate::config::hidden() == crate::config::Hidden::IncludeCarapace;
        for flag in &cmd.flags {
            if flag.hidden && !include_hidden {
                continue;
            }
            let description = flag.description.clone();
            if is_shorthand_series {
                let Some(shorthand) = flag.shorthand else {
                    continue;
                };
                // abort if a previous shorthand in the series takes a value
                let series: Vec<char> = value.chars().skip(1).collect();
                let valid = series.iter().all(|shorthand| {
                    cmd.lookup_short(*shorthand)
                        .is_none_or(|f| !f.takes_value || f.is_optarg())
                });
                if !valid {
                    continue;
                }
                let mut value = RawValue::styled_described(shorthand.to_string(), description, "");
                value.tag = "shorthand flags".to_owned();
                if flag.is_optarg() {
                    value = value.nospace(true);
                }
                values.push(value);
            } else {
                let mut longhand = RawValue::styled_described(
                    format!("{double_prefix}{}", flag.name),
                    description.clone(),
                    "",
                );
                longhand.tag = "longhand flags".to_owned();
                values.push(longhand);
                if let Some(shorthand) = flag.shorthand {
                    let mut short =
                        RawValue::styled_described(format!("{prefix}{shorthand}"), description, "");
                    short.tag = "shorthand flags".to_owned();
                    values.push(short);
                }
            }
        }
        let action = Action::raw(values);
        if is_shorthand_series {
            action.prefix(value.clone())
        } else {
            action
        }
    })
}

/// Completion of the visible subcommands of the command.
pub fn action_commands(cmd: &Command) -> Action {
    let include_hidden = crate::config::hidden() == crate::config::Hidden::IncludeCarapace;
    let values: Vec<(String, String)> = cmd
        .subcommands
        .iter()
        .filter(|subcommand| include_hidden || !subcommand.hidden)
        .map(|subcommand| (subcommand.name.clone(), subcommand.description.clone()))
        .collect();
    Action::values_described(values).tag("commands")
}
