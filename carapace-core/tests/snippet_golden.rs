//! Snippet golden checks per shell (adapted from Go's
//! `internal/shell/multi/snippet_test.go` to the single-command snippets
//! the port generates).

use carapace_core::shell::{Shell, bash, elvish, fish, nushell, powershell, simple, xonsh, zsh};

const BINARY: &str = "carapace-test";

#[test]
fn bash_snippet() {
    let s = bash::snippet(BINARY);
    assert!(s.contains("#!/bin/bash"), "missing bash shebang");
    assert!(
        s.contains(format!("_{BINARY}_completion() {{").as_str()),
        "missing completer function"
    );
    assert!(
        s.contains("xargs carapace-test _carapace bash"),
        "missing invocation"
    );
    assert!(s.contains("complete -o noquote -F _carapace-test_completion carapace-test"));
}

#[test]
fn zsh_snippet() {
    let s = zsh::snippet(BINARY);
    assert!(s.contains("#compdef"), "missing compdef header");
    assert!(
        s.contains(format!("_{BINARY}_completion").as_str()),
        "missing completer function"
    );
    assert!(
        s.contains("CARAPACE_COMPLINE="),
        "missing compline handover"
    );
    assert!(
        s.contains("CARAPACE_ZSH_HASH_DIRS=\"$(hash -d)\""),
        "missing hash dirs"
    );
    // the wire format has four fields; the snippet must read all of them
    assert!(
        s.contains("read -r -d '' zstyle message noprefix data"),
        "missing noprefix field"
    );
    assert!(
        s.contains("compstate[insert]=menu"),
        "missing noprefix handling"
    );
}

#[test]
fn fish_snippet() {
    let s = fish::snippet(BINARY);
    assert!(
        s.contains(format!("function _{BINARY}_completion").as_str()),
        "missing completer function"
    );
    assert!(
        s.contains("commandline -cp"),
        "missing commandline reference"
    );
    assert!(
        s.contains(format!("complete -c '{BINARY}' -f -a '(_{BINARY}_completion)' -r").as_str())
    );
}

#[test]
fn elvish_snippet() {
    let s = elvish::snippet(BINARY);
    assert!(
        s.contains("edit:completion:arg-completer"),
        "missing arg-completer"
    );
    assert!(
        s.contains("_carapace elvish (all $arg)"),
        "missing invocation"
    );
    // messages and usage surface through edit:notify
    assert!(
        s.contains("$completion[Messages]"),
        "missing message handling"
    );
    assert!(s.contains("$completion[Usage]"), "missing usage handling");
    assert!(
        s.contains("edit:complex-candidate"),
        "missing candidate rendering"
    );
}

#[test]
fn nushell_snippet() {
    let s = nushell::snippet(BINARY);
    assert!(
        s.contains("carapace__test_completer"),
        "missing sanitized completer name"
    );
    assert!(s.contains("$place | describe"), "missing record workaround");
    assert!(
        s.contains("_carapace nushell ...$spans"),
        "missing invocation"
    );
}

#[test]
fn powershell_snippet() {
    let s = powershell::snippet(BINARY);
    assert!(
        s.contains("System.Management.Automation"),
        "missing namespace"
    );
    assert!(
        s.contains("_carapace__test_completer"),
        "missing completer function"
    );
    assert!(
        s.contains("Register-ArgumentCompleter -Native -ScriptBlock"),
        "missing registration"
    );
    assert!(s.contains("ConvertFrom-Json"), "missing JSON parsing");
}

#[test]
fn xonsh_snippet() {
    let s = xonsh::snippet(BINARY);
    assert!(
        s.contains("contextual_command_completer"),
        "missing contextual completer"
    );
    assert!(s.contains("_carapace', 'xonsh'"), "missing invocation");
    assert!(
        s.contains("RichCompletion"),
        "missing rich completion mapping"
    );
    assert!(s.contains("add_one_completer"), "missing registration");
}

#[test]
fn oil_snippet() {
    let s = simple::oil::snippet(BINARY);
    assert!(s.contains("#!/bin/osh"), "missing osh shebang");
    assert!(
        s.contains(format!("_{BINARY}_completion() {{").as_str()),
        "missing completer function"
    );
    assert!(s.contains("_carapace oil"), "missing invocation");
    assert!(s.contains("compopt -o nospace"), "missing nospace handling");
}

#[test]
fn tcsh_snippet() {
    let s = simple::tcsh::snippet(BINARY);
    assert!(
        s.contains("complete \"carapace-test\""),
        "missing registration"
    );
    assert!(
        s.contains("$COMMAND_LINE"),
        "missing command line reference"
    );
    assert!(s.contains("_carapace tcsh"), "missing invocation");
}

#[test]
fn bash_ble_snippet() {
    let s = simple::bash_ble::snippet(BINARY);
    // the bash body is wrapped, not replaced
    assert!(s.contains("#!/bin/bash"), "missing bash body");
    assert!(s.contains("BLE_ATTACHED"), "missing ble guard");
    assert!(
        s.contains("ble/complete/cand/yield mandb"),
        "missing candidate yield"
    );
    assert!(s.contains(format!("complete -F _{BINARY}_completion_ble {BINARY}").as_str()));
}

#[test]
fn cmd_clink_snippet() {
    let s = simple::cmd_clink::snippet(BINARY);
    assert!(s.contains("CARAPACE_COMPLINE"), "missing compline handover");
    assert!(s.contains("io.popenyield"), "missing async popen");
    assert!(
        s.contains("match_builder:addmatch"),
        "missing match building"
    );
    assert!(s.contains("clink.argmatcher(50, 'carapace-test', 'carapace-test.exe')"));
}

#[test]
fn unsupported_shell_is_rejected() {
    assert!(Shell::parse("unsupported").is_none());
    assert!(Shell::parse("export").is_some());
}
