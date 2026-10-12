//! `_carapace` entry point dispatching completion requests.

use std::path::PathBuf;

use carapace_rs::shell::{Shell, bash};

/// Handle a `_carapace` invocation.
///
/// Returns the formatted completion output when `args` is a completion
/// request (`["_carapace", "<shell>", "<command name>", ...words]`, the
/// command name included as the shells' snippets send it), or `None` when
/// the program should run normally.
pub fn complete(factory: impl Fn() -> clap::Command, args: &[String]) -> Option<String> {
    if args.first().map(String::as_str) != Some("_carapace") {
        return None;
    }
    carapace_rs::log::log(&"-".repeat(80));
    carapace_rs::log::log(&format!("{args:?}"));
    // _carapace spec: dump the command tree as YAML spec
    #[cfg(feature = "spec")]
    if args.get(1).map(String::as_str) == Some("spec") {
        let mut cmd = factory();
        let model = crate::to_model(&mut cmd);
        return Some(carapace_rs_spec::to_yaml(&model));
    }
    // _carapace style set <name> <style>: persist to styles.json
    if args.get(1).map(String::as_str) == Some("style")
        && args.get(2).map(String::as_str) == Some("set")
    {
        if let (Some(name), Some(style)) = (args.get(3), args.get(4)) {
            style_set(name, style);
            return Some(String::new());
        }
        return Some("usage: _carapace style set <name> <style>".to_owned());
    }
    match args.len() {
        // no shell given: print the snippet for the detected shell
        1 => Shell::detect().map(|shell| snippet(factory, shell)),
        // named shell: print its snippet
        2 => Shell::parse(&args[1]).map(|shell| snippet(factory, shell)),
        // completion request: the first word after the shell is the command
        // name as the shell handed it over (Go's complete() drops it before
        // traversing, mirroring the snippets which pass the full compline)
        _ => {
            let shell = Shell::parse(&args[1])?;
            Some(complete_output(factory, shell, &args[3..]))
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

    let (completion, bash_context) = match shell {
        Shell::Bash => complete_bash(&model, words),
        // shells handing over quoted words: dequote before traversal
        Shell::Nushell => (
            carapace_rs::traverse(&model, carapace_rs::shell::nushell::patch(words.to_vec())),
            bash::Context::default(),
        ),
        Shell::CmdClink => (
            carapace_rs::traverse(
                &model,
                carapace_rs::shell::simple::cmd_clink::patch(words.to_vec()),
            ),
            bash::Context::default(),
        ),
        _ => (
            carapace_rs::traverse(&model, words.to_vec()),
            bash::Context::default(),
        ),
    };
    let invoked = completion.action.invoke(&completion.context);
    shell.value(
        &completion.context.value,
        &invoked.meta,
        &invoked.values,
        &bash_context,
    )
}

/// Traverse with bash patching applied; returns the completion together
/// with the patch context the formatter needs (wordbreak prefix, COMP_TYPE,
/// COMP_WORDBREAKS).
fn complete_bash(
    model: &carapace_rs::Command,
    words: &[String],
) -> (carapace_rs::Completion, bash::Context) {
    match bash::patch(words.to_vec()) {
        bash::Patch::Redirect => {
            let context = carapace_rs::Context::default().args(words.to_vec());
            (
                carapace_rs::Completion {
                    action: carapace_rs::Action::files(Vec::<String>::new()),
                    context,
                },
                bash::Context::default(),
            )
        }
        bash::Patch::Patched {
            args,
            wordbreak_prefix,
            comp_type,
        } => {
            let completion = carapace_rs::traverse(model, args);
            (
                completion,
                bash::Context {
                    // the current word already includes the wordbreak prefix;
                    // the formatter trims it from the values so bash
                    // re-attaches the prefix itself
                    wordbreak_prefix,
                    comp_type,
                    wordbreaks: std::env::var("COMP_WORDBREAKS").unwrap_or_default(),
                },
            )
        }
        bash::Patch::None => {
            let completion = carapace_rs::traverse(model, words.to_vec());
            (completion, bash::Context::default())
        }
    }
}

/// Execute completion against an embedded clap command (multi-completer
/// pattern): completes the embedded tree in-process with the current
/// context args and value.
pub fn execute_embedded(cmd: clap::Command) -> carapace_rs::Action {
    let mut cmd = cmd;
    let model = crate::to_model(&mut cmd);
    carapace_rs::Action::execute(std::sync::Arc::new(model))
}

/// Merge a style entry into the user's `styles.json`.
fn style_set(name: &str, style: &str) {
    let Some(config_dir) = std::env::var("XDG_CONFIG_HOME")
        .ok()
        .filter(|dir| !dir.is_empty())
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var("HOME")
                .ok()
                .map(|home| PathBuf::from(home).join(".config"))
        })
        .map(|dir| dir.join("carapace"))
    else {
        return;
    };
    let path = config_dir.join("styles.json");
    let mut styles: std::collections::HashMap<String, String> = std::fs::read_to_string(&path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
        .unwrap_or_default();
    styles.insert(name.to_owned(), style.to_owned());
    if let Ok(json) = serde_json::to_string_pretty(&styles) {
        let _ = std::fs::create_dir_all(&config_dir);
        let _ = std::fs::write(&path, json);
    }
}

/// Generate the shell integration snippet for the command.
///
/// The `export` shell has no integration script; it serializes the command
/// tree as JSON instead (Go `internal/shell/export.Snippet`).
pub fn snippet(factory: impl Fn() -> clap::Command, shell: Shell) -> String {
    if shell == Shell::Export {
        let mut cmd = factory();
        let model = crate::to_model(&mut cmd);
        return carapace_rs::shell::export::snippet(&model);
    }
    let cmd = factory();
    shell.snippet(cmd.get_name())
}
