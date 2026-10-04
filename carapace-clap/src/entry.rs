//! `_carapace` entry point dispatching completion requests.

use carapace_core::shell::Shell;

/// Handle a `_carapace` invocation.
///
/// Returns the formatted completion output when `args` is a completion
/// request (`["_carapace", "<shell>", ...words]`), or `None` when the
/// program should run normally.
pub fn complete(factory: impl Fn() -> clap::Command, args: &[String]) -> Option<String> {
    if args.first().map(String::as_str) != Some("_carapace") {
        return None;
    }
    let shell = Shell::parse(args.get(1)?)?;
    Some(complete_output(factory, shell, &args[2..]))
}

/// Run completion for the given shell and word list, returning formatted output.
///
/// The shared shell dispatch applies prefix filtering, flag merging,
/// message integration, nospace propagation and sorting.
pub fn complete_output(
    factory: impl Fn() -> clap::Command,
    shell: Shell,
    words: &[String],
) -> String {
    let mut cmd = factory();
    let model = crate::to_model(&mut cmd);
    let completion = carapace_core::traverse(&model, words.to_vec());
    let invoked = completion.action.invoke(&completion.context);
    shell.value(&completion.context.value, &invoked.meta, &invoked.values)
}

/// Generate the shell integration snippet for the command.
pub fn snippet(factory: impl Fn() -> clap::Command, shell: Shell) -> String {
    let cmd = factory();
    shell.snippet(cmd.get_name())
}
