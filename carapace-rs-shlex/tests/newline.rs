use carapace_rs_shlex::{Format, TokenSliceExt, join, split};

fn strings_of(joined: &str, format: Format) -> Vec<String> {
    split(joined, format).unwrap().words().strings()
}

#[test]
fn quoting_preserves_newline() {
    // Paths and values dropped the newline in bash/zsh/oil (backslash-
    // newline inside double quotes is a line continuation), fish (backslash-
    // newline dropped there too, raw newline kept instead), and tcsh
    // (backslash-newline preserved inside quotes). Xonsh and Lua escape the
    // newline as `\n` and their lexers do not unescape it, so they
    // round-trip the escape rather than the literal newline.
    for format in [
        Format::Default,
        Format::Bash,
        Format::Zsh,
        Format::Oil,
        Format::Tcsh,
        Format::Fish,
    ] {
        let joined = join(&["words".to_owned(), "a\nb".to_owned()], format);
        assert_eq!(
            strings_of(&joined, format),
            vec!["words".to_owned(), "a\nb".to_owned()],
            "format {format:?} (joined={joined:?})"
        );
    }
}
