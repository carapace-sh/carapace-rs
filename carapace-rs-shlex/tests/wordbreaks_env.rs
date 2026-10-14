//! `COMP_WORDBREAKS` env-isolation coverage (ports of Go
//! `TestVerify_CompletionWordBreaks` and
//! `TestVerify_FishNoEnvVarDependency`).
//!
//! Lives in its own integration test binary so the env mutation cannot
//! race the lib test binary, whose bash-classifier assertions assume the
//! default state.

use carapace_rs_shlex::{Format, TokenType, split};

#[test]
fn completion_word_breaks_and_env_isolation() {
    let saved = std::env::var("COMP_WORDBREAKS").ok();
    unsafe {
        std::env::remove_var("COMP_WORDBREAKS");
    }

    // default: @ is a wordbreak, so `a@b` lexes as word @ word
    let tokens = split("echo a@b", Format::Bash).unwrap();
    assert!(
        tokens
            .iter()
            .any(|t| t.kind == TokenType::Wordbreak && t.value == "@"),
        "{tokens:?}"
    );

    // hostname completion off: COMP_WORDBREAKS without @ keeps `a@b` whole
    unsafe {
        std::env::set_var("COMP_WORDBREAKS", " \t\n\"'><=;|&(:");
    }
    let tokens = split("echo a@b", Format::Bash).unwrap();
    assert!(
        tokens
            .iter()
            .any(|t| t.kind == TokenType::Word && t.value == "a@b"),
        "{tokens:?}"
    );

    // the fish format must be identical regardless of COMP_WORDBREAKS:
    // separators always split, `@` never does
    let tokens = split("echo a|b", Format::Fish).unwrap();
    assert!(tokens.iter().any(|t| t.value == "|"), "{tokens:?}");
    let tokens = split("echo a@b", Format::Fish).unwrap();
    assert!(tokens.iter().any(|t| t.value == "a@b"), "{tokens:?}");

    match saved {
        Some(value) => unsafe { std::env::set_var("COMP_WORDBREAKS", value) },
        None => unsafe { std::env::remove_var("COMP_WORDBREAKS") },
    }
}
