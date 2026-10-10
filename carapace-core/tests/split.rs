use std::sync::Arc;

use carapace_core::lex::Format;
use carapace_core::{Action, Command, Context, Flag, Positional};

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

fn split(value: &str, dir: Option<&std::path::Path>) -> carapace_core::InvokedAction {
    let mut context = Context::default().value(value);
    if let Some(dir) = dir {
        context = context.dir(dir);
    }
    inner_action().split(Format::default()).invoke(&context)
}

/// Values as the shared dispatch would deliver them (sorted).
fn values(invoked: &carapace_core::InvokedAction) -> Vec<String> {
    let mut values: Vec<String> = invoked.values.iter().map(|v| v.value.clone()).collect();
    values.sort();
    values
}

#[test]
fn split_completes_first_positional_with_space() {
    let invoked = split("", None);
    assert_eq!(values(&invoked), vec!["pos1 ", "positional1 "]);
    assert_eq!(invoked.meta.nospace, vec!['*']);
}

#[test]
fn split_completes_second_positional_after_space() {
    let dir = std::env::temp_dir().join(format!("carapace-split-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("subdir")).unwrap();
    std::fs::write(dir.join("file1.txt"), "").unwrap();

    let invoked = split("pos1 ", Some(&dir));
    assert_eq!(values(&invoked), vec!["pos1 file1.txt ", "pos1 subdir/"]);
    assert_eq!(invoked.meta.nospace, vec!['*']);

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
    assert_eq!(invoked.meta.nospace, vec!['*']);
}
