use carapace_rs::Context;
use carapace_rs::condition;

#[test]
fn completing_path_detects_path_prefixes() {
    let mut truthy = vec![".hidden", "./relative", "/absolute", "~home"];
    if cfg!(windows) {
        truthy.push("C:\\windows");
    }
    for value in truthy {
        assert!(
            condition::completing_path(&Context::default().value(value)),
            "expected path-like: {value}"
        );
    }
    let falsy = ["", "value", "value/x"];
    for value in falsy {
        assert!(
            !condition::completing_path(&Context::default().value(value)),
            "expected not path-like: {value}"
        );
    }
}

#[test]
fn completing_path_s_includes_separators() {
    assert!(condition::completing_path_s(
        &Context::default().value("dir/file")
    ));
    assert!(!condition::completing_path_s(
        &Context::default().value("file")
    ));
}
