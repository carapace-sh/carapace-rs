//! Sandbox test harness (port of Go `pkg/sandbox`).
//!
//! Go needs subprocess isolation because of its global storage map; the
//! Rust engine is in-process and side-effect free, so the harness runs
//! completions directly and compares their export JSON.

use carapace_core::Action;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::MutexGuard;

/// Isolated sandbox with a temp work directory and mocked command replies.
///
/// Holds a global lock for its lifetime: `CARAPACE_SANDBOX` is process
/// state, so sandbox tests must not run concurrently (Go's sandbox makes
/// the same tradeoff by not supporting `t.Parallel()`).
pub struct Sandbox {
    dir: PathBuf,
    env: HashMap<String, String>,
    replies: HashMap<String, String>,
    _guard: MutexGuard<'static, ()>,
}

impl Sandbox {
    /// Create a sandbox named after the test.
    pub fn new(test: &str) -> Self {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let guard = LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
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
            _guard: guard,
        }
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
        let key = carapace_core::mock::Mock::key(program, args);
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
        let model = carapace_clap::to_model(&mut cmd);
        let mut context = carapace_core::Context::new(args.iter().map(|s| s.to_string()));
        context.dir = self.dir.join("work");
        for (key, value) in &self.env {
            context.env.insert(key.clone(), value.clone());
        }

        let mock = carapace_core::mock::Mock {
            dir: self.dir.clone(),
            replies: self.replies.clone(),
        };
        let mock_json = serde_json::to_string(&mock).unwrap();
        unsafe { std::env::set_var("CARAPACE_SANDBOX", &mock_json) };

        // traverse takes the full word list (last word = completed value)
        let mut words = context.args.clone();
        words.push(context.value.clone());
        let mut completion = carapace_core::traverse(&model, words);
        completion.context.dir = context.dir;
        completion.context.env = context.env;
        let invoked = completion.action.invoke(&completion.context);
        unsafe { std::env::remove_var("CARAPACE_SANDBOX") };

        Run {
            export: carapace_core::export::Export::from_parts(&invoked.meta, &invoked.values),
            args: args.iter().map(|arg| arg.to_string()).collect(),
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
    export: carapace_core::export::Export,
    args: Vec<String>,
}

impl Run {
    /// The export JSON of the run (for custom assertions).
    pub fn output(&self) -> String {
        self.export.to_json()
    }

    /// The completion values of the run.
    pub fn values(&self) -> Vec<String> {
        self.export.values.iter().map(|v| v.value.clone()).collect()
    }

    /// Assert the run equals the expected action (compared as export JSON).
    pub fn expect(&self, action: Action) {
        let invoked = action.invoke(&carapace_core::Context::default());
        let expected = carapace_core::export::Export::from_parts(&invoked.meta, &invoked.values);
        assert_eq!(
            serde_json::to_string(&self.export).unwrap(),
            serde_json::to_string(&expected).unwrap(),
            "completion mismatch for run {:?}",
            self.args
        );
    }

    /// Assert the run does NOT equal the given action.
    pub fn expect_not(&self, action: Action) {
        let invoked = action.invoke(&carapace_core::Context::default());
        let expected = carapace_core::export::Export::from_parts(&invoked.meta, &invoked.values);
        assert_ne!(
            serde_json::to_string(&self.export).unwrap(),
            serde_json::to_string(&expected).unwrap(),
            "completion unexpectedly matched for run {:?}",
            self.args
        );
    }
}
