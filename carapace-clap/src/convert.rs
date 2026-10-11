//! Conversion of a `clap::Command` tree into the normalized carapace model.

use carapace_core::Action;
use carapace_core::model::{ArgumentStyle, Command, Flag, Nargs, Positional};

use crate::completer::{
    ArgCompleter, CommandCompleter, DashAnyCompleter, DashCompleter, FlagCompleter,
    PositionalCompleter,
};

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
    // the command-level positional list fills positionals without their own
    // completion slot (the Nth action completes the Nth positional)
    // the flag map fills flags without their own completion slot; entries
    // are applied in reverse so a repeated key keeps its last action
    if let Some(completer) = cmd.get::<FlagCompleter>() {
        for (name, action) in completer.0.iter().rev() {
            let matches = |flag: &Flag| {
                flag.name == *name
                    || flag.aliases.iter().any(|alias| alias == name)
                    || (name.chars().count() == 1 && flag.shorthand == name.chars().next())
            };
            if let Some(flag) = model.flags.iter_mut().find(|flag| matches(flag)) {
                if flag.completion.is_none() {
                    flag.completion = Some(action.clone());
                }
            }
        }
    }
    if let Some(completer) = cmd.get::<PositionalCompleter>() {
        for (index, action) in completer.0.iter().enumerate() {
            if let Some(positional) = model.positionals.get_mut(index) {
                if positional.completion.is_none() {
                    positional.completion = Some(action.clone());
                }
            }
        }
    }
    if let Some(completer) = cmd.get::<DashAnyCompleter>() {
        model.dash_any = Some(completer.0.clone());
    }
    if let Some(completer) = cmd.get::<DashCompleter>() {
        model.dash_positionals = completer
            .0
            .iter()
            .enumerate()
            .map(|(index, action)| Positional {
                name: format!("dash{}", index + 1),
                completion: Some(action.clone()),
                ..Default::default()
            })
            .collect();
    }
    model.hidden = cmd.is_hide_set();
    init_help_completion(&mut model);
    model
}

/// Complete the arguments of the auto-generated `help` subcommand with the
/// visible subcommands of the root (port of Go `initHelpCompletion`).
fn init_help_completion(model: &mut Command) {
    let has_help = model.subcommands.iter().any(|c| c.name == "help");
    if !has_help {
        return;
    }
    let mut root = model.clone();
    root.subcommands.retain(|c| c.name != "help");
    let commands = carapace_core::traverse::action_commands(&root);
    if let Some(help) = model.subcommands.iter_mut().find(|c| c.name == "help") {
        help.positional_any = Some(commands);
    }
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
                if min >= 1 {
                    Nargs::AtLeastOne
                } else {
                    Nargs::Any
                }
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
        shorthand_only: arg.get_long().is_none() && arg.get_short().is_some(),
        aliases,
        takes_value,
        nargs,
        optional,
        argument_style: argument_style(arg),
        repeatable,
        required: arg.is_required_set(),
        hidden: arg.is_hide_set(),
        allow_hyphen_values: arg.is_allow_hyphen_values_set(),
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
                value
                    .get_help()
                    .map(|help| help.to_string())
                    .unwrap_or_default(),
            )
        })
        .collect();
    (!values.is_empty()).then(|| Action::values_described(values))
}

fn value_hint_action(arg: &clap::Arg) -> Option<Action> {
    use clap::builder::ValueHint;
    match arg.get_value_hint() {
        ValueHint::DirPath => Some(Action::directories()),
        ValueHint::AnyPath | ValueHint::FilePath => Some(Action::files(Vec::<String>::new())),
        _ => None,
    }
}
