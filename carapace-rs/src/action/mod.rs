//! The Action API: lazy completion descriptions and their materialized form.

mod batch;
mod constructors;
mod invoked;
mod modifiers;

use std::sync::Arc;

use crate::context::Context;
use crate::meta::Meta;
use crate::value::RawValues;

pub use batch::batch;
pub use invoked::{InvokedAction, merge_all};

/// Callback producing an [`Action`] at invocation time.
pub type ActionCallback = Arc<dyn Fn(&Context) -> Action + Send + Sync>;

/// A lazy description of completions.
///
/// Actions are constructed at definition time and do no work until
/// [`Action::invoke`] materializes them into an [`InvokedAction`].
#[derive(Clone, Default)]
pub struct Action {
    pub(crate) meta: Meta,
    kind: ActionKind,
}

#[derive(Clone, Default)]
enum ActionKind {
    #[default]
    Empty,
    /// Pre-computed values; no callback.
    Raw(RawValues),
    /// Lazy callback producing another action.
    Callback(ActionCallback),
}

impl std::fmt::Debug for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Action")
            .field("meta", &self.meta)
            .field(
                "kind",
                &match &self.kind {
                    ActionKind::Empty => "empty",
                    ActionKind::Raw(values) => return write!(f, "Raw({} values)", values.len()),
                    ActionKind::Callback(_) => "callback",
                },
            )
            .finish()
    }
}

impl Action {
    /// Create an action from a callback.
    pub fn callback<F>(f: F) -> Self
    where
        F: Fn(&Context) -> Action + Send + Sync + 'static,
    {
        Self {
            kind: ActionKind::Callback(Arc::new(f)),
            ..Default::default()
        }
    }

    /// Create an action producing static values.
    pub fn raw(values: RawValues) -> Self {
        Self {
            kind: ActionKind::Raw(values),
            ..Default::default()
        }
    }

    /// Whether this action carries a callback.
    pub fn is_callback(&self) -> bool {
        matches!(self.kind, ActionKind::Callback(_))
    }

    /// Materialize the action: run the callback chain and collect values.
    pub fn invoke(&self, context: &Context) -> InvokedAction {
        let mut meta = self.meta.clone();
        let values = match &self.kind {
            ActionKind::Empty => RawValues::new(),
            ActionKind::Raw(values) => values.clone(),
            ActionKind::Callback(callback) => {
                let inner = callback(context);
                let invoked = inner.invoke(context);
                meta.merge(&invoked.meta);
                invoked.values
            }
        };
        InvokedAction { meta, values }
    }

    /// Internal helper used by modifiers: wrap a transform of the invoked form.
    pub(crate) fn map_invoked<F>(self, f: F) -> Self
    where
        F: Fn(&Context, InvokedAction) -> InvokedAction + Send + Sync + 'static,
    {
        Self::callback(move |c| f(c, self.invoke(c)).to_action())
    }
}

impl From<InvokedAction> for Action {
    fn from(invoked: InvokedAction) -> Self {
        Self {
            meta: invoked.meta,
            kind: ActionKind::Raw(invoked.values),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{Action, Context};

    #[test]
    fn values_invoke() {
        let invoked = Action::values(["a", "b", "c"]).invoke(&Context::default());
        let values: Vec<&str> = invoked.values.iter().map(|v| v.value.as_str()).collect();
        assert_eq!(values, vec!["a", "b", "c"]);
    }

    #[test]
    fn values_filter_empty() {
        // Go ActionValues skips empty strings (defaultActions.go:185)
        let invoked = Action::values(["a", "", "b", ""]).invoke(&Context::default());
        let values: Vec<&str> = invoked.values.iter().map(|v| v.value.as_str()).collect();
        assert_eq!(values, vec!["a", "b"]);
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
        let merged = crate::action::merge_all([a, b]);
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

    fn values(invoked: crate::InvokedAction) -> Vec<String> {
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
            Some(format!("file://{}", crate::uid::path_escape(&abs)).as_str())
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
            Some(format!("file://{}", crate::uid::path_escape(&abs)).as_str())
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
        assert_eq!(invoked.values[0].style, "");

        *cell.write().unwrap() = "blue".to_owned();
        let invoked = action.invoke(&Context::default());
        assert_eq!(invoked.values[0].style, "blue");
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
        let values: Vec<String> = invoked.values.iter().map(|v| v.value.clone()).collect();
        assert!(values.iter().any(|v| v.starts_with("/tmp/")), "{values:?}");

        // the candidates keep the root as their typed directory part
        let context = Context::default().value("/t");
        let invoked = Action::files(Vec::<String>::new()).invoke(&context);
        assert!(
            invoked.values.iter().any(|v| v.value == "/tmp/"),
            "{:?}",
            invoked.values
        );
    }

    #[test]
    fn files_suffix_is_case_sensitive() {
        // Go matches suffixes with strings.HasSuffix (internalActions.go:71)
        let base = std::env::temp_dir().join(format!("carapace-suffix-{}", std::process::id()));
        std::fs::create_dir_all(&base).unwrap();
        std::fs::write(base.join("file.TXT"), "").unwrap();
        std::fs::write(base.join("file.txt"), "").unwrap();

        let context = Context::default().dir(base.clone());
        let invoked = Action::files([".txt"]).invoke(&context);
        let values: Vec<&str> = invoked.values.iter().map(|v| v.value.as_str()).collect();
        assert!(values.contains(&"file.txt"), "{values:?}");
        assert!(!values.contains(&"file.TXT"), "{values:?}");

        std::fs::remove_dir_all(base).unwrap();
    }
}

#[cfg(test)]
mod split_tests {
    use std::sync::Arc;

    use crate::{Action, Command, Context, Flag, Positional};
    use carapace_rs_shlex::Format;

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

    fn split(value: &str, dir: Option<&std::path::Path>) -> crate::InvokedAction {
        let mut context = Context::default().value(value);
        if let Some(dir) = dir {
            context = context.dir(dir);
        }
        inner_action().split(Format::default()).invoke(&context)
    }

    /// Values as the shared dispatch would deliver them (sorted).
    fn values(invoked: &crate::InvokedAction) -> Vec<String> {
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
        // the files action contributes '/' to the nospace matcher (Go's
        // MultiParts("/")), merged with split's own '*'
        assert_eq!(invoked.meta.nospace, vec!['*', '/']);

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

    #[test]
    fn split_p_completes_files_for_redirect() {
        let dir = std::env::temp_dir().join(format!("carapace-splitp-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("file1.txt"), "").unwrap();

        let mut context = Context::default().value("pos1 > ");
        context.dir = dir.clone();
        let invoked = inner_action().split_p(Format::default()).invoke(&context);
        // the redirect target completes files
        assert_eq!(values(&invoked), vec!["pos1 > file1.txt "]);
        assert_eq!(invoked.meta.nospace, vec!['*', '/']);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn split_p_restarts_args_at_pipeline_delimiter() {
        // After the pipe the current pipeline restarts: args are empty, so the
        // first positional completes again (Split would pass the earlier
        // pipeline's words plus the delimiter as args).
        let invoked = split_p("pos1 | ");
        assert_eq!(
            values(&invoked),
            vec!["pos1 | pos1 ", "pos1 | positional1 "]
        );
    }

    fn split_p(value: &str) -> crate::InvokedAction {
        let context = Context::default().value(value);
        inner_action().split_p(Format::default()).invoke(&context)
    }
}

#[cfg(test)]
mod import_tests {
    use crate::{Action, Context};

    #[test]
    fn uid_modifier_sets_value_uids() {
        let action = Action::values(["a", "b/c"]).uid("cmd", "app", vec![("flag", "force".into())]);
        let invoked = action.invoke(&Context::default());
        assert_eq!(
            invoked.values[0].uid.as_deref(),
            Some("cmd://app/a?flag=force")
        );
        assert_eq!(
            invoked.values[1].uid.as_deref(),
            Some("cmd://app/b/c?flag=force")
        );
    }

    #[test]
    fn import_roundtrips_export() {
        let source = Action::values_described([("a", "first")]).invoke(&Context::default());
        let json = crate::export::Export::from_parts(&source.meta, &source.values).to_json();
        let action = Action::import(&json);
        let invoked = action.invoke(&Context::default());
        assert_eq!(invoked.values[0].value, "a");
        assert_eq!(invoked.values[0].description, "first");
    }

    #[test]
    fn import_invalid_json_becomes_message() {
        let invoked = Action::import("not json").invoke(&Context::default());
        assert!(!invoked.meta.messages.is_empty());
        assert!(invoked.values.is_empty());
    }

    #[test]
    fn export_sorts_by_value() {
        let invoked = Action::values(["b", "a"]).invoke(&Context::default());
        let json = crate::export::Export::from_parts(&invoked.meta, &invoked.values).to_json();
        assert!(json.find("\"a\"").unwrap() < json.find("\"b\"").unwrap());
    }

    #[test]
    fn usage_f_sets_usage_unless_empty() {
        let invoked = Action::values(["a"])
            .usage_f(|| "the usage".to_owned())
            .invoke(&Context::default());
        assert_eq!(invoked.meta.usage.as_deref(), Some("the usage"));

        // an empty string leaves the usage untouched (Go UsageF)
        let invoked = Action::values(["a"])
            .usage("original")
            .usage_f(String::new)
            .invoke(&Context::default());
        assert_eq!(invoked.meta.usage.as_deref(), Some("original"));
    }

    #[test]
    fn query_f_adds_query_and_c_value() {
        let action = Action::values(["a"]).query_f(|_| {
            Ok(crate::meta::Query {
                scheme: "cmd".to_owned(),
                host: "app".to_owned(),
                path: "entry".to_owned(),
                opts: vec!["flag".to_owned(), "force".to_owned()],
            })
        });
        let invoked = action.invoke(&Context::default().value("x"));
        assert_eq!(invoked.meta.queries.len(), 1);
        let opts = &invoked.meta.queries[0].opts;
        assert!(opts.contains(&"C_VALUE".to_owned()) && opts.contains(&"x".to_owned()));

        // error from the callback degrades to a message
        let invoked = Action::values(["a"])
            .query_f(|_| Err("oops".to_owned()))
            .invoke(&Context::default());
        assert!(!invoked.meta.messages.is_empty());
    }

    #[test]
    fn import_parses_go_carapace_export() {
        // shape emitted by Go carapace: flattened meta, matcher strings,
        // lowercase value keys
        let json = r#"{
            "version": "v0.7.0",
            "messages": ["oops"],
            "noprefix": "ab",
            "nospace": "/=",
            "usage": "the usage",
            "queries": ["cmd://app/sub?flag=force&C_VALUE=x"],
            "values": [
                {"value": "a", "display": "a", "description": "first"},
                {"value": "b/c", "display": "b/c", "style": "blue", "uid": "file://x/y"}
            ]
        }"#;
        let invoked = Action::import(json).invoke(&Context::default());
        assert_eq!(invoked.values[0].value, "a");
        assert_eq!(invoked.values[0].description, "first");
        assert_eq!(invoked.values[1].style, "blue");
        assert_eq!(invoked.meta.messages, vec!["oops"]);
        assert_eq!(invoked.meta.nospace, vec!['/', '=']);
        assert_eq!(invoked.meta.no_prefix, vec!['a', 'b']);
        assert_eq!(invoked.meta.usage.as_deref(), Some("the usage"));
        assert_eq!(invoked.meta.queries[0].scheme, "cmd");
        assert_eq!(invoked.meta.queries[0].host, "app");
        assert_eq!(invoked.meta.queries[0].path, "sub");
    }

    #[test]
    fn export_json_matches_go_shape() {
        let meta = crate::Meta::default().nospace(['/']).no_prefix(['a']);
        let invoked = Action::values_described([("a", "first")]).invoke(&Context::default());
        let json = crate::export::Export::from_parts(&meta, &invoked.values).to_json();
        assert!(!json.contains("\"Meta\""), "meta must be flattened: {json}");
        assert!(json.contains("\"values\":"), "{json}");
        assert!(json.contains("\"value\":\"a\""), "{json}");
        assert!(json.contains("\"nospace\":\"/\""), "{json}");
        assert!(json.contains("\"noprefix\":\"a\""), "{json}");
    }
}
