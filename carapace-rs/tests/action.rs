use carapace_rs::{Action, Context};

#[test]
fn values_invoke() {
    let invoked = Action::values(["a", "b", "c"]).invoke(&Context::default());
    let values: Vec<&str> = invoked.values.iter().map(|v| v.value.as_str()).collect();
    assert_eq!(values, vec!["a", "b", "c"]);
}

#[test]
fn values_described() {
    let invoked =
        Action::values_described([("a", "first"), ("b", "second")]).invoke(&Context::default());
    assert_eq!(invoked.values[0].value, "a");
    assert_eq!(invoked.values[0].display, "a");
    assert_eq!(invoked.values[0].description, "first");
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
    assert_eq!(values(invoked), vec!["watermelon", "waterdrop"]);

    // prefix mismatch suppresses all values
    let invoked = action
        .prefix("water")
        .invoke(&Context::default().value("salt"));
    assert!(invoked.values.is_empty());
}

#[test]
fn prefix_filters_by_current_value() {
    let action = Action::values(["melon", "drop"]).prefix("water");
    // filtering by the current value happens later in the shell dispatch
    let invoked = action.invoke(&Context::default().value("waterme"));
    assert_eq!(values(invoked), vec!["watermelon", "waterdrop"]);
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
    assert!(invoked.values.iter().all(|v| v.style == "blue"));
    assert!(invoked.values.iter().all(|v| v.tag == "letters"));
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
    assert_eq!(invoked.meta.messages, vec!["nothing here"]);
    assert!(invoked.values.is_empty());
}

#[test]
fn unless() {
    let action = Action::values(["a", "b"]);
    assert!(
        action
            .clone()
            .unless(true)
            .invoke(&Context::default())
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
    assert_eq!(values(invoked), vec!["two", "three"]);
}

#[test]
fn nospace() {
    let invoked = Action::values(["a/"])
        .nospace(['/'])
        .invoke(&Context::default());
    assert_eq!(invoked.meta.nospace, vec!['/']);
    let invoked = Action::values(["a"])
        .nospace([])
        .invoke(&Context::default());
    assert_eq!(invoked.meta.nospace, vec!['*']);
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
    assert_eq!(values(invoked), vec!["one,one", "one,two", "one,three"]);
}

#[test]
fn unique_list() {
    let action = Action::values(["one", "two", "three"]).unique_list(",");
    let invoked = action.invoke(&Context::default().value("one,t"));
    assert_eq!(values(invoked), vec!["one,two", "one,three"]);
    let invoked = action.invoke(&Context::default().value("one,three,t"));
    assert_eq!(values(invoked), vec!["one,three,two"]);
}

#[test]
fn chained_meta_merges_through_callbacks() {
    let invoked = Action::callback(|_| Action::message("inner").nospace([':']))
        .usage("outer usage")
        .invoke(&Context::default());
    assert_eq!(invoked.meta.messages, vec!["inner"]);
    assert_eq!(invoked.meta.usage.as_deref(), Some("outer usage"));
    assert_eq!(invoked.meta.nospace, vec![':']);
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
    // Go TestUnique: repeated values collapse
    let action = Action::values(["one", "two", "one", "three"]).unique();
    assert_eq!(
        values(action.invoke(&Context::default())),
        vec!["one", "two", "three"]
    );
}

#[test]
fn unique_keeps_distinct_uids() {
    // deduplication is by (value, uid): same value with different uid
    // survives, same value with no uid collapses
    let action = Action::callback(|_| {
        let mut invoked = Action::values(["a", "a"]).invoke(&Context::default());
        invoked.values[0].uid = Some("u1".to_owned());
        invoked.values[1].uid = Some("u2".to_owned());
        invoked.to_action()
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
    assert_eq!(invoked.meta.usage.as_deref(), Some("explicit usage"));
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
    assert!(invoked.values.is_empty());
    assert!(!invoked.meta.messages.is_empty());

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
    assert!(invoked.meta.nospace.contains(&'/'));
    let dir = invoked
        .values
        .iter()
        .find(|value| value.value == "subdir/")
        .unwrap();
    assert_eq!(dir.nospace, Some(true));
    let file = invoked
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
        action.invoke(&Context::default()).meta.messages,
        vec!["skipcache"]
    );
}

fn values(invoked: carapace_rs::InvokedAction) -> Vec<String> {
    invoked.values.into_iter().map(|v| v.value).collect()
}

#[test]
fn files_carry_file_uids_and_query() {
    // Go ActionFiles attaches file:// uids and a "file" query annotation
    let base = std::env::temp_dir().join(format!("carapace-fileuid-{}", std::process::id()));
    std::fs::create_dir_all(base.join("subdir")).unwrap();
    std::fs::write(base.join("file1.txt"), "").unwrap();

    let context = Context::default().dir(base.clone()).value("f");
    let invoked = Action::files(Vec::<String>::new()).invoke(&context);
    let value = invoked
        .values
        .iter()
        .find(|value| value.value == "file1.txt")
        .unwrap();
    let abs = context.abs("file1.txt").unwrap();
    assert_eq!(
        value.uid.as_deref(),
        Some(format!("file://{}", carapace_rs::uid::path_escape(&abs)).as_str())
    );

    // the query annotation records scheme/host, the context directory and
    // the completed value (Go's C_DIR/C_VALUE opts)
    let query = &invoked.meta.queries[0];
    assert_eq!(query.scheme, "file");
    assert_eq!(query.host, "files");
    let c_dir = context.abs(&base.to_string_lossy()).unwrap();
    let c_dir_index = query.opts.iter().position(|opt| opt == "C_DIR").unwrap();
    assert_eq!(query.opts[c_dir_index + 1], c_dir);
    assert!(query.opts.contains(&"C_VALUE".to_string()));
    assert!(query.opts.contains(&"f".to_string()));

    // directories use the "directories" host without a suffix opt
    let context = Context::default().dir(base.clone()).value("s");
    let invoked = Action::directories().invoke(&context);
    let query = &invoked.meta.queries[0];
    assert_eq!(query.host, "directories");
    assert!(!query.opts.iter().any(|opt| opt == "suffix"));
    let dir = invoked
        .values
        .iter()
        .find(|value| value.value == "subdir/")
        .unwrap();
    let abs = context.abs("subdir/").unwrap();
    assert_eq!(
        dir.uid.as_deref(),
        Some(format!("file://{}", carapace_rs::uid::path_escape(&abs)).as_str())
    );

    std::fs::remove_dir_all(base).unwrap();
}
