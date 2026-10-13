use std::sync::Arc;

use carapace_rs::{Action, Command, Context, Flag, Positional};
use carapace_rs_shlex::Format;

fn inner_action() -> Action {
    let mut cmd = Command::new("inner");
    cmd.flags.push(Flag {
        id: "bool".into(),
        name: "bool".into(),
        completion: Some(Action::values(["true", "false"])),
        ..Default::default()
    });
    cmd.positionals.push(Positional {
        name: "target".into(),
        completion: Some(Action::values(["pos1", "positional1"])),
        ..Default::default()
    });
    cmd.positionals.push(Positional {
        name: "rest".into(),
        completion: Some(Action::files(Vec::<String>::new())),
        ..Default::default()
    });
    Action::execute(Arc::new(cmd))
}

fn split(value: &str, dir: Option<&std::path::Path>) -> carapace_rs::InvokedAction {
    let mut context = Context::default().value(value);
    if let Some(dir) = dir {
        context = context.dir(dir);
    }
    inner_action().split(Format::default()).invoke(&context)
}

/// Values as the shared dispatch would deliver them (sorted).
fn values(invoked: &carapace_rs::InvokedAction) -> Vec<String> {
    let mut values: Vec<String> = invoked
        .to_export()
        .values
        .iter()
        .map(|v| v.value.clone())
        .collect();
    values.sort();
    values
}

#[test]
fn split_completes_first_positional_with_space() {
    let invoked = split("", None);
    assert_eq!(values(&invoked), vec!["pos1 ", "positional1 "]);
    assert_eq!(invoked.to_export().meta.nospace, "*");
}

#[test]
fn split_completes_second_positional_after_space() {
    let dir = std::env::temp_dir().join(format!("carapace-split-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("subdir")).unwrap();
    std::fs::write(dir.join("file1.txt"), "").unwrap();

    let invoked = split("pos1 ", Some(&dir));
    assert_eq!(values(&invoked), vec!["pos1 file1.txt ", "pos1 subdir/"]);
    // the files action contributes '/' to the nospace matcher (Go's
    // MultiParts("/")), merged with split's own '*'
    assert_eq!(invoked.to_export().meta.nospace, "*");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn split_completes_flag_value_with_inner_prefix() {
    let invoked = split("pos1 --bool=", None);
    assert_eq!(
        values(&invoked),
        vec!["pos1 --bool=false ", "pos1 --bool=true "]
    );
}

#[test]
fn split_closes_open_quote() {
    let invoked = split("pos1 '--bool=", None);
    assert_eq!(
        values(&invoked),
        vec!["pos1 '--bool=false' ", "pos1 '--bool=true' "]
    );
}

#[test]
fn split_completes_environment_variables() {
    let mut context = Context::default().value("echo café | grep $HO");
    context.env.insert("HOME".into(), "/tmp".into());
    let invoked = inner_action().split(Format::default()).invoke(&context);
    assert_eq!(values(&invoked), vec!["echo café | grep $HOME"]);
    assert_eq!(invoked.to_export().meta.nospace, "*");
}

#[test]
fn split_p_completes_files_for_redirect() {
    let dir = std::env::temp_dir().join(format!("carapace-splitp-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("file1.txt"), "").unwrap();

    let mut context = Context::default().value("pos1 > ");
    context.dir = dir.clone();
    let invoked = inner_action().split_p(Format::default()).invoke(&context);
    // the redirect target completes files
    assert_eq!(values(&invoked), vec!["pos1 > file1.txt "]);
    assert_eq!(invoked.to_export().meta.nospace, "*");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn split_p_restarts_args_at_pipeline_delimiter() {
    // After the pipe the current pipeline restarts: args are empty, so the
    // first positional completes again (Split would pass the earlier
    // pipeline's words plus the delimiter as args).
    let invoked = split_p("pos1 | ");
    assert_eq!(
        values(&invoked),
        vec!["pos1 | pos1 ", "pos1 | positional1 "]
    );
}

fn split_p(value: &str) -> carapace_rs::InvokedAction {
    let context = Context::default().value(value);
    inner_action().split_p(Format::default()).invoke(&context)
}
