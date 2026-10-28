//! The completion engine: argument traversal and completion resolution.
//!
//! Direct port of Go carapace's `traverse.go` state machine over the
//! normalized [`Command`] model.

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
        // the template's environment overrides the process environment
        // entry by entry (the sandbox analog is the mock's env)
        for (key, value) in &template.env {
            context.env.insert(key.clone(), value.clone());
        }
        // the template overrides the process state only when set: an
        // unseeded traverse keeps the process working directory
        if !template.dir.as_os_str().is_empty() {
            context.dir = template.dir.clone();
        }
        context.mock = template.mock.clone().or(context.mock);
        // the flags derive from the merged environment map (the env is
        // their single source of truth)
        context.config = crate::config::Flags::from_env_map(&context.env);
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
            // the current command's flags parse before descending (Go
            // `ParseFlags(inArgs)` in the subcommand case), so the flag
            // state accumulates down the tree (hooks read the values of
            // flags defined on ancestors)
            let mut sub_template = template;
            let mut parent_flags = parse::parse(cmd, &in_args).flags;
            sub_template
                .parsed_flags
                .retain(|(name, _)| !parent_flags.iter().any(|(flag, _)| flag == name));
            parent_flags.append(&mut sub_template.parsed_flags);
            sub_template.parsed_flags = parent_flags;
            // recurse with everything after the subcommand, including the
            // value being completed as the last word (Go passes args[i+1:]);
            // the uid path uses the subcommand's actual name (Go
            // `uid.Command` builds from `cmd.Name()`, not the typed alias)
            let mut sub_path = path.clone();
            sub_path.push(subcommand.name.clone());
            return traverse_command(
                subcommand,
                args[index + 1..].to_vec(),
                sub_template,
                sub_path,
            );
        }

        crate::log::log(&format!("arg {arg:?} is a positional"));
        in_args.push(arg.clone());
        in_positionals.push(arg);
    }

    if !cmd.disable_flag_parsing && !dash {
        if let Some(message) = unknown_flag {
            if !context.config.lenient {
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
        context.parsed_flags = template.parsed_flags.clone();
        for (name, values) in parsed.flags.iter() {
            // repeatable flags accumulate values across occurrences (pflag
            // slice semantics); others keep the last occurrence only
            let repeatable = cmd.lookup_id(name).is_some_and(|flag| flag.repeatable);
            if repeatable {
                if let Some(existing) = context
                    .parsed_flags
                    .iter_mut()
                    .find(|(flag, _)| flag == name)
                {
                    existing.1.extend(values.iter().cloned());
                    continue;
                }
            }
            context.parsed_flags.retain(|(flag, _)| flag != name);
            context.parsed_flags.push((name.clone(), values.clone()));
        }
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
            // a value delimiter completes the last segment only; the
            // earlier segments ride the prefix (clap `value_delimiter`)
            let action = match flag.flag.value_delimiter {
                Some(delimiter) => match context.value.rsplit_once(delimiter) {
                    Some((head, _)) => {
                        get_flag(cmd, &flag.flag.name).prefix(format!("{head}{delimiter}"))
                    }
                    None => get_flag(cmd, &flag.flag.name),
                },
                None => get_flag(cmd, &flag.flag.name),
            };
            return Completion { action, context };
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
                            .style_f(|_, value| crate::style::keyword_style(&value.value))
                            .usage(flag.flag.description.clone())
                            .prefix(&flag.arg_prefix),
                        context,
                    };
                }
                let action = match flag.flag.value_delimiter {
                    Some(delimiter) => {
                        let value_part = &context.value[flag.arg_prefix.len()..];
                        match value_part.rsplit_once(delimiter) {
                            Some((head, _)) => get_flag(cmd, &flag.flag.name)
                                .prefix(format!("{}{head}{delimiter}", flag.arg_prefix)),
                            None => get_flag(cmd, &flag.flag.name).prefix(&flag.arg_prefix),
                        }
                    }
                    None => get_flag(cmd, &flag.flag.name).prefix(&flag.arg_prefix),
                };
                return Completion { action, context };
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
    let positional = match cmd
        .positional_at(context.args.len())
        .and_then(|positional| positional.value_delimiter)
    {
        Some(delimiter) => match context.value.rsplit_once(delimiter) {
            Some((head, _)) => positional.prefix(format!("{head}{delimiter}")),
            None => positional,
        },
        None => positional,
    };
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
        let include_hidden = c.config.hidden != crate::config::Hidden::None;
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
                || matches!(flag.nargs, Nargs::Range(_, max) if max > 1)
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

        let hidden = c.config.hidden;
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

#[cfg(test)]
mod tests {
    use crate::{Action, Command, Flag, Positional, traverse};

    fn app() -> Command {
        Command::new("app")
            .flag(Flag {
                id: "verbose".into(),
                name: "verbose".into(),
                shorthand: Some('v'),
                ..Default::default()
            })
            .flag(Flag {
                id: "output".into(),
                name: "output".into(),
                shorthand: Some('o'),
                takes_value: true,
                completion: Some(Action::values(["file1", "file2"])),
                ..Default::default()
            })
            .flag(Flag {
                id: "include".into(),
                name: "include".into(),
                takes_value: true,
                nargs: crate::Nargs::AtLeastOne,
                completion: Some(Action::values(["inc1", "inc2"])),
                ..Default::default()
            })
            .flag(Flag {
                id: "define".into(),
                name: "define".into(),
                takes_value: true,
                optional: true,
                completion: Some(Action::values(["key1", "key2"])),
                ..Default::default()
            })
            .positional(Positional {
                name: "target".into(),
                completion: Some(Action::values(["t1", "t2"])),
                ..Default::default()
            })
            .positional(Positional {
                name: "rest".into(),
                repeatable: true,
                completion: Some(Action::values(["r1", "r2"])),
                ..Default::default()
            })
            .subcommand(
                Command::new("build")
                    .description("build things")
                    .flag(Flag {
                        id: "jobs".into(),
                        name: "jobs".into(),
                        takes_value: true,
                        completion: Some(Action::values(["1", "2"])),
                        ..Default::default()
                    })
                    .positional(Positional {
                        name: "artifact".into(),
                        completion: Some(Action::values(["bin", "lib"])),
                        ..Default::default()
                    }),
            )
            .subcommand(Command::new("test").description("run tests"))
    }

    fn complete(args: &[&str]) -> Vec<String> {
        let completion = traverse(&app(), args.iter().map(|s| s.to_string()).collect());
        let mut values: Vec<String> = completion
            .action
            .invoke(&completion.context)
            .values
            .into_iter()
            .map(|v| v.value)
            .collect();
        values.sort();
        values
    }

    #[test]
    fn completes_positionals() {
        assert_eq!(complete(&[]), vec!["build", "t1", "t2", "test"]);
        assert_eq!(complete(&["t1", ""]), vec!["r1", "r2"]);
        assert_eq!(complete(&["t1", "r1", ""]), vec!["r1", "r2"]);
    }

    #[test]
    fn completes_flag_names() {
        // `--` excludes shorthand candidates (they do not carry the prefix,
        // Go `actionFlags` wraps the batch in `MultiParts(".")`)
        assert_eq!(
            complete(&["--"]),
            vec!["--define", "--include", "--output", "--verbose"]
        );
        assert_eq!(
            complete(&["-"]),
            vec!["--define", "--include", "--output", "--verbose", "-o", "-v"]
        );
    }

    #[test]
    fn completes_flag_value_after_separate_arg() {
        // `--output ` completes the flag's value
        assert_eq!(complete(&["--output", ""]), vec!["file1", "file2"]);
        // shorthand
        assert_eq!(complete(&["-o", ""]), vec!["file1", "file2"]);
    }

    #[test]
    fn completes_attached_flag_value() {
        assert_eq!(
            complete(&["--output="]),
            vec!["--output=file1", "--output=file2"]
        );
        assert_eq!(complete(&["-o="]), vec!["-o=file1", "-o=file2"]);
        assert_eq!(complete(&["-ofile"]), vec!["-ofile1", "-ofile2"]);
    }

    #[test]
    fn completes_optional_flag_value() {
        // optarg with explicit delimiter
        assert_eq!(
            complete(&["--define="]),
            vec!["--define=key1", "--define=key2"]
        );
        // optarg never consumes the next positional
        assert_eq!(
            complete(&["--define", ""]),
            vec!["build", "t1", "t2", "test"]
        );
    }

    #[test]
    fn completes_bool_flag_value_with_delimiter() {
        assert_eq!(
            complete(&["--verbose="]),
            vec!["--verbose=false", "--verbose=true"]
        );
    }

    #[test]
    fn flag_consumes_next_arg() {
        // after --output file1, no positionals were consumed yet: positional 0 + commands
        assert_eq!(
            complete(&["--output", "file1", ""]),
            vec!["build", "t1", "t2", "test"]
        );
        // variadic flag keeps consuming
        assert_eq!(complete(&["--include", "inc1", ""]), vec!["inc1", "inc2"]);
    }

    #[test]
    fn completes_after_subcommand() {
        assert_eq!(complete(&["build", ""]), vec!["bin", "lib"]);
        assert_eq!(complete(&["build", "--jobs", ""]), vec!["1", "2"]);
        // `-j` is a shorthand series whose member takes a required value: the
        // series aborts (Go `actionFlags` shorthand handling)
        assert_eq!(complete(&["build", "-j"]), Vec::<String>::new());
    }

    #[test]
    fn dash_disables_flag_completion() {
        assert_eq!(complete(&["--", ""]), vec!["t1", "t2"]);
        assert_eq!(complete(&["t1", "--", ""]), vec!["t1", "t2"]);
        assert_eq!(complete(&["t1", "--", "x", ""]), vec!["r1", "r2"]);
        assert_eq!(complete(&["--", "t1", ""]), vec!["r1", "r2"]);
    }

    #[test]
    fn shorthand_series_prefix() {
        // series of bool flags with partial shorthand: flag-name completion prefixed
        let completion = traverse(&app(), vec!["-v".to_string(), "-".to_string()]);
        let values = completion
            .action
            .invoke(&completion.context)
            .values
            .into_iter()
            .map(|v| v.value)
            .collect::<Vec<_>>();
        assert!(values.contains(&"-o".to_string()));
        assert!(values.contains(&"--output".to_string()));
    }

    #[test]
    fn unknown_flags_are_reported_unless_lenient() {
        // pflag errors on unknown flags; `CARAPACE_LENIENT` falls back to
        // ignoring them (Go `env.Lenient`)
        let completion = traverse(&app(), vec!["--unknown".to_string(), "".to_string()]);
        let invoked = completion.action.invoke(&completion.context);
        assert_eq!(invoked.values, Vec::new());
        assert_eq!(
            invoked.meta.messages,
            vec!["unknown flag: --unknown".to_owned()]
        );

        // the flags derive from the context environment map (Go's sandbox
        // analog seeds the child environment)
        let lenient = crate::Context::default().env("CARAPACE_LENIENT", "1");
        let completion = crate::traverse_seeded(
            &app(),
            vec!["--unknown".to_string(), "".to_string()],
            lenient,
        );
        let mut values: Vec<String> = completion
            .action
            .invoke(&completion.context)
            .values
            .into_iter()
            .map(|v| v.value)
            .collect();
        values.sort();
        assert_eq!(values, vec!["build", "t1", "t2", "test"]);
    }

    #[test]
    fn positional_any_fallback() {
        let cmd = Command::new("any")
            .positional(Positional {
                name: "only".into(),
                repeatable: true,
                ..Default::default()
            })
            .positional_any(Action::values(["any1"]));
        let completion = traverse(
            &cmd,
            vec!["a".to_string(), "b".into(), "c".into(), "".into()],
        );
        let values: Vec<String> = completion
            .action
            .invoke(&completion.context)
            .values
            .into_iter()
            .map(|v| v.value)
            .collect();
        assert_eq!(values, vec!["any1"]);
    }

    #[test]
    fn context_args_track_positionals() {
        let completion = traverse(&app(), vec!["t1".to_string(), "r1".into(), "".into()]);
        assert_eq!(completion.context.args, vec!["t1", "r1"]);
    }

    #[test]
    fn subcommand_recursion_ignores_completed_value() {
        // "app build xyz<TAB>" completes the empty word inside `build`,
        // not the value "xyz" itself
        assert_eq!(complete(&["build", "xyz"]), vec!["bin", "lib"]);
        // partial word equal to the subcommand name completes commands/positionals
        assert_eq!(complete(&["build"]), vec!["build", "t1", "t2", "test"]);
    }

    #[test]
    fn fixed_nargs_consume_flag_like_values() {
        // pflag consumes the next argument unconditionally for fixed nargs
        let app = app();
        let parsed = crate::traverse::parse::parse(
            &app,
            &["--output".to_string(), "--flaglike".to_string()],
        );
        assert_eq!(
            parsed.flags,
            vec![("output".to_string(), vec!["--flaglike".to_string()])]
        );
        assert!(parsed.positionals.is_empty());
    }

    #[test]
    fn hyphen_values_are_consumed_as_flag_arguments() {
        let cmd = Command::new("app").flag(Flag {
            id: "grep".into(),
            name: "grep".into(),
            takes_value: true,
            nargs: crate::Nargs::Any,
            allow_hyphen_values: true,
            completion: Some(Action::values(["pattern1", "pattern2"])),
            ..Default::default()
        });

        // "-not-a-flag" is consumed as a value of --grep, completing its value
        let completion = traverse(
            &cmd,
            vec![
                "--grep".into(),
                "first".into(),
                "-not-a-flag".into(),
                "".into(),
            ],
        );
        let values: Vec<String> = completion
            .action
            .invoke(&completion.context)
            .values
            .into_iter()
            .map(|value| value.value)
            .collect();
        assert_eq!(values, vec!["pattern1", "pattern2"]);

        // without allow_hyphen_values, "-not-a-flag" ends variadic consumption
        // once a value was consumed (the first value is always taken)
        let cmd = Command::new("app").flag(Flag {
            id: "grep".into(),
            name: "grep".into(),
            takes_value: true,
            nargs: crate::Nargs::Any,
            completion: Some(Action::values(["pattern1", "pattern2"])),
            ..Default::default()
        });
        let completion = traverse(
            &cmd,
            vec![
                "--grep".into(),
                "first".into(),
                "-not-a-flag".into(),
                "".into(),
            ],
        );
        let values: Vec<String> = completion
            .action
            .invoke(&completion.context)
            .values
            .into_iter()
            .map(|value| value.value)
            .collect();
        assert!(values.is_empty(), "hyphen arg ends variadic consumption");
    }

    #[test]
    fn completes_attached_flag_value_with_partial_value() {
        // the arg prefix is the flag name including the delimiter, so the
        // partially typed value is replaced by the completed values
        assert_eq!(
            complete(&["--output=fi"]),
            vec!["--output=file1", "--output=file2"]
        );
        assert_eq!(complete(&["-o=fi"]), vec!["-o=file1", "-o=file2"]);
    }

    #[test]
    fn flag_names_carry_kind_styles() {
        // Go Flag.Style(): FlagNoArg, FlagArg, FlagMultiArg, FlagOptArg
        let completion = traverse(&app(), vec!["--".to_string()]);
        let mut values: Vec<(String, String)> = completion
            .action
            .invoke(&completion.context)
            .values
            .into_iter()
            .map(|v| (v.value, v.style))
            .collect();
        values.sort();

        let styles: std::collections::HashMap<String, String> = values.into_iter().collect();
        assert_eq!(styles["--verbose"], ""); // no argument (style.Default)
        assert_eq!(styles["--output"], "blue"); // single argument
        assert_eq!(styles["--include"], "magenta"); // multiple arguments
        assert_eq!(styles["--define"], "yellow"); // optional argument
    }

    #[test]
    fn bool_flag_values_carry_keyword_styles() {
        // Go StyleF(style.ForKeyword): true is positive, false negative
        let completion = traverse(&app(), vec!["--verbose=".to_string()]);
        let mut values: Vec<(String, String)> = completion
            .action
            .invoke(&completion.context)
            .values
            .into_iter()
            .map(|v| (v.value, v.style))
            .collect();
        values.sort();

        let styles: std::collections::HashMap<String, String> = values.into_iter().collect();
        assert_eq!(styles["--verbose=false"], "red");
        assert_eq!(styles["--verbose=true"], "green");
    }

    #[test]
    fn command_groups_style_and_tag() {
        use crate::model::Command as ModelCommand;
        let cmd = ModelCommand::new("app")
            .subcommand(
                ModelCommand::new("alpha")
                    .description("first group")
                    .alias("al"),
            )
            .subcommand(ModelCommand::new("beta").group("admin"))
            .subcommand(ModelCommand::new("gamma").group("admin"))
            .subcommand(ModelCommand::new("delta").group("ops"))
            .subcommand(ModelCommand::new("plain"));

        let completion = traverse(&cmd, vec!["".to_string()]);
        let invoked = completion.action.invoke(&completion.context);

        let mut entries: Vec<(String, String, String)> = invoked
            .values
            .into_iter()
            .map(|v| (v.value, v.tag, v.style))
            .collect();
        entries.sort();

        let by_value: std::collections::HashMap<String, (String, String)> =
            entries.into_iter().map(|(v, t, s)| (v, (t, s))).collect();

        // ungrouped commands in a grouped tree are "other commands", unstyled
        assert_eq!(
            by_value["plain"],
            ("other commands".to_owned(), String::new())
        );
        // the first group's position maps to Highlight1
        assert_eq!(
            by_value["beta"],
            ("admin commands".to_owned(), "blue".to_owned())
        );
        assert_eq!(
            by_value["gamma"],
            ("admin commands".to_owned(), "blue".to_owned())
        );
        // the second group maps to Highlight2
        assert_eq!(
            by_value["delta"],
            ("ops commands".to_owned(), "yellow".to_owned())
        );
        // aliases carry the same group
        assert_eq!(by_value["al"], ("other commands".to_owned(), String::new()));
    }

    #[test]
    fn ungrouped_tree_tags_plain_commands() {
        let completion = traverse(&app(), vec!["".to_string()]);
        let invoked = completion.action.invoke(&completion.context);
        // command candidates use the plain tag; positional candidates keep theirs
        let commands: Vec<_> = invoked
            .values
            .iter()
            .filter(|v| v.value == "build" || v.value == "test")
            .collect();
        assert!(
            commands
                .iter()
                .all(|v| v.tag == "commands" && v.style.is_empty()),
            "ungrouped tree uses the plain commands tag"
        );
    }

    #[test]
    fn implicit_uids_on_flags_and_subcommands() {
        use crate::model::Command as ModelCommand;
        let cmd = ModelCommand::new("app")
            .flag(Flag {
                id: "verbose".into(),
                name: "verbose".into(),
                shorthand: Some('v'),
                ..Default::default()
            })
            .subcommand(ModelCommand::new("sub").alias("s").subcommand(
                ModelCommand::new("deep").flag(Flag {
                    id: "force".into(),
                    name: "force".into(),
                    ..Default::default()
                }),
            ));

        // flag candidates carry cmd://app?flag=<name>
        let completion = traverse(&cmd, vec!["-".to_string()]);
        let invoked = completion.action.invoke(&completion.context);
        let uid_of = |value: &str| {
            invoked
                .values
                .iter()
                .find(|v| v.value == value)
                .unwrap()
                .uid
                .clone()
                .unwrap()
        };
        assert_eq!(uid_of("--verbose"), "cmd://app?flag=verbose");
        assert_eq!(uid_of("-v"), "cmd://app?flag=verbose");

        // subcommand candidates carry cmd://app/<name>; aliases resolve to the
        // actual name (Go's cmd.Find in the UidF closure)
        let completion = traverse(&cmd, vec!["".to_string()]);
        let invoked = completion.action.invoke(&completion.context);
        let uid_of = |value: &str| {
            invoked
                .values
                .iter()
                .find(|v| v.value == value)
                .unwrap()
                .uid
                .clone()
                .unwrap()
        };
        assert_eq!(uid_of("sub"), "cmd://app/sub");
        assert_eq!(uid_of("s"), "cmd://app/sub");

        // deeper nesting accumulates the path
        let completion = traverse(&cmd, vec!["sub".to_string(), "".to_string()]);
        let invoked = completion.action.invoke(&completion.context);
        let deep = invoked
            .values
            .iter()
            .find(|v| v.value == "deep")
            .unwrap()
            .uid
            .clone()
            .unwrap();
        assert_eq!(deep, "cmd://app/sub/deep");

        // flag uids inside a subcommand use the nested path
        let completion = traverse(
            &cmd,
            vec!["sub".to_string(), "deep".to_string(), "-".to_string()],
        );
        let invoked = completion.action.invoke(&completion.context);
        let force = invoked
            .values
            .iter()
            .find(|v| v.value == "--force")
            .unwrap()
            .uid
            .clone()
            .unwrap();
        assert_eq!(force, "cmd://app/sub/deep?flag=force");
    }

    #[test]
    fn traverse_keeps_the_process_working_directory() {
        // an unseeded traverse completes relative to the process cwd (the
        // template's empty dir must not blank it)
        let completion = traverse(&app(), vec!["".to_string()]);
        assert_eq!(completion.context.dir, std::env::current_dir().unwrap());
    }

    #[test]
    fn longhand_value_does_not_register_shorthands() {
        // `--output=dev` is a longhand with a value, not a shorthand series:
        // its trailing `v` must not register the `v` shorthand as given
        let completion = traverse(
            &app(),
            vec!["--output".to_string(), "dev".to_string(), "-".to_string()],
        );
        let given: Vec<&str> = completion
            .context
            .parsed_flags
            .iter()
            .map(|(name, _)| name.as_str())
            .collect();
        assert_eq!(given, vec!["output"]);
    }
}
