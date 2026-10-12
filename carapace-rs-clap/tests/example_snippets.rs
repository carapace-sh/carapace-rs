//! Snippet golden fixtures for the example command tree (port of Go
//! `example/cmd/root_test.go`'s `testScript` with its `_test/` fixture
//! files). The fixtures document the Rust snippets' own shape (command
//! name instead of Go's `uid.Executable()` path); regenerate them with
//! `cargo run -p carapace-rs-clap --example example -- _carapace <shell>
//! > examples/example/_test/<file>`.

#[path = "../examples/example/cmd/mod.rs"]
mod cmd;

use carapace_rs::shell::Shell;

const FIXTURES: &[(Shell, &str)] = &[
    (Shell::Bash, "bash.sh"),
    (Shell::BashBle, "bash-ble.sh"),
    (Shell::CmdClink, "cmd-clink.lua"),
    (Shell::Elvish, "elvish.elv"),
    (Shell::Fish, "fish.fish"),
    (Shell::Nushell, "nushell.nu"),
    (Shell::Oil, "oil.sh"),
    (Shell::PowerShell, "powershell.ps1"),
    (Shell::Tcsh, "tcsh.csh"),
    (Shell::Xonsh, "xonsh.py"),
    (Shell::Zsh, "zsh.sh"),
];

#[test]
fn snippets_match_fixtures() {
    for (shell, file) in FIXTURES {
        let snippet = carapace_rs_clap::snippet(cmd::root_cmd, *shell);
        let fixture = std::fs::read_to_string(format!(
            "{}/examples/example/_test/{file}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap_or_else(|error| panic!("missing fixture {file}: {error}"));
        assert_eq!(snippet + "\n", fixture, "snippet mismatch for {file}");
    }
}
