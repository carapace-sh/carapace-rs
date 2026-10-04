//! Conversion of a `clap::Command` tree into the normalized carapace model.

use carapace_core::model::{ArgumentStyle, Command, Flag, Nargs, Positional};
use carapace_core::Action;

use crate::completer::{ArgCompleter, CommandCompleter};

/// Convert a built clap command tree into the normalized model.
///
/// The command is built (`Command::build`) first so indices and implicit
/// args are resolved.
pub fn to_model(cmd: &mut clap::Command) -> Command {
    cmd.build();
    convert_command(cmd)
}

fn convert_command(cmd: &clap::Command) -> Command {
    let mut model = Command::new(cmd.get_name().to_owned());
    model.aliases = cmd.get_all_aliases().map(String::from).collect();
    model.description = cmd
        .get_about()
        .or_else(|| cmd.get_long_about())
        .map(|about| about.to_string())
        .unwrap_or_default();
    model.interspersed = !cmd
        .get_positionals()
        .any(|arg| arg.is_trailing_var_arg_set());
    model.flags = cmd
        .get_arguments()
        .filter(|arg| !arg.is_positional())
        .map(convert_flag)
        .collect();
    model.positionals = {
        let mut positionals: Vec<Positional> =
            cmd.get_positionals().map(convert_positional).collect();
        positionals.sort_by_key(|positional| {
            cmd.get_positionals()
                .find(|arg| arg.get_id().as_str() == positional.name)
                .and_then(|arg| arg.get_index())
                .unwrap_or(usize::MAX)
        });
        positionals
    };
    model.subcommands = cmd.get_subcommands().map(convert_command).collect();
    if let Some(completer) = cmd.get::<CommandCompleter>() {
        model.positional_any = Some(completer.0.clone());
    }
    model
}

fn convert_flag(arg: &clap::Arg) -> Flag {
    let num_args = arg.get_num_args();
    let takes_value = num_args.is_some_and(|range| range.takes_values());
    let repeatable = matches!(
        *arg.get_action(),
        clap::builder::ArgAction::Append | clap::builder::ArgAction::Count
    );
    let (nargs, optional) = match num_args {
        Some(range) => {
            let min = range.min_values();
            let max = range.max_values();
            let optional = takes_value && min == 0;
            let nargs = if max == usize::MAX {
                if min >= 1 { Nargs::AtLeastOne } else { Nargs::Any }
            } else {
                Nargs::Exact(max)
            };
            (nargs, optional)
        }
        None => (Nargs::Exact(1), false),
    };

    let mut aliases: Vec<String> = arg
        .get_aliases()
        .into_iter()
        .flatten()
        .map(String::from)
        .collect();
    aliases.retain(|alias| Some(alias.as_str()) != arg.get_long());
    aliases.dedup();

    Flag {
        id: arg.get_id().as_str().to_owned(),
        name: arg
            .get_long()
            .unwrap_or_else(|| arg.get_id().as_str())
            .to_owned(),
        shorthand: arg.get_short(),
        aliases,
        takes_value,
        nargs,
        optional,
        argument_style: argument_style(arg),
        optarg_delimiter: '=',
        repeatable,
        required: arg.is_required_set(),
        hidden: arg.is_hide_set(),
        description: arg
            .get_long_help()
            .or_else(|| arg.get_help())
            .map(|help| help.to_string())
            .unwrap_or_default(),
        style: None,
        tag: None,
        completion: arg
            .get::<ArgCompleter>()
            .map(|completer| completer.0.clone())
            .or_else(|| possible_values_action(arg))
            .or_else(|| value_hint_action(arg)),
    }
}

fn argument_style(arg: &clap::Arg) -> ArgumentStyle {
    if arg.is_require_equals_set() {
        ArgumentStyle {
            accepts_next: false,
            accepts_delimited: true,
            accepts_attached: false,
        }
    } else {
        ArgumentStyle::ALL
    }
}

fn convert_positional(arg: &clap::Arg) -> Positional {
    let num_args = arg.get_num_args();
    Positional {
        name: arg.get_id().as_str().to_owned(),
        optional: num_args.is_none_or(|range| range.min_values() == 0),
        repeatable: num_args.is_some_and(|range| range.max_values() > 1),
        description: arg
            .get_long_help()
            .or_else(|| arg.get_help())
            .map(|help| help.to_string())
            .unwrap_or_default(),
        completion: arg
            .get::<ArgCompleter>()
            .map(|completer| completer.0.clone())
            .or_else(|| possible_values_action(arg))
            .or_else(|| value_hint_action(arg)),
        tag: None,
    }
}

fn possible_values_action(arg: &clap::Arg) -> Option<Action> {
    let values: Vec<(String, String)> = arg
        .get_possible_values()
        .into_iter()
        .filter(|value| !value.is_hide_set())
        .map(|value| {
            (
                value.get_name().to_owned(),
                value.get_help().map(|help| help.to_string()).unwrap_or_default(),
            )
        })
        .collect();
    (!values.is_empty()).then(|| Action::values_described(values))
}

fn value_hint_action(arg: &clap::Arg) -> Option<Action> {
    use clap::builder::ValueHint;
    match arg.get_value_hint() {
        ValueHint::DirPath => Some(Action::directories()),
        ValueHint::AnyPath | ValueHint::FilePath => {
            Some(Action::files(Vec::<String>::new()))
        }
        _ => None,
    }
}
