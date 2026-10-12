//! Example command tree (port of Go `carapace/example/cmd`).
//!
//! A deliberately wide command tree exercising the completion surfaces:
//! groups, persistent flags, optargs, embedded commands, shorthand
//! chains, disabled flag parsing, mutually exclusive flags and every
//! action modifier. The sandbox tests in `tests/example.rs` run against
//! this tree.
//!
//! Cobra-only surfaces are adapted: `Standalone` has no completion effect
//! here, the dynamic `PreRun` plugin command is attached statically, the
//! root `PreInvoke` chdir hook and the `--optarg` toggle for `flag` are
//! omitted (no completion behavior or test coverage), and `compat`
//! (cobra's `RegisterFlagCompletionFunc`/`ValidArgsFunction` directives)
//! is kept as a bare command since the directive semantics are covered by
//! the cobra bridge tests in `carapace-rs-bridge`.

mod action;
mod chain;
mod compat;
mod flag;
mod flag_disabled;
mod group;
mod interspersed;
mod modifier;
mod multiparts;
mod special;
mod subcommand;

use std::sync::{Arc, OnceLock};

use carapace_rs::Action;
use carapace_rs::model::Command as Model;
use carapace_rs_clap::Carapace;
use clap::builder::ArgAction;
use clap::{Arg, Command};

/// The root command of the example tree.
pub fn root_cmd() -> Command {
    root_inner(true)
}

/// The root command without the actions that reference the model snapshot
/// (used to build the snapshot itself without recursion).
fn root_basic() -> Command {
    root_inner(false)
}

fn root_inner(complete: bool) -> Command {
    let action_cmd = action::cmd(if complete {
        Some(commands_action())
    } else {
        None
    });
    // the dash completion resolves against the model snapshot (Go
    // `ActionPositional`); the snapshot build path must not attach it to
    // avoid reentrant snapshot initialization
    let action_cmd = if complete {
        action_cmd.dash_any_completion(carapace_rs::traverse::action_positional(&action_snapshot()))
    } else {
        action_cmd
    };

    // group declaration order drives the highlight styles (Go `AddGroup`):
    // main, modifier, plugin
    Command::new("example")
        .about("example completion")
        .use_line("example")
        .disable_help_flag(true)
        .disable_version_flag(true)
        .disable_help_subcommand(true)
        .subcommand(action_cmd.use_line("action [pos1] [pos2] [--] [dashAny]..."))
        .subcommand(flag::cmd().subcommand(flag_disabled::cmd()))
        .subcommand(modifier::cmd())
        .subcommand(
            Command::new("plugin")
                .about("dynamic plugin command")
                .completion_group("plugin")
                .positional_completion([Action::values(["pl1", "pluginArg1"])]),
        )
        .subcommand(chain::cmd())
        .subcommand(compat::cmd())
        // carapace's own hidden entry point (Go: the `_carapace`
        // subcommand added by `carapace.Gen`)
        .subcommand(Command::new("_carapace").hide(true))
        .subcommand(
            Command::new("completion")
                .about("Generate the autocompletion script for the specified shell")
                .subcommand(
                    Command::new("bash").about("Generate the autocompletion script for bash"),
                )
                .subcommand(
                    Command::new("fish").about("Generate the autocompletion script for fish"),
                )
                .subcommand(
                    Command::new("powershell")
                        .about("Generate the autocompletion script for powershell"),
                )
                .subcommand(
                    Command::new("zsh").about("Generate the autocompletion script for zsh"),
                ),
        )
        .subcommand(group::cmd())
        .subcommand(
            Command::new("help")
                .about("Help about any command")
                .use_line("help [command]"),
        )
        .subcommand(interspersed::cmd())
        .subcommand(multiparts::cmd())
        .subcommand(special::cmd())
        .subcommand(subcommand::cmd())
        .arg(
            Arg::new("chdir")
                .add(carapace_rs_clap::ArgPersistent)
                .short('C')
                .long("chdir")
                .action(ArgAction::Set)
                .help("change work directory"),
        )
        .arg(
            Arg::new("toggle")
                .add(carapace_rs_clap::ArgPersistent)
                .short('t')
                .long("toggle")
                .action(ArgAction::SetTrue)
                .help("Help message for toggle"),
        )
        .arg(
            Arg::new("persistentFlag")
                .add(carapace_rs_clap::ArgPersistent)
                .short('p')
                .long("persistentFlag")
                .action(ArgAction::Set)
                .num_args(0..=1)
                .default_missing_value("defaultValue")
                .help("Help message for persistentFlag"),
        )
        .arg(
            Arg::new("persistentFlag2")
                .add(carapace_rs_clap::ArgPersistent)
                .long("persistentFlag2")
                .action(ArgAction::Set)
                .help("Help message for persistentFlag2"),
        )
        .arg(
            Arg::new("array")
                .add(carapace_rs_clap::ArgPersistent)
                .short('a')
                .long("array")
                .action(ArgAction::Append)
                .help("multiflag"),
        )
        .arg(
            Arg::new("help")
                .short('h')
                .long("help")
                .action(ArgAction::SetTrue)
                .help("help for example"),
        )
        .arg(
            Arg::new("version")
                .short('v')
                .long("version")
                .action(ArgAction::SetTrue)
                .help("version for example"),
        )
        .flag_completion([
            ("chdir", Action::directories()),
            ("persistentFlag", Action::values(["p1", "p2", "p3"])),
            ("persistentFlag2", Action::values(["p4", "p5", "p6"])),
        ])
        // every action of the tree completes relative to the `--chdir`
        // flag's value (Go `Gen(rootCmd).PreInvoke`)
        .pre_invoke(std::sync::Arc::new(|c: &carapace_rs::Context, action| {
            let dir = c
                .parsed_flags
                .iter()
                .find(|(name, _)| name == "chdir")
                .and_then(|(_, values)| values.first().cloned())
                .unwrap_or_default();
            action.chdir(dir)
        }))
}

/// cobra's auto-generated `help` flag with its wording. Must be applied
/// before the command is built (clap adds the built-in help flag on
/// `build()` otherwise).
pub(crate) fn with_help(cmd: Command) -> Command {
    let name = cmd.get_name().to_owned();
    cmd.disable_help_flag(true)
        .disable_help_subcommand(true)
        .arg(
            Arg::new("help")
                .short('h')
                .long("help")
                .action(ArgAction::SetTrue)
                .help(format!("help for {name}")),
        )
        .subcommand(carapace_entry())
}

/// Help flag for standalone commands: carapace's `Standalone()` hides the
/// cobra-generated help flag, so it never completes.
pub(crate) fn with_hidden_help(cmd: Command) -> Command {
    cmd.disable_help_flag(true)
        .disable_help_subcommand(true)
        .arg(
            Arg::new("help")
                .short('h')
                .long("help")
                .action(ArgAction::SetTrue)
                .hide(true),
        )
        .subcommand(carapace_entry())
}

/// The hidden `_carapace` entry point subcommand (Go
/// `addCompletionCommand`): present on every `Gen`'ed command during
/// completion and only visible with `CARAPACE_HIDDEN=2`.
pub(crate) fn carapace_entry() -> Command {
    Command::new("_carapace").hide(true)
}

/// `ActionCommands(rootCmd).Split()`: completes subcommand paths of the
/// root, resolved against a snapshot of the model tree (the engine's
/// `action_commands` works on the normalized model).
fn commands_action() -> Action {
    carapace_rs::traverse::action_commands(&root_snapshot(), &["example".to_owned()])
        .split(carapace_rs::Format::Bash)
}

/// Snapshot of the example tree as the normalized model, built lazily on
/// first use (Go reads the package-level command tree directly).
pub(crate) fn root_snapshot() -> Arc<Model> {
    static SNAPSHOT: OnceLock<Arc<Model>> = OnceLock::new();
    SNAPSHOT
        .get_or_init(|| {
            let mut cmd = root_basic();
            Arc::new(carapace_rs_clap::to_model(&mut cmd))
        })
        .clone()
}

/// Snapshot of the `action` subcommand model (Go `ActionPositional(cmd)`).
pub(crate) fn action_snapshot() -> Arc<Model> {
    static SNAPSHOT: OnceLock<Arc<Model>> = OnceLock::new();
    SNAPSHOT
        .get_or_init(|| {
            let model = root_snapshot();
            let action = model
                .lookup_subcommand("action")
                .expect("action subcommand in snapshot");
            Arc::new(action.clone())
        })
        .clone()
}
