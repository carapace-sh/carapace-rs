use carapace_rs::{Action, Context, RawValue};

#[test]
fn values_invoke() {
    let invoked = Action::values(["a", "b", "c"]).invoke(&Context::default());
    let export = invoked.to_export();
    let values: Vec<&str> = export.values.iter().map(|v| v.value.as_str()).collect();
    assert_eq!(values, vec!["a", "b", "c"]);
}

#[test]
fn values_described() {
    let invoked =
        Action::values_described([("a", "first"), ("b", "second")]).invoke(&Context::default());
    assert_eq!(invoked.to_export().values[0].value, "a");
    assert_eq!(invoked.to_export().values[0].display, "a");
    assert_eq!(invoked.to_export().values[0].description, "first");
}

#[test]
fn filter_and_retain() {
    let action = Action::values(["A", "B", "C"]);
    assert_eq!(
        values(action.clone().filter(["B"]).invoke(&Context::default())),
        vec!["A", "C"]
    );
    assert_eq!(
        values(action.retain(["A", "C"]).invoke(&Context::default())),
        vec!["A", "C"]
    );
}

#[test]
fn filter_args() {
    let action = Action::values(["A", "B", "C"]);
    let context = Context::default().args(["A"]);
    assert_eq!(
        values(action.filter_args().invoke(&context)),
        vec!["B", "C"]
    );
}

#[test]
fn prefix_and_suffix() {
    let action = Action::values(["melon", "drop"]);
    let invoked = action
        .clone()
        .prefix("water")
        .invoke(&Context::default().value(""));
    assert_eq!(values(invoked), vec!["waterdrop", "watermelon"]);

    // prefix mismatch suppresses all values
    let invoked = action
        .prefix("water")
        .invoke(&Context::default().value("salt"));
    assert!(invoked.to_export().values.is_empty());
}

#[test]
fn prefix_filters_by_current_value() {
    let action = Action::values(["melon", "drop"]).prefix("water");
    // filtering by the current value happens later in the shell dispatch
    let invoked = action.invoke(&Context::default().value("waterme"));
    assert_eq!(values(invoked), vec!["waterdrop", "watermelon"]);
}

#[test]
fn suffix() {
    let invoked = Action::values(["apple"])
        .suffix("juice")
        .invoke(&Context::default());
    assert_eq!(values(invoked), vec!["applejuice"]);
}

#[test]
fn style_and_tag() {
    let invoked = Action::values(["a", "b"])
        .style("blue")
        .tag("letters")
        .invoke(&Context::default());
    assert!(invoked.to_export().values.iter().all(|v| v.style == "blue"));
    assert!(
        invoked
            .to_export()
            .values
            .iter()
            .all(|v| v.tag == "letters")
    );
}

#[test]
fn callback_is_lazy() {
    let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = calls.clone();
    let action = Action::callback(move |_| {
        counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Action::values(["x"])
    });
    assert!(action.is_callback());
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    action.invoke(&Context::default());
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[test]
fn message() {
    let invoked = Action::message("nothing here").invoke(&Context::default());
    assert_eq!(invoked.to_export().meta.messages, vec!["nothing here"]);
    assert!(invoked.to_export().values.is_empty());
}

#[test]
fn unless() {
    let action = Action::values(["a", "b"]);
    assert!(
        action
            .clone()
            .unless(true)
            .invoke(&Context::default())
            .to_export()
            .values
            .is_empty()
    );
    assert_eq!(
        values(action.unless(false).invoke(&Context::default())),
        vec!["a", "b"]
    );
}

#[test]
fn unless_f() {
    let action = Action::values(["a", "b"]).unless_f(|c| !c.args.is_empty());
    assert!(
        action
            .invoke(&Context::default().args(["arg"]))
            .to_export()
            .values
            .is_empty()
    );
    assert_eq!(values(action.invoke(&Context::default())), vec!["a", "b"]);
}

#[test]
fn shift() {
    let action = Action::callback(|c| Action::values(c.args.clone()));
    let context = Context::default().args(["one", "two", "three"]);
    let invoked = action.shift(1).invoke(&context);
    assert_eq!(values(invoked), vec!["three", "two"]);
}

#[test]
fn nospace() {
    let invoked = Action::values(["a/"])
        .nospace(['/'])
        .invoke(&Context::default());
    assert_eq!(invoked.to_export().meta.nospace, "/");
    let invoked = Action::values(["a"])
        .nospace([])
        .invoke(&Context::default());
    assert_eq!(invoked.to_export().meta.nospace, "*");
}

#[test]
fn multiparts_completes_last_segment() {
    let action = Action::multiparts("/", |c| {
        let values: &[&str] = match c.value.as_str() {
            "" => &["a", "b"],
            "a/" => &["x", "y"],
            _ => &["end"],
        };
        Action::values(values.iter().copied())
    });
    let invoked = action.invoke(&Context::default().value("a/"));
    assert_eq!(values(invoked), vec!["a/a", "a/b"]);

    let invoked = action.invoke(&Context::default().value("a/x"));
    assert_eq!(values(invoked), vec!["a/end"]);
}

#[test]
fn list() {
    let action = Action::values(["one", "two", "three"]).list(",");
    // filtering by the current value happens later in the shell dispatch
    let invoked = action.invoke(&Context::default().value("one,t"));
    assert_eq!(values(invoked), vec!["one,one", "one,three", "one,two"]);
}

#[test]
fn unique_list() {
    let action = Action::values(["one", "two", "three"]).unique_list(",");
    let invoked = action.invoke(&Context::default().value("one,t"));
    assert_eq!(values(invoked), vec!["one,three", "one,two"]);
    let invoked = action.invoke(&Context::default().value("one,three,t"));
    assert_eq!(values(invoked), vec!["one,three,two"]);
}

#[test]
fn chained_meta_merges_through_callbacks() {
    let invoked = Action::callback(|_| Action::message("inner").nospace([':']))
        .usage("outer usage")
        .invoke(&Context::default());
    assert_eq!(invoked.to_export().meta.messages, vec!["inner"]);
    assert_eq!(invoked.to_export().meta.usage, "outer usage");
    assert_eq!(invoked.to_export().meta.nospace, ":");
}

#[test]
fn invoked_merge_dedups() {
    let a = Action::values(["a", "b"]).invoke(&Context::default());
    let b = Action::values(["b", "c"]).invoke(&Context::default());
    let merged = carapace_rs::action::merge_all([a, b]);
    assert_eq!(values(merged), vec!["a", "b", "c"]);
}

#[test]
fn unique_dedups_by_value() {
    // Go `RawValues.Unique`: repeated values collapse (later entries win)
    // and the result is sorted by display
    let action = Action::values(["one", "two", "one", "three"]).unique();
    assert_eq!(
        values(action.invoke(&Context::default())),
        vec!["one", "three", "two"]
    );
}

#[test]
fn unique_keeps_distinct_uids() {
    // deduplication is by (value, uid): same value with different uid
    // survives, same value with no uid collapses
    let action = Action::callback(|_| {
        Action::raw(vec![
            RawValue {
                value: "a".to_owned(),
                uid: Some("u1".to_owned()),
                ..Default::default()
            },
            RawValue {
                value: "a".to_owned(),
                uid: Some("u2".to_owned()),
                ..Default::default()
            },
        ])
    });
    assert_eq!(
        values(action.unique().invoke(&Context::default())),
        vec!["a", "a"]
    );
}

#[test]
fn usage_sets_meta() {
    let invoked = Action::values(["a"])
        .usage("explicit usage")
        .invoke(&Context::default());
    assert_eq!(invoked.to_export().meta.usage, "explicit usage");
}

#[test]
fn chdir_changes_context_dir() {
    // the resolved absolute directory is what the action sees, and the
    // target must exist (Go Chdir stats the directory)
    let base = std::env::temp_dir().join(format!("carapace-chdir-abs-{}", std::process::id()));
    std::fs::create_dir_all(base.join("subdir")).unwrap();

    let action = Action::callback(|c| Action::values([c.dir.display().to_string()]));
    let context = Context::default().dir(base.clone());
    let invoked = action.chdir("subdir").invoke(&context);
    assert_eq!(
        values(invoked),
        vec![base.join("subdir").display().to_string()]
    );

    std::fs::remove_dir_all(base).unwrap();
}

#[test]
fn chdir_scopes_files_completion() {
    // Go TestChdir: files complete relative to the chdir'ed directory
    let base = std::env::temp_dir().join(format!("carapace-chdir-{}", std::process::id()));
    std::fs::create_dir_all(base.join("subdir")).unwrap();
    std::fs::write(base.join("subdir/file1.txt"), "").unwrap();

    let action = Action::files(Vec::<String>::new()).chdir("subdir");
    let context = Context::default().dir(base.clone());
    assert_eq!(values(action.invoke(&context)), vec!["file1.txt"]);

    // a non-directory target degrades to a message
    std::fs::write(base.join("notadir"), "").unwrap();
    let invoked = Action::files(Vec::<String>::new())
        .chdir("notadir")
        .invoke(&context);
    assert!(invoked.to_export().values.is_empty());
    assert!(!invoked.to_export().meta.messages.is_empty());

    std::fs::remove_dir_all(base).unwrap();
}

#[test]
fn files_completion_marks_directories_nospace() {
    // Go wraps ActionFiles/ActionDirectories in MultiParts("/"), which adds
    // "/" to the nospace matcher; shells consulting only the meta (nushell,
    // powershell, xonsh) rely on it to suppress the trailing space
    let base = std::env::temp_dir().join(format!("carapace-nospace-{}", std::process::id()));
    std::fs::create_dir_all(base.join("subdir")).unwrap();
    std::fs::write(base.join("file1.txt"), "").unwrap();

    let context = Context::default().dir(base.clone()).value("");
    let invoked = Action::files(Vec::<String>::new()).invoke(&context);
    assert!(invoked.to_export().meta.nospace.contains('/'));
    let export = invoked.to_export();
    let dir = export
        .values
        .iter()
        .find(|value| value.value == "subdir/")
        .unwrap();
    assert_eq!(dir.nospace, Some(true));
    let file = export
        .values
        .iter()
        .find(|value| value.value == "file1.txt")
        .unwrap();
    assert_ne!(file.nospace, Some(true));

    std::fs::remove_dir_all(base).unwrap();
}

#[test]
fn multiparts_prefixes_deep_segments() {
    // Go TestMultiPartsP: the full prefix is re-attached at every level
    let action = Action::multiparts("/", |c| match (c.parts.len(), c.value.as_str()) {
        (0, "") => Action::values(["keys"]),
        (1, "") => Action::values(["key1", "key2"]),
        (2, "") => Action::values(["val1", "val2"]),
        _ => Action::values(Vec::<String>::new()),
    });
    let invoked = action.invoke(&Context::default().value("keys/key1/"));
    assert_eq!(values(invoked), vec!["keys/key1/val1", "keys/key1/val2"]);
}

#[test]
fn invoked_modifiers_preserve_messages() {
    // Go TestSkipCache: a merged message survives no-op modifiers and
    // conversion back into a lazy action
    let action = Action::callback(|c| {
        Action::values(Vec::<String>::new())
            .invoke(c)
            .merge([Action::message("skipcache").invoke(c)])
            .filter(std::iter::empty::<String>())
            .prefix("")
            .suffix("")
            .to_action()
    });
    // the message only materializes on invocation (lazy two-phase API)
    assert!(action.is_callback());
    assert_eq!(
        action.invoke(&Context::default()).to_export().meta.messages,
        vec!["skipcache"]
    );
}

fn values(invoked: carapace_rs::InvokedAction) -> Vec<String> {
    // the export wire format sorts values (Go MarshalJSON); assertions
    // compare in that order
    let mut values: Vec<String> = invoked
        .to_export()
        .values
        .iter()
        .map(|v| v.value.clone())
        .collect();
    values.sort();
    values
}

/// Parse the `key=value` pairs of an export query URL string.
fn query_opts(query: &str) -> Vec<(String, String)> {
    let (_, query) = query.split_once('?').unwrap_or((query, ""));
    query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            pair.split_once('=')
                .map_or((pair.to_owned(), String::new()), |(key, value)| {
                    (key.to_owned(), value.to_owned())
                })
        })
        .collect()
}

#[test]
fn files_carry_file_uids_and_query() {
    // Go ActionFiles attaches file:// uids and a "file" query annotation
    let base = std::env::temp_dir().join(format!("carapace-fileuid-{}", std::process::id()));
    std::fs::create_dir_all(base.join("subdir")).unwrap();
    std::fs::write(base.join("file1.txt"), "").unwrap();

    let context = Context::default().dir(base.clone()).value("f");
    let invoked = Action::files(Vec::<String>::new()).invoke(&context);
    let export = invoked.to_export();
    let value = export
        .values
        .iter()
        .find(|value| value.value == "file1.txt")
        .unwrap();
    let abs = context.abs("file1.txt").unwrap();
    assert_eq!(
        value.uid,
        format!("file://{}", carapace_rs::uid::path_escape(&abs))
    );

    // the query annotation records scheme/host, the context directory and
    // the completed value (Go's C_DIR/C_VALUE opts); in the export wire
    // format queries are uid-style URL strings with `key=value` pairs
    let query = &export.meta.queries[0];
    assert!(query.starts_with("file://files?"), "{query}");
    let opts = query_opts(query);
    let c_dir = context.abs(&base.to_string_lossy()).unwrap();
    assert_eq!(
        opts.iter().find(|(key, _)| key == "C_DIR").unwrap().1,
        carapace_rs::uid::query_escape(&c_dir)
    );
    assert!(
        opts.iter()
            .any(|(key, value)| key == "C_VALUE" && value == "f")
    );

    // directories use the "directories" host without a suffix opt
    let context = Context::default().dir(base.clone()).value("s");
    let invoked = Action::directories().invoke(&context);
    let export = invoked.to_export();
    let query = &export.meta.queries[0];
    assert!(query.starts_with("file://directories"), "{query}");
    assert!(!query.contains("suffix"));
    let dir = export
        .values
        .iter()
        .find(|value| value.value == "subdir/")
        .unwrap();
    let abs = context.abs("subdir/").unwrap();
    assert_eq!(
        dir.uid,
        format!("file://{}", carapace_rs::uid::path_escape(&abs))
    );

    std::fs::remove_dir_all(base).unwrap();
}

#[test]
fn style_r_reads_the_cell_at_invocation() {
    // Go StyleR: the style resolves at invoke time, so runtime changes
    // apply to later completions
    let cell = std::sync::Arc::new(std::sync::RwLock::new(String::new()));
    let action = Action::values(["one"]).style_r(cell.clone());

    let invoked = action.invoke(&Context::default());
    assert_eq!(invoked.to_export().values[0].style, "");

    *cell.write().unwrap() = "blue".to_owned();
    let invoked = action.invoke(&Context::default());
    assert_eq!(invoked.to_export().values[0].style, "blue");
}

#[test]
#[cfg(unix)]
fn files_completion_resolves_the_filesystem_root() {
    // `/tmp` (no trailing slash): the empty directory part before the
    // leading slash is the filesystem root, not the context directory.
    // POSIX-shaped (drive-relative roots on Windows complete through a
    // different path), so it only runs where the root is unambiguous.
    let context = Context::default().value("/tmp");
    let invoked = Action::files(Vec::<String>::new()).invoke(&context);
    let values: Vec<String> = invoked
        .to_export()
        .values
        .iter()
        .map(|v| v.value.clone())
        .collect();
    assert!(values.iter().any(|v| v.starts_with("/tmp/")), "{values:?}");

    // the candidates keep the root as their typed directory part
    let context = Context::default().value("/t");
    let invoked = Action::files(Vec::<String>::new()).invoke(&context);
    assert!(
        invoked
            .to_export()
            .values
            .iter()
            .any(|v| v.value == "/tmp/"),
        "{:?}",
        invoked.to_export().values
    );
}
