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
pub fn complete_output(
    factory: impl Fn() -> clap::Command,
    shell: Shell,
    words: &[String],
) -> String {
    let mut cmd = factory();
    let model = crate::to_model(&mut cmd);
    let completion = carapace_core::traverse(&model, words.to_vec());
    let invoked = completion.action.invoke(&completion.context);

    // shell.Value post-processing: filter by current value, then sort
    let mut values: Vec<carapace_core::RawValue> = invoked
        .values
        .into_iter()
        .filter(|value| value.value.starts_with(&completion.context.value))
        .collect();
    values.sort_by(carapace_core::value::by_display);

    shell.value(&completion.context.value, &invoked.meta, &values)
}

/// Generate the shell integration snippet for the command.
pub fn snippet(factory: impl Fn() -> clap::Command, shell: Shell) -> String {
    let cmd = factory();
    shell.snippet(cmd.get_name())
}
