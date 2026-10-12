//! The completion engine: argument traversal and completion resolution.
//!
//! Direct port of Go carapace's `traverse.go` state machine over the
//! normalized [`Command`](crate::model::Command) model.

mod lookup;
pub mod parse;

pub mod git;

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
    traverse_command(root, args, Context::default(), vec![root.name.clone()])
}

/// Like [`traverse`] but seeds every level's context with the template's
/// directory, environment and mock (the in-process analog of Go's sandbox
/// `NewContext` overrides). Actions invoked during traversal (e.g. the
/// positional/command merge) see the seeded state.
pub fn traverse_seeded(root: &Command, args: Vec<String>, template: Context) -> Completion {
    traverse_command(root, args, template, vec![root.name.clone()])
}

fn traverse_command(
    cmd: &Command,
    args: Vec<String>,
    template: Context,
    path: Vec<String>,
) -> Completion {
    crate::log::log(&format!(
        "traverse called for {:?} with args {args:?}",
        cmd.name
    ));
    let mut context = {
        let mut context = Context::new(args.clone());
        context.env = template.env.clone();
        context.dir = template.dir.clone();
        context.mock = template.mock.clone();
        context
    };

    let mut in_args: Vec<String> = Vec::new();
    let mut in_positionals: Vec<String> = Vec::new();
    let mut in_flag: Option<MatchedFlag> = None;
    let mut unknown_flag: Option<String> = None;
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
                // pflag errors on unknown flags (Go's ParseFlags error);
                // the message surfaces unless completion is lenient
                let double_prefix: String = [cmd.flag_prefix, cmd.flag_prefix].iter().collect();
                unknown_flag = unknown_flag.or_else(|| {
                    Some(if arg.starts_with(&double_prefix) {
                        format!("unknown flag: {arg}")
                    } else {
                        arg.chars()
                            .skip(cmd.flag_prefix.len_utf8())
                            .find(|shorthand| cmd.lookup_short(*shorthand).is_none())
                            .map(|shorthand| {
                                format!("unknown shorthand flag: '{shorthand}' in {arg}")
                            })
                            .unwrap_or_else(|| format!("unknown flag: {arg}"))
                    })
                });
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
            return traverse_command(subcommand, args[index + 1..].to_vec(), template, sub_path);
        }

        crate::log::log(&format!("arg {arg:?} is a positional"));
        in_args.push(arg.clone());
        in_positionals.push(arg);
    }

    if !cmd.disable_flag_parsing && !dash {
        if let Some(message) = unknown_flag {
            if !crate::config::lenient() {
                return Completion {
                    action: Action::message(message),
                    context,
                };
            }
        }
    }

    let parsed = if cmd.disable_flag_parsing {
        crate::log::log(&format!("flag parsing disabled for {:?}", cmd.name));
        // cobra skips ParseFlags entirely: the classified args are the
        // positional args (everything, including `--` and flag-lookalikes)
        context.args = in_args.clone();
        parse::Parsed::default()
    } else {
        let mut to_parse = in_args.clone();
        if !dash {
            if let Some(flag) = in_flag.as_ref() {
                if flag.args.is_empty() && flag.consumes("") {
                    to_parse.pop();
                }
            }
            // shorthand series (Go: `IsShorthandSeries(context.Value)`):
            // previous shorthands register as changed so they are not
            // offered again; when the series ends with a flag that still
            // expects a value, that shorthand is stripped so it does not
            // consume anything yet
            if (cmd.interspersed || in_positionals.is_empty())
                && is_shorthand_series(cmd, &context.value)
            {
                match lookup::lookup_arg(cmd, &context.value) {
                    None => {}
                    Some(local)
                        if (local.args.is_empty()
                            || local.args.first().is_some_and(String::is_empty))
                            && (!local.flag.is_optarg() || local.arg_prefix.ends_with('=')) =>
                    {
                        let shorthand = local
                            .flag
                            .shorthand
                            .map(|shorthand| shorthand.to_string())
                            .unwrap_or_default();
                        if let Some(index) = local.arg_prefix.rfind(&shorthand) {
                            to_parse.push(local.arg_prefix[..index].to_owned());
                        }
                    }
                    Some(local) => to_parse.push(local.arg_prefix),
                }
            }
        }
        let parsed = parse::parse(cmd, &to_parse);
        context.args = parsed.positionals.clone();
        context.parsed_flags = parsed.flags.clone();
        parsed
    };

    // dash argument (only when parsing recorded the dash: pflag's
    // non-interspersed mode never reaches `--` after a positional, Go
    // `common.IsDash`)
    if parsed.dash {
        context.args = parsed.dash_positionals().to_vec();
        // the full positional list (pflag `Args()`) for actions that
        // complete the pre-dash position (Go `ActionPositional`)
        context.all_positionals = Some(parsed.positionals.clone());
        let action = cmd
            .dash_positionals
            .get(context.args.len())
            .and_then(|positional| positional.completion.clone())
            .or_else(|| cmd.dash_any.clone())
            .unwrap_or_else(|| get_positional(cmd, context.args.len()));
        // the use line is the dash completion's fallback usage as well
        // (Go `storage.getPositional`)
        let action = with_usage_fallback(action, use_line_fallback(cmd));
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
                        // Go styles bool values as keywords and surfaces
                        // the flag usage
                        action: Action::values(["true", "false"])
                            .style_f(|_, value| crate::config::keyword_style(&value.value))
                            .usage(flag.flag.description.clone())
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

/// Whether the value is a shorthand flag series (Go `FlagSet.IsShorthandSeries`):
/// single prefix, at least one non-prefix character, not the double prefix.
fn is_shorthand_series(cmd: &Command, value: &str) -> bool {
    let double_prefix: String = [cmd.flag_prefix, cmd.flag_prefix].iter().collect();
    value.starts_with(cmd.flag_prefix)
        && !value.starts_with(&double_prefix)
        && value
            .chars()
            .nth(1)
            .is_some_and(|second| second != cmd.flag_prefix)
}

/// Completion for the flag value registered on the command (empty when
/// unset). The flag's description is the fallback usage (Go
/// `storage.getFlag`).
pub fn get_flag(cmd: &Command, name: &str) -> Action {
    let (action, usage) = cmd
        .lookup_long(name)
        .map(|flag| {
            (
                flag.completion.clone().unwrap_or_default(),
                flag.description.clone(),
            )
        })
        .unwrap_or_default();
    with_usage_fallback(action, usage)
}

/// Completion for the positional at `index` (falling back to any-positional).
/// The command's use line is the fallback usage when it has more than one
/// field (Go `storage.getPositional`).
pub fn get_positional(cmd: &Command, index: usize) -> Action {
    let action = cmd
        .positional_at(index)
        .and_then(|positional| positional.completion.clone())
        .or_else(|| cmd.positional_any.clone())
        .unwrap_or_default();
    with_usage_fallback(action, use_line_fallback(cmd))
}

/// The command's use line when it specifies arguments (Go: `cmd.Use` with
/// more than one field).
fn use_line_fallback(cmd: &Command) -> String {
    cmd.use_line
        .as_ref()
        .filter(|use_line| use_line.split_whitespace().count() > 1)
        .cloned()
        .unwrap_or_default()
}

/// Wrap an action so its invocation receives `usage` as fallback usage when
/// the action itself does not define one.
fn with_usage_fallback(action: Action, usage: String) -> Action {
    if usage.is_empty() {
        return action;
    }
    Action::callback(move |c| {
        let invoked = action.invoke(c);
        let mut action = invoked.to_action();
        if action.meta.usage.as_deref().is_none_or(str::is_empty) {
            action.meta.usage = Some(usage.clone());
        }
        action
    })
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
        let is_shorthand_series = is_shorthand_series(&cmd, value);

        let carapace_styles = crate::config::styles().carapace;
        let mut values: RawValues = Vec::new();
        let mut series_nospace: Vec<char> = Vec::new();
        let include_hidden = crate::config::hidden() != crate::config::Hidden::None;
        for flag in &cmd.flags {
            if flag.hidden && !include_hidden {
                continue;
            }
            // don't repeat a flag that was already given (Go `f.Changed &&
            // !f.IsRepeatable()`) and skip flags of a mutually-exclusive
            // group that already has a member set
            let changed = |name: &str| c.parsed_flags.iter().any(|(flag, _)| flag == name);
            if changed(&flag.name) && !flag.repeatable {
                continue;
            }
            if flag.aliases.iter().any(|alias| changed(alias)) && !flag.repeatable {
                continue;
            }
            if flag.conflicts.iter().any(|other| changed(other)) {
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
                    series_nospace.push(shorthand);
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
            if series_nospace.is_empty() {
                action.prefix(value.clone())
            } else {
                action.prefix(value.clone()).nospace(series_nospace)
            }
        } else {
            // flags group with `.` (Go `batch.ToA().MultiParts(".")`)
            let action = action;
            Action::callback(move |c| action.invoke(c).to_multiparts_a(&["."]))
        }
    })
}

pub fn action_commands(cmd: &Command, path: &[String]) -> Action {
    let cmd = cmd.clone();
    let path = path.to_vec();
    Action::callback(move |c| {
        // `Context.Args` traverses the command tree further down (Go
        // `ActionCommands`): the first arg selects the subcommand, the
        // remainder shift into the nested completion
        if let Some(first) = c.args.first() {
            if let Some(subcommand) = cmd.lookup_subcommand(first) {
                let mut sub_path = path.clone();
                sub_path.push(subcommand.name.clone());
                return action_commands(subcommand, &sub_path).shift(1);
            }
            return Action::message(format!("unknown subcommand {first:?} for {:?}", cmd.name));
        }

        let hidden = crate::config::hidden();
        let include_hidden = hidden != crate::config::Hidden::None;

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
            .filter(|subcommand| {
                // the carapace-internal `_carapace` subcommand only shows
                // with `CARAPACE_HIDDEN=2` (Go `HIDDEN_INCLUDE_CARAPACE`)
                !(subcommand.hidden
                    && subcommand.name == "_carapace"
                    && hidden != crate::config::Hidden::IncludeCarapace)
            })
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
                move |_, value| match group_for_style.get(&value.value).cloned().flatten() {
                    Some(group) => group_order
                        .iter()
                        .position(|id| *id == group)
                        .map(|level| carapace_styles.highlight(level))
                        .unwrap_or_default(),
                    _ => String::new(),
                },
            )
    })
}

/// Completion of the positional arguments of the command, ignoring the
/// dash (port of Go `ActionPositional`): the pre-dash position selects
/// the explicit positional or falls back to the any-positional action.
///
/// Only valid while completing dash arguments; any other context yields
/// a message (Go checks `ArgsLenAtDash`).
pub fn action_positional(cmd: &Command) -> Action {
    let cmd = cmd.clone();
    Action::callback(move |c| {
        let Some(args) = c.all_positionals.clone() else {
            return Action::message("only allowed for dash arguments [ActionPositional]");
        };
        let mut context = c.clone();
        context.args = args;
        let action = cmd
            .positional_at(context.args.len())
            .and_then(|positional| positional.completion.clone())
            .or_else(|| cmd.positional_any.clone())
            .unwrap_or_default();
        action.invoke(&context).to_action()
    })
}
