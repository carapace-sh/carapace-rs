use carapace_rs::Action;
use carapace_rs_clap::Carapace;
use carapace_rs_testkit::Sandbox;
use clap::{Arg, Command};

fn cli() -> Command {
    Command::new("myapp")
        .disable_help_subcommand(true)
        .arg(Arg::new("target").index(1))
        .subcommand(Command::new("build").about("build things"))
}

#[test]
fn sandbox_completes_positionals() {
    let sandbox = Sandbox::new("positional");
    sandbox.run(cli, &[""]).expect(
        Action::values_described([("build", "build things")])
            .tag("commands")
            .uid_f(|value| carapace_rs::uid::command("myapp", &[value])),
    );
}

#[test]
fn sandbox_expect_not() {
    let sandbox = Sandbox::new("expectnot");
    sandbox
        .run(cli, &["build", ""])
        .expect_not(Action::values(["nonsense"]));
}

#[test]
fn sandbox_files_visible_to_actions() {
    let mut sandbox = Sandbox::new("files");
    sandbox.files([("data", "content"), ("notes.txt", "content")]);

    let run = sandbox.run(
        || {
            Command::new("myapp").arg(
                Arg::new("file")
                    .index(1)
                    .value_hint(clap::builder::ValueHint::FilePath),
            )
        },
        &[""],
    );
    let values = run.values();
    assert!(
        values.iter().any(|v| v.ends_with("notes.txt")),
        "{values:?}"
    );
    assert!(values.iter().any(|v| v.ends_with("data")), "{values:?}");
}

#[test]
fn sandbox_replies_mock_commands() {
    let mut sandbox = Sandbox::new("replies");
    sandbox
        .reply("git", &["branch", "--list"])
        .with("main\nfeature");

    sandbox
        .run(
            || {
                let cmd = Command::new("myapp").arg(Arg::new("branch").index(1));
                cmd.positional_completion([Action::exec_command(
                    "git",
                    ["branch", "--list"],
                    |stdout| {
                        let output = String::from_utf8_lossy(stdout).into_owned();
                        let branches: Vec<String> = output
                            .lines()
                            .filter(|line| !line.is_empty())
                            .map(String::from)
                            .collect();
                        Action::values_with(branches)
                    },
                )])
            },
            &[""],
        )
        .expect(Action::values(["main", "feature"]));
}

#[test]
fn sandbox_env_reaches_actions() {
    let mut sandbox = Sandbox::new("env");
    sandbox.env("SANDBOX_ENV_TEST", "mocked");

    sandbox
        .run(
            || {
                let cmd = Command::new("myapp").arg(Arg::new("value").index(1));
                cmd.positional_completion([Action::callback(|context| {
                    Action::values_with([context.get_env("SANDBOX_ENV_TEST")])
                })])
            },
            &[""],
        )
        .expect(Action::values(["mocked"]));
}

#[test]
fn sandbox_cache_persists_across_runs() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn cached_cmd(calls: &Arc<AtomicUsize>) -> Command {
        let calls = calls.clone();
        let cmd = Command::new("myapp").arg(Arg::new("value").index(1));
        cmd.positional_completion([Action::callback(move |_| {
            calls.fetch_add(1, Ordering::SeqCst);
            Action::values(["cached"])
        })
        .cache("sandbox-cache-site", 60, vec!["k".into()])])
    }

    let calls = Arc::new(AtomicUsize::new(0));
    let sandbox = Sandbox::new("sandbox-cache");
    let run1 = sandbox.run(|| cached_cmd(&calls), &[""]);
    let run2 = sandbox.run(|| cached_cmd(&calls), &[""]);
    assert_eq!(run1.values(), vec!["cached".to_string()]);
    assert_eq!(run2.values(), vec!["cached".to_string()]);
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "second run must hit the cache"
    );
}
