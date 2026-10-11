//! Port of Go `context_test.go`: `TestContextAbs`, `TestEnv`, `TestEnvsubst`.

use carapace_core::Context;

fn wd() -> String {
    std::env::current_dir()
        .map(|dir| dir.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default()
}

fn parent_of(suffix: &str) -> String {
    // Go's parent(): the parent directory of cwd, with a trailing slash
    let wd = wd();
    let parent = std::path::Path::new(&wd)
        .parent()
        .map(|dir| dir.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|| "/".to_owned());
    format!("{parent}/{suffix}")
}

fn home() -> String {
    std::env::var(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .unwrap_or_default()
        .replace('\\', "/")
}

#[test]
fn context_abs() {
    let cases = [
        ("/", "file", "/file".to_owned()),
        ("", "file", format!("{}/file", wd())),
        ("", "../", parent_of("")),
        ("", "../file", parent_of("file")),
        ("", "~/file", format!("{}/file", home())),
        ("/", "~/file", format!("{}/file", home())),
        ("/", "file", "/file".to_owned()),
        ("/dir", "file", "/dir/file".to_owned()),
        ("/dir", "./.file", "/dir/.file".to_owned()),
        ("", "/dir/", "/dir/".to_owned()),
        ("/dir/", "", "/dir/".to_owned()),
        ("~/", "file", format!("{}/file", home())),
        ("", "/", "/".to_owned()),
        ("", ".hidden", format!("{}/.hidden", wd())),
        ("", "./", format!("{}/", wd())),
        ("", "", format!("{}/", wd())),
        ("", ".", format!("{}/.", wd())),
        ("", "~", home()),
        ("", "~/file", format!("{}/file", home())),
    ];

    for (dir, path, expected) in cases {
        let context = Context::default().dir(dir);
        let actual = context.abs(path).unwrap();
        assert_eq!(actual, expected, "context: {dir:?} arg: {path:?}");
    }
}

#[test]
fn context_env() {
    let mut context = Context::default();
    assert_eq!(context.get_env("example"), "");
    assert_eq!(context.lookup_env("example"), None);

    context = context.env("example", "value");
    assert_eq!(context.get_env("example"), "value");
    assert_eq!(context.lookup_env("example"), Some("value"));

    context = context.env("example", "newvalue");
    assert_eq!(context.get_env("example"), "newvalue");
    assert_eq!(context.lookup_env("example"), Some("newvalue"));
}

#[test]
fn envsubst() {
    let context = Context::default()
        .env("REPLACE", "me")
        .env("REPEAT", "banana");
    let cases = [
        ("BEFORE${REPLACE}AFTER", "BEFOREmeAFTER"),
        ("BEFORE${!REPLACE}AFTER", "BEFORE${REPLACE}AFTER"),
        ("BEFORE${REPLACE:-default}AFTER", "BEFOREmeAFTER"),
        (
            "BEFORE${!REPLACE:-default}AFTER",
            "BEFORE${REPLACE:-default}AFTER",
        ),
        ("BEFORE${REPLACE/me/you}AFTER", "BEFOREyouAFTER"),
        (
            "BEFORE${!REPLACE/me/you}AFTER",
            "BEFORE${REPLACE/me/you}AFTER",
        ),
        ("BEFORE${UNSET:-default}AFTER", "BEFOREdefaultAFTER"),
        (
            "BEFORE${!UNSET:-default}AFTER",
            "BEFORE${UNSET:-default}AFTER",
        ),
        // first occurrence only, `//` replaces all
        ("BEFORE${REPEAT/na/NA}AFTER", "BEFOREbaNAnaAFTER"),
        ("BEFORE${REPEAT//na/NA}AFTER", "BEFOREbaNANAAFTER"),
        // escaped slashes and backslashes in the replacement
        ("BEFORE${REPLACE/me/a\\/b}AFTER", "BEFOREa/bAFTER"),
    ];
    for (input, expected) in cases {
        assert_eq!(
            context.envsubst(input).unwrap(),
            expected,
            "input {input:?}"
        );
    }

    // malformed substitutions error
    assert!(context.envsubst("BEFORE${}AFTER").is_err());
    assert!(context.envsubst("BEFORE${REPLACE").is_err());
}
