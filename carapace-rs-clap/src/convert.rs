//! Conversion of a `clap::Command` tree into the normalized carapace model.

use carapace_rs::Action;
use carapace_rs::model::{ArgumentStyle, Command, Flag, Nargs, Positional};

use crate::completer::{
    ArgCompleter, ArgConflicts, CommandCompleter, DashAnyCompleter, DashCompleter, FlagCompleter,
    PositionalCompleter, PreInvoke, PreInvokeHook,
};

/// Convert a built clap command tree into the normalized model.
///
/// The command is built (`Command::build`) first so indices and implicit
/// args are resolved.
pub fn to_model(cmd: &mut clap::Command) -> Command {
    cmd.build();
    convert_command(cmd, &[], &[])
}

fn convert_command(
    cmd: &clap::Command,
    inherited: &[(String, Flag)],
    hooks: &[PreInvokeHook],
) -> Command {
    let mut model = Command::new(cmd.get_name().to_owned());
    model.aliases = cmd.get_all_aliases().map(String::from).collect();
    model.short_flag = cmd.get_short_flag();
    model.description = cmd
        .get_about()
        .or_else(|| cmd.get_long_about())
        .map(|about| about.to_string())
        .unwrap_or_default();
    model.interspersed = !cmd
        .get_positionals()
        .any(|arg| arg.is_trailing_var_arg_set());
    // the flag map is consulted per flag during conversion (explicit
    // registration outranks the clap-annotation fallbacks below), applied in
    // reverse so a repeated key keeps its last action
    let flag_completer = cmd
        .get::<FlagCompleter>()
        .map(|completer| completer.0.clone())
        .unwrap_or_default();
    let mut flags: Vec<Flag> = cmd
        .get_arguments()
        .filter(|arg| !arg.is_positional())
        .map(|arg| convert_flag(arg, &flag_completer))
        .collect();
    // persistent flags are inherited by every subcommand, fully converted
    // from their defining command (cobra's shared `*pflag.Flag`)
    for (name, flag) in inherited {
        let present = cmd.get_arguments().any(|arg| {
            !arg.is_positional()
                && (arg.get_id().as_str() == name || arg.get_long() == Some(name.as_str()))
        });
        if !present {
            flags.push(flag.clone());
        }
    }
    model.flags = flags;
    model.positionals = {
        let mut positionals: Vec<Positional> =
            cmd.get_positionals().map(convert_positional).collect();
        let index_map: std::collections::HashMap<String, usize> = cmd
            .get_positionals()
            .filter_map(|arg| {
                arg.get_index()
                    .map(|idx| (arg.get_id().as_str().to_owned(), idx))
            })
            .collect();
        positionals
            .sort_by_key(|positional| *index_map.get(&positional.name).unwrap_or(&usize::MAX));
        positionals
    };
    // inherited persistent flags skip the per-arg conversion above, so the
    // child's flag map can still fill their (empty) completion; entries are
    // applied in reverse so a repeated key keeps its last action
    if !flag_completer.is_empty() {
        for (name, action) in flag_completer.iter().rev() {
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
    // persistent flags flow down to subcommands with their converted
    // completion (Go's shared flag object lookup); clap's native
    // `global(true)` marks persistence just like the adapter's own
    // `ArgPersistent` extension (clap propagates globals at parse time,
    // not in the built tree, so the model must copy them)
    let mut persistent: Vec<(String, Flag)> = inherited.to_vec();
    for flag in &model.flags {
        let marked = cmd.get_arguments().any(|arg| {
            !arg.is_positional()
                && (arg.get::<crate::completer::ArgPersistent>().is_some() || arg.is_global_set())
                && (arg.get_id().as_str() == flag.name
                    || arg.get_long() == Some(flag.name.as_str()))
        });
        if marked && !persistent.iter().any(|(key, _)| key == &flag.name) {
            persistent.push((flag.name.clone(), flag.clone()));
        }
    }
    // ancestors' PreInvoke hooks run first (outermost), the command's own
    // hook last (Go's `storage.preinvoke` parent walk)
    let mut hooks: Vec<PreInvokeHook> = hooks.to_vec();
    if let Some(hook) = cmd.get::<PreInvoke>() {
        hooks.push(hook.0.clone());
    }
    model.subcommands = cmd
        .get_subcommands()
        .map(|subcommand| convert_command(subcommand, &persistent, &hooks))
        .collect();
    if let Some(completer) = cmd.get::<CommandCompleter>() {
        model.positional_any = Some(completer.0.clone());
    }
    // the command-level positional list fills positionals without their own
    // completion slot (the Nth action completes the Nth positional);
    // explicit registration outranks the clap-annotation fallbacks applied
    // afterwards
    if let Some(completer) = cmd.get::<PositionalCompleter>() {
        let actions: Vec<Action> = completer.0.clone();
        // Completions map by argument index (Go completes the Nth argument,
        // not the Nth positional definition): a variadic positional spans
        // all remaining indices, so it expands into one slot per provided
        // action plus a repeatable tail for the indices beyond.
        let max_values_of = |positional: &Positional| -> usize {
            cmd.get_positionals()
                .find(|arg| arg.get_id().as_str() == positional.name)
                .and_then(|arg| arg.get_num_args())
                .map(|range| range.max_values())
                .unwrap_or(1)
        };
        let mut expanded: Vec<Positional> = Vec::new();
        for positional in std::mem::take(&mut model.positionals) {
            if positional.repeatable {
                let max = max_values_of(&positional);
                let remaining = actions.len().saturating_sub(expanded.len()).max(1);
                let slots = if max == usize::MAX { remaining } else { max };
                for _ in 0..slots {
                    let mut slot = positional.clone();
                    slot.repeatable = false;
                    expanded.push(slot);
                }
                if max == usize::MAX {
                    expanded.push(positional);
                }
            } else {
                expanded.push(positional);
            }
        }
        model.positionals = expanded;
        // the Nth action completes the Nth argument even when clap defines
        // fewer positionals (Go's storage is independent of the command's
        // arg definition): pad with virtual positionals
        while model.positionals.len() < actions.len() {
            let index = model.positionals.len();
            model.positionals.push(carapace_rs::model::Positional {
                name: format!("positional{}", index + 1),
                optional: true,
                ..Default::default()
            });
        }
        for (index, action) in actions.iter().enumerate() {
            if let Some(positional) = model.positionals.get_mut(index) {
                if positional.completion.is_none() {
                    positional.completion = Some(action.clone());
                }
            }
        }
    }
    // clap annotations are the last completion fallback, applied after the
    // explicit registrations above: possible values, then value hints
    for positional in &mut model.positionals {
        if positional.completion.is_some() {
            continue;
        }
        positional.completion = cmd
            .get_positionals()
            .find(|arg| arg.get_id().as_str() == positional.name)
            .and_then(|arg| possible_values_action(arg).or_else(|| value_hint_action(arg)));
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
    crate::integration::apply_meta(cmd, &mut model);
    init_help_completion(&mut model);
    // after init_help_completion so the auto-generated help completion is
    // wrapped as well (Go's preinvoke applies to the help command too)
    wrap_pre_invoke(&mut model, &hooks);
    model
}

/// Wrap every completion slot with the hooks (Go `storage.preinvoke`):
/// the first hook is the outermost, so ancestors' hooks run first.
fn wrap_pre_invoke(model: &mut Command, hooks: &[PreInvokeHook]) {
    if hooks.is_empty() {
        return;
    }
    let wrap = |action: Action, hooks: &[PreInvokeHook]| {
        let mut action = action;
        // wrap from the innermost hook out, so the first hook (the
        // ancestor) is the outermost and runs first (Go `preinvoke`:
        // parent's hook receives the child-wrapped action)
        for hook in hooks.iter().rev() {
            let hook = hook.clone();
            action = Action::callback(move |c| hook(c, action.clone()));
        }
        action
    };
    for flag in &mut model.flags {
        if let Some(completion) = flag.completion.take() {
            flag.completion = Some(wrap(completion, hooks));
        }
    }
    for positional in model
        .positionals
        .iter_mut()
        .chain(model.dash_positionals.iter_mut())
    {
        if let Some(completion) = positional.completion.take() {
            positional.completion = Some(wrap(completion, hooks));
        }
    }
    for slot in [model.positional_any.as_mut(), model.dash_any.as_mut()]
        .into_iter()
        .flatten()
    {
        let action = slot.clone();
        *slot = wrap(action, hooks);
    }
}

/// Complete the arguments of the auto-generated `help` subcommand with the
/// visible subcommands of the root (port of Go `initHelpCompletion`).
fn init_help_completion(model: &mut Command) {
    let has_help = model.subcommands.iter().any(|c| c.name == "help");
    if !has_help {
        return;
    }
    // temporarily remove the help subcommand so it does not appear as a
    // completion candidate of itself (Go's `initHelpCompletion` rebuilds
    // from a clone; we swap to avoid the full model clone)
    let mut subcommands = std::mem::take(&mut model.subcommands);
    let mut help = subcommands
        .iter()
        .position(|c| c.name == "help")
        .map(|i| subcommands.remove(i))
        .unwrap();
    model.subcommands = subcommands;
    let commands = carapace_rs::traverse::action_commands(model, std::slice::from_ref(&model.name));
    {
        // cobra's help command declares `Use: "help [command]"`; the
        // use line doubles as the completion's fallback usage
        help.use_line = Some("help [command]".to_owned());
        help.positional_any = Some(commands);
        // clap's built help subcommand carries a copy of the parent's
        // subcommand tree (`_check_help_and_version`'s expand_help_tree)
        // stripped of extensions; cobra's help command has no
        // subcommands and the groupless copies would pollute the
        // traverse's positional/commands merge
        help.subcommands.clear();
    }
    model.subcommands.push(help);
}

fn convert_flag(arg: &clap::Arg, flag_completer: &[(String, Action)]) -> Flag {
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
            // the optional (optarg) shape is exactly one optional value
            // (`0..=1`), pflag's `NoOptDefVal` analog: it never consumes
            // the next argument, values bind only through `=`. Wider
            // `0..N` ranges are variadic: the engine's lookahead consumes
            // values until the next flag-lookalike.
            let optional = takes_value && min == 0 && max == 1;
            let nargs = if max == usize::MAX {
                if min >= 1 {
                    Nargs::AtLeastOne
                } else {
                    Nargs::Any
                }
            } else if min == max {
                Nargs::Exact(max)
            } else if min == 0 {
                Nargs::Any
            } else {
                // bounded variadic (e.g. `2..=4`): consumes up to `max`
                // values, unconditionally until the minimum is reached
                Nargs::Range(min, max)
            };
            (nargs, optional)
        }
        None => (Nargs::Exact(1), false),
    };

    let mut aliases: Vec<String> = arg
        .get_all_aliases()
        .into_iter()
        .flatten()
        .map(String::from)
        .collect();
    aliases.retain(|alias| Some(alias.as_str()) != arg.get_long());
    aliases.dedup();

    let name = arg
        .get_long()
        .unwrap_or_else(|| arg.get_id().as_str())
        .to_owned();
    let shorthand = arg.get_short();
    // the flag map is explicit registration, so it outranks the
    // clap-annotation fallbacks (possible values, value hints); the last
    // matching entry wins (the map merges per key)
    let map_completion = flag_completer
        .iter()
        .rev()
        .find(|(key, _)| {
            *key == name
                || aliases.iter().any(|alias| alias == key)
                || (key.chars().count() == 1 && shorthand == key.chars().next())
        })
        .map(|(_, action)| action.clone());

    Flag {
        id: arg.get_id().as_str().to_owned(),
        name,
        shorthand,
        shorthand_only: arg.get_long().is_none() && arg.get_short().is_some(),
        aliases,
        takes_value,
        nargs,
        optional,
        argument_style: argument_style(arg),
        repeatable,
        required: arg.is_required_set(),
        hidden: arg.is_hide_set(),
        persistent: arg.is_global_set(),
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
            .or(map_completion)
            .or_else(|| possible_values_action(arg))
            .or_else(|| value_hint_action(arg)),
        conflicts: arg
            .get::<ArgConflicts>()
            .map(|conflicts| conflicts.0.clone())
            .unwrap_or_default(),
        value_delimiter: arg.get_value_delimiter(),
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
        // the positional map and the clap-annotation fallbacks are applied
        // later in `convert_command`; the arg slot is the most explicit
        completion: arg
            .get::<ArgCompleter>()
            .map(|completer| completer.0.clone()),
        tag: None,
        value_delimiter: arg.get_value_delimiter(),
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
    (!values.is_empty()).then(|| {
        Action::values_described(
            values
                .iter()
                .map(|(value, description)| (value.as_str(), description.as_str())),
        )
    })
}

fn value_hint_action(arg: &clap::Arg) -> Option<Action> {
    use clap::builder::ValueHint;
    match arg.get_value_hint() {
        ValueHint::DirPath => Some(Action::directories()),
        ValueHint::AnyPath | ValueHint::FilePath => Some(Action::files([])),
        // command hints complete executables on PATH (Go has no dedicated
        // engine action; `pkg/actions/tools` provides richer ones)
        ValueHint::CommandName | ValueHint::CommandString | ValueHint::CommandWithArguments => {
            Some(Action::executables())
        }
        _ => None,
    }
}
