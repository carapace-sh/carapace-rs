//! Sandbox test harness (port of Go `pkg/sandbox`).
//!
//! Go needs subprocess isolation because of its global storage map; the
//! Rust engine is in-process and side-effect free, so the harness runs
//! completions directly and compares their export JSON.

use carapace_rs::Action;
use std::collections::HashMap;
use std::path::PathBuf;

/// Isolated sandbox with a temp work directory and mocked command replies.
///
/// The mock travels on the completion [`Context`](carapace_rs::Context),
/// so sandboxes carry no process-global state and tests may run in parallel.
pub struct Sandbox {
    dir: PathBuf,
    env: HashMap<String, String>,
    replies: HashMap<String, String>,
}

impl Sandbox {
    /// Create a sandbox named after the test.
    pub fn new(test: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "carapace-sandbox_{test}_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(dir.join("cache")).unwrap();
        std::fs::create_dir_all(dir.join("work")).unwrap();
        Self {
            dir,
            env: HashMap::new(),
            replies: HashMap::new(),
        }
    }

    /// The sandbox work directory (completion context directory).
    pub fn work_dir(&self) -> std::path::PathBuf {
        self.dir.join("work")
    }

    /// Create files (path, content) relative to the work directory.
    pub fn files<const N: usize>(&self, entries: [(&str, &str); N]) -> &Self {
        for (path, content) in entries {
            if path.starts_with('/') || path.contains("..") {
                panic!("sandbox file path must be relative: {path}");
            }
            let target = self.dir.join("work").join(path);
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            std::fs::write(target, content).unwrap();
        }
        self
    }

    /// Register mocked output for a command invocation.
    pub fn reply(&mut self, program: &str, args: &[&str]) -> Reply<'_> {
        let args: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
        let key = carapace_rs::mock::Mock::key(program, &args);
        Reply { sandbox: self, key }
    }

    /// Set an environment variable for the completion context.
    pub fn env(&mut self, key: &str, value: &str) -> &mut Self {
        self.env.insert(key.to_owned(), value.to_owned());
        self
    }

    /// Remove the sandbox cache directory (between subtests).
    pub fn clear_cache(&self) {
        std::fs::remove_dir_all(self.dir.join("cache")).ok();
    }

    /// Run a completion: `args`'s last word is the value being completed.
    pub fn run<F>(&self, factory: F, args: &[&str]) -> Run
    where
        F: FnOnce() -> clap::Command,
    {
        let mut cmd = factory();
        let model = carapace_rs_clap::to_model(&mut cmd);
        let mut context = carapace_rs::Context::new(args.iter().map(|s| s.to_string()));
        context.dir = self.dir.join("work");
        for (key, value) in &self.env {
            context.env.insert(key.clone(), value.clone());
        }

        let mock = carapace_rs::mock::Mock {
            dir: self.dir.clone(),
            replies: self.replies.clone(),
        };

        // traverse takes the full word list (last word = completed value);
        // the seeded context applies the sandbox state to actions invoked
        // during traversal as well
        let mut words = context.args.clone();
        words.push(context.value.clone());
        context.mock = Some(mock);
        let completion = carapace_rs::traverse_seeded(&model, words, context.clone());
        let invoked = completion.action.invoke(&completion.context);

        // the actual side round-trips through the export shell's shared
        // dispatch (prefix filtering, flag merging, sorting, uid
        // stripping) exactly like Go's sandbox (`ActionExecute` runs
        // `_carapace export`)
        let json = carapace_rs::shell::Shell::Export.value_with(
            &context.value,
            &invoked.meta,
            &invoked.values,
            &carapace_rs::shell::bash::Context::default(),
            &completion.context.config,
        );
        let export = carapace_rs::export::Export::from_json(&json).unwrap_or_else(|| {
            carapace_rs::export::Export {
                version: String::new(),
                meta: Default::default(),
                values: Vec::new(),
            }
        });

        Run {
            export,
            args: args.iter().map(|arg| arg.to_string()).collect(),
            context: completion.context,
            value: context.value.clone(),
        }
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.dir).ok();
    }
}

/// Builder for a mocked command reply.
pub struct Reply<'s> {
    sandbox: &'s mut Sandbox,
    key: String,
}

impl<'s> Reply<'s> {
    pub fn with(self, output: &str) -> &'s mut Sandbox {
        self.sandbox.replies.insert(self.key, output.to_owned());
        self.sandbox
    }
}

/// A completion run awaiting assertions.
pub struct Run {
    export: carapace_rs::export::Export,
    args: Vec<String>,
    context: carapace_rs::Context,
    value: String,
}

impl Run {
    /// The export JSON of the run (for custom assertions).
    ///
    /// Like Go's sandbox comparison, values are filtered by the completed
    /// prefix, sorted and stripped of uids.
    pub fn output(&self) -> String {
        serde_json::to_string(&self.normalized_export()).unwrap()
    }

    /// The completion values of the run.
    pub fn values(&self) -> Vec<String> {
        self.export.values.iter().map(|v| v.value.clone()).collect()
    }

    /// Assert the run equals the expected action (compared as export JSON).
    ///
    /// Following Go's sandbox comparison (`invoke` with `strict=false`),
    /// both sides are filtered by the completed prefix, sorted by value and
    /// stripped of uids, and the expectation is invoked with the run's
    /// context (directory and environment).
    pub fn expect(&self, action: Action) {
        let invoked = action.invoke(&self.context);
        let expected = carapace_rs::export::Export::from_parts(&invoked.meta, &invoked.values);
        assert_eq!(
            serde_json::to_string(&self.normalized_export()).unwrap(),
            serde_json::to_string(&normalized(expected, &self.value)).unwrap(),
            "completion mismatch for run {:?}",
            self.args
        );
    }

    /// Assert the run's export JSON equals the given (already normalized)
    /// output of another run.
    pub fn expect_output(&self, output: &str) {
        assert_eq!(
            self.output(),
            output,
            "completion mismatch for run {:?}",
            self.args
        );
    }

    /// Assert the run does NOT equal the given action.
    pub fn expect_not(&self, action: Action) {
        let invoked = action.invoke(&self.context);
        let expected = carapace_rs::export::Export::from_parts(&invoked.meta, &invoked.values);
        assert_ne!(
            serde_json::to_string(&self.normalized_export()).unwrap(),
            serde_json::to_string(&normalized(expected, &self.value)).unwrap(),
            "completion unexpectedly matched for run {:?}",
            self.args
        );
    }

    fn normalized_export(&self) -> carapace_rs::export::Export {
        normalized(self.export.clone(), &self.value)
    }
}

/// Normalize an export for comparison (Go `sandbox.run.invoke` with
/// `strict=false`): filter values by the completed prefix, sort by value
/// and strip uids.
fn normalized(mut export: carapace_rs::export::Export, value: &str) -> carapace_rs::export::Export {
    export.values.retain(|v| v.value.starts_with(value));
    export.values.sort_by(|a, b| {
        a.value
            .cmp(&b.value)
            .then_with(|| a.display.cmp(&b.display))
    });
    for v in &mut export.values {
        v.uid = String::new();
        // the per-value nospace is a Rust-only wire extension; Go's
        // comparison has no equivalent field
        v.nospace = None;
    }
    export
}
