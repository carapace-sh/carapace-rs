//! `_carapace` entry point dispatching completion requests.

use carapace_core::shell::{Shell, bash};

/// Handle a `_carapace` invocation.
///
/// Returns the formatted completion output when `args` is a completion
/// request (`["_carapace", "<shell>", ...words]`), or `None` when the
/// program should run normally.
pub fn complete(factory: impl Fn() -> clap::Command, args: &[String]) -> Option<String> {
    if args.first().map(String::as_str) != Some("_carapace") {
        return None;
    }
    match args.len() {
        // no shell given: print the snippet for the detected shell
        1 => Shell::detect().map(|shell| snippet(factory, shell)),
        // named shell: print its snippet
        2 => Shell::parse(&args[1]).map(|shell| snippet(factory, shell)),
        // completion request
        _ => {
            let shell = Shell::parse(&args[1])?;
            Some(complete_output(factory, shell, &args[2..]))
        }
    }
}

/// Run completion for the given shell and word list, returning formatted output.
///
/// The shared shell dispatch applies prefix filtering, flag merging,
/// message integration, nospace propagation and sorting. Bash performs
/// COMP_LINE re-lexing and redirect filtering first.
pub fn complete_output(
    factory: impl Fn() -> clap::Command,
    shell: Shell,
    words: &[String],
) -> String {
    let mut cmd = factory();
    let model = crate::to_model(&mut cmd);
    let completion = complete_model(&model, shell, words);
    let invoked = completion.action.invoke(&completion.context);
    shell.value(&completion.context.value, &invoked.meta, &invoked.values)
}

fn complete_model(
    model: &carapace_core::Command,
    shell: Shell,
    words: &[String],
) -> carapace_core::Completion {
    match shell {
        Shell::Bash => match bash::patch(words.to_vec()) {
            bash::Patch::Redirect => {
                let context = carapace_core::Context::default().args(words.to_vec());
                carapace_core::Completion {
                    action: carapace_core::Action::files(Vec::<String>::new()),
                    context,
                }
            }
            bash::Patch::Patched {
                args,
                wordbreak_prefix,
                comp_type,
            } => {
                bash::set_patch_context(bash::Context {
                    wordbreak_prefix: wordbreak_prefix.clone(),
                    comp_type: comp_type.clone(),
                    wordbreaks: std::env::var("COMP_WORDBREAKS").unwrap_or_default(),
                });
                let completion = carapace_core::traverse(model, args);
                let mut invoked_context = completion.context.clone();
                // wordbreak prefix: values are trimmed during formatting
                if !wordbreak_prefix.is_empty() {
                    invoked_context.value = format!("{wordbreak_prefix}{}", invoked_context.value);
                }
                carapace_core::Completion {
                    action: completion.action,
                    context: invoked_context,
                }
            }
            bash::Patch::None => carapace_core::traverse(model, words.to_vec()),
        },
        _ => carapace_core::traverse(model, words.to_vec()),
    }
}

/// Generate the shell integration snippet for the command.
pub fn snippet(factory: impl Fn() -> clap::Command, shell: Shell) -> String {
    let cmd = factory();
    shell.snippet(cmd.get_name())
}
