//! The completion engine: argument traversal and completion resolution.
//!
//! Direct port of Go carapace's `traverse.go` state machine over the
//! normalized [`Command`](crate::model::Command) model.

mod lookup;
pub mod parse;

use crate::action::merge_all;
use crate::context::Context;
use crate::model::{Command, Nargs};
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
    traverse_command(root, args, vec![root.name.clone()])
}

fn traverse_command(cmd: &Command, args: Vec<String>, path: Vec<String>) -> Completion {
    crate::log::log(&format!(
        "traverse called for {:?} with args {args:?}",
        cmd.name
    ));
    let mut context = Context::new(args.clone());

    let mut in_args: Vec<String> = Vec::new();
    let mut in_positionals: Vec<String> = Vec::new();
    let mut in_flag: Option<MatchedFlag> = None;
    let mut dash = false;

    for (index, arg) in context.args.clone().into_iter().enumerate() {
        if let Some(flag) = in_flag.as_ref() {
            if flag.consumes(&arg) {
                crate::log::log(&format!("arg {arg:?} is a flag argument"));
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
            crate::log::log(&format!("arg {arg:?} is dash"));
            in_args.extend(context.args[index..].to_vec());
            dash = true;
            break;
        }

        let starts_with_prefix = arg.starts_with(cmd.flag_prefix);
        if !cmd.disable_flag_parsing
            && starts_with_prefix
            && (cmd.interspersed || in_positionals.is_empty())
        {
            crate::log::log(&format!("arg {arg:?} is a flag"));
            let matched = lookup::lookup_arg(cmd, &arg);
            if matched.is_none() {
                crate::log::log(&format!("flag {arg:?} is unknown"));
            }
            in_args.push(arg.clone());
            in_flag = matched;
            continue;
        }

        if let Some(subcommand) = cmd.lookup_subcommand(&arg) {
            crate::log::log(&format!("arg {arg:?} is a subcommand"));
            // recurse with everything after the subcommand, including the
            // value being completed as the last word (Go passes args[i+1:]);
            // the uid path uses the subcommand's actual name (Go
            // `uid.Command` builds from `cmd.Name()`, not the typed alias)
            let mut sub_path = path.clone();
            sub_path.push(subcommand.name.clone());
            return traverse_command(subcommand, args[index + 1..].to_vec(), sub_path);
        }

        crate::log::log(&format!("arg {arg:?} is a positional"));
        in_args.push(arg.clone());
        in_positionals.push(arg);
    }

    let parsed = if cmd.disable_flag_parsing {
        crate::log::log(&format!("flag parsing disabled for {:?}", cmd.name));
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
                        // Go styles bool values as keywords
                        action: Action::values(["true", "false"])
                            .style_f(|value| crate::config::keyword_style(&value.value))
                            .prefix(&flag.arg_prefix),
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
        }
        return Completion {
            action: action_flags(cmd, &path),
            context,
        };
    }

    // positional or subcommand
    let positional = get_positional(cmd, context.args.len());
    if !cmd.subcommands.is_empty() && context.args.is_empty() {
        let action = merge_all([
            positional.invoke(&context),
            action_commands(cmd, &path).invoke(&context),
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
///
/// Each candidate carries the implicit flag uid `cmd://<root>/<path>?flag=<name>`
/// (Go `actionFlags`' `UidF(uid.Flag(cmd, f))`).
pub fn action_flags(cmd: &Command, path: &[String]) -> Action {
    let cmd = cmd.clone();
    let prefix = cmd.flag_prefix;
    let double_prefix: String = [prefix, prefix].iter().collect();
    let root = path.first().map(String::as_str).unwrap_or("").to_owned();
    let segments: Vec<String> = path.get(1..).unwrap_or(&[]).to_vec();
    Action::callback(move |c| {
        let value = &c.value;
        let is_shorthand_series =
            value.starts_with(prefix) && !value.starts_with(&double_prefix) && value.len() > 2;

        let carapace_styles = crate::config::styles().carapace;
        let mut values: RawValues = Vec::new();
        let include_hidden = crate::config::hidden() == crate::config::Hidden::IncludeCarapace;
        for flag in &cmd.flags {
            if flag.hidden && !include_hidden {
                continue;
            }
            // Go Flag.Style(): the flag kind decides the style
            let style = if !flag.takes_value {
                carapace_styles.flag_no_arg.clone()
            } else if flag.is_optarg() {
                carapace_styles.flag_opt_arg.clone()
            } else if matches!(flag.nargs, Nargs::AtLeastOne | Nargs::Any)
                || matches!(flag.nargs, Nargs::Exact(count) if count > 1)
            {
                carapace_styles.flag_multi_arg.clone()
            } else {
                carapace_styles.flag_arg.clone()
            };
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
                let mut value =
                    RawValue::styled_described(shorthand.to_string(), description, &style);
                value.tag = "shorthand flags".to_owned();
                value.uid = Some(crate::uid::flag(&root, &segments, &flag.name));
                if flag.is_optarg() {
                    value = value.nospace(true);
                }
                values.push(value);
            } else {
                let mut longhand = RawValue::styled_described(
                    format!("{double_prefix}{}", flag.name),
                    description.clone(),
                    &style,
                );
                longhand.tag = "longhand flags".to_owned();
                longhand.uid = Some(crate::uid::flag(&root, &segments, &flag.name));
                values.push(longhand);
                if let Some(shorthand) = flag.shorthand {
                    let mut short = RawValue::styled_described(
                        format!("{prefix}{shorthand}"),
                        description,
                        &style,
                    );
                    short.tag = "shorthand flags".to_owned();
                    short.uid = Some(crate::uid::flag(&root, &segments, &flag.name));
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
///
/// Commands carry their group's tag (`{id} commands`) and the Highlight
/// style of the group's position (Go `common.Group`); the group order is
/// the first-appearance order among the subcommands, matching how the
/// spec registers groups. Each candidate carries the implicit uid
/// `cmd://<root>/<path>/<name>` with aliases resolved to the actual
/// name (Go `ActionCommands`' `UidF`).
pub fn action_commands(cmd: &Command, path: &[String]) -> Action {
    let include_hidden = crate::config::hidden() == crate::config::Hidden::IncludeCarapace;

    let mut group_order: Vec<String> = Vec::new();
    let mut group_of: std::collections::HashMap<String, Option<String>> =
        std::collections::HashMap::new();
    for subcommand in &cmd.subcommands {
        if let Some(group) = &subcommand.group {
            if !group_order.contains(group) {
                group_order.push(group.clone());
            }
        }
        group_of.insert(subcommand.name.clone(), subcommand.group.clone());
        for alias in &subcommand.aliases {
            group_of.insert(alias.clone(), subcommand.group.clone());
        }
    }
    let has_groups = !group_order.is_empty();

    let values: Vec<(String, String)> = cmd
        .subcommands
        .iter()
        .filter(|subcommand| include_hidden || !subcommand.hidden)
        .flat_map(|subcommand| {
            std::iter::once((subcommand.name.clone(), subcommand.description.clone())).chain(
                subcommand
                    .aliases
                    .iter()
                    .map(|alias| (alias.clone(), subcommand.description.clone())),
            )
        })
        .collect();
    let action = Action::values_described(values).tag("commands");

    // aliases resolve to the actual command name in the uid path (Go's
    // `cmd.Find` in the UidF closure)
    let name_of: std::collections::HashMap<String, String> = cmd
        .subcommands
        .iter()
        .flat_map(|subcommand| {
            std::iter::once((subcommand.name.clone(), subcommand.name.clone())).chain(
                subcommand
                    .aliases
                    .iter()
                    .map(|alias| (alias.clone(), subcommand.name.clone())),
            )
        })
        .collect();
    let root = path.first().map(String::as_str).unwrap_or("").to_owned();
    let segments: Vec<String> = path.get(1..).unwrap_or(&[]).to_vec();

    let carapace_styles = crate::config::styles().carapace;
    let group_for_tag = group_of.clone();
    let group_for_style = group_of;
    action
        .uid_f(move |value| {
            let actual = name_of
                .get(value)
                .cloned()
                .unwrap_or_else(|| value.to_string());
            let segments: Vec<&str> = segments
                .iter()
                .map(String::as_str)
                .chain(std::iter::once(actual.as_str()))
                .collect();
            crate::uid::command(&root, &segments)
        })
        .tag_f(
            move |value| match group_for_tag.get(&value.value).cloned().flatten() {
                Some(group) if group.ends_with(" commands") => group,
                Some(group) => format!("{group} commands"),
                None if has_groups => "other commands".to_owned(),
                None => "commands".to_owned(),
            },
        )
        .style_f(
            move |value| match group_for_style.get(&value.value).cloned().flatten() {
                Some(group) => group_order
                    .iter()
                    .position(|id| *id == group)
                    .map(|level| carapace_styles.highlight(level))
                    .unwrap_or_default(),
                _ => String::new(),
            },
        )
}
