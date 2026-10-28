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
    // _carapace macro [name [words...]]: run a registered custom macro
    // (Go `spec.Register`'s macro subcommand; the external `$<exe>.name`
    // protocol consumes the export output)
    #[cfg(feature = "spec")]
    if args.get(1).map(String::as_str) == Some("macro") {
        return Some(macro_subcommand(&args[2..]));
    }
    // _carapace style set <name> <style>: persist to styles.json
    if args.get(1).map(String::as_str) == Some("style")
        && args.get(2).map(String::as_str) == Some("set")
    {
        if let (Some(name), Some(style)) = (args.get(3), args.get(4)) {
            return Some(match style_set(name, style) {
                Ok(()) => String::new(),
                Err(error) => error,
            });
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
    let export = invoked.to_export();
    shell.value(
        &completion.context.value,
        &export.meta.to_meta(),
        &export.to_raw_values(),
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
                    action: carapace_rs::Action::files([]),
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

/// Handle the `_carapace macro` subcommand (Go `register.go`): list the
/// registered macros, report a signature placeholder, or run one and emit
/// its result as export JSON.
#[cfg(feature = "spec")]
fn macro_subcommand(args: &[String]) -> String {
    match args.len() {
        0 => {
            let exe = std::env::current_exe()
                .ok()
                .and_then(|path| {
                    path.file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                })
                .unwrap_or_default();
            let entries: Vec<serde_json::Value> = carapace_rs_spec::macro_names()
                .iter()
                .map(|name| {
                    serde_json::json!({
                        "name": format!("{exe}.{name}"),
                        "signature": "\u{2014}",
                        "description": carapace_rs_spec::macro_description(name)
                            .unwrap_or_default(),
                    })
                })
                .collect();
            format!(
                "{{\"macros\":{}}}",
                serde_json::to_string(&entries).unwrap_or_default()
            )
        }
        1 => {
            // the Rust macro functions have no typed signature; Go prints
            // the em dash placeholder when none exists
            match carapace_rs_spec::macro_description(&args[0]) {
                Some(_) => "\u{2014}".to_owned(),
                None => format!("unknown macro: {}", args[0]),
            }
        }
        _ => {
            let (value, words) = args[1..].split_last().expect("len >= 2");
            match carapace_rs_spec::invoke_macro(&args[0], words, value) {
                Some(action) => {
                    let context = carapace_rs::Context::new(args[1..].to_vec());
                    let invoked = action.invoke(&context);
                    invoked.to_export().to_json()
                }
                None => format!("unknown macro: {}", args[0]),
            }
        }
    }
}

/// Merge a style entry into the user's `styles.json`. I/O errors surface
/// to the caller instead of being swallowed (Go `style.Set` reports the
/// error on stderr).
fn style_set(name: &str, style: &str) -> Result<(), String> {
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
        return Err("failed to resolve config dir".to_owned());
    };
    let path = config_dir.join("styles.json");
    // a missing or unreadable file only means the defaults apply
    let mut styles: std::collections::HashMap<String, String> = std::fs::read_to_string(&path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
        .unwrap_or_default();
    styles.insert(name.to_owned(), style.to_owned());
    let json = serde_json::to_string_pretty(&styles).map_err(|error| error.to_string())?;
    std::fs::create_dir_all(&config_dir).map_err(|error| error.to_string())?;
    std::fs::write(&path, json).map_err(|error| error.to_string())?;
    Ok(())
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
