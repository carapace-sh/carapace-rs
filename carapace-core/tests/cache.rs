use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use carapace_core::{Action, Context};

static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Unique, cleaned-up sandbox per test (env must not leak across tests).
struct Sandbox(std::path::PathBuf);

impl Sandbox {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!(
            "carapace-cache-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        unsafe { std::env::set_var("CARAPACE_SANDBOX", &dir) };
        Self(dir)
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        unsafe { std::env::remove_var("CARAPACE_SANDBOX") };
        std::fs::remove_dir_all(&self.0).ok();
    }
}

#[test]
fn cache_stores_and_reuses_results() {
    let _env = ENV_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let _sandbox = Sandbox::new();

    let calls = Arc::new(AtomicUsize::new(0));
    let calls_clone = calls.clone();

    let action = Action::callback(move |_| {
        calls_clone.fetch_add(1, Ordering::SeqCst);
        Action::values(["cached-value"])
    })
    .cache("test-site", 60, vec!["k1".into()]);

    let context = Context::default();
    let first = action.invoke(&context);
    let second = action.invoke(&context);

    assert_eq!(calls.load(Ordering::SeqCst), 1, "second invocation must hit the cache");
    assert_eq!(first.values[0].value, "cached-value");
    assert_eq!(second.values[0].value, "cached-value");

    let file = carapace_core::cache::file("test-site", &["k1".to_owned()]).unwrap();
    assert!(file.exists());
}

#[test]
fn cache_keys_distinguish_entries() {
    let _env = ENV_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let _sandbox = Sandbox::new();
    let action = Action::values(["static"]); // raw actions skip caching
    let first = action
        .clone()
        .invoke(&Context::default().env("X", "1"));
    let second = action.invoke(&Context::default().env("X", "2"));
    assert_eq!(first.values, second.values);
}

#[test]
fn cache_skips_error_states() {
    let _env = ENV_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let _sandbox = Sandbox::new();
    let action = Action::message("error state").cache("error-site", 60, vec![]);
    let invoked = action.invoke(&Context::default());
    assert_eq!(invoked.meta.messages, vec!["error state"]);
    let file = carapace_core::cache::file("error-site", &[]).unwrap();
    assert!(!file.exists(), "messages must not be cached");
}

#[test]
fn cache_expires_after_ttl() {
    let _env = ENV_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let _sandbox = Sandbox::new();
    let path = carapace_core::cache::file("ttl-site", &[]).unwrap();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let export = carapace_core::export::Export::from_parts(
        &carapace_core::Meta::default(),
        &vec![carapace_core::RawValue::new("old")],
    );
    carapace_core::cache::write(&path, &export).unwrap();

    assert!(carapace_core::cache::load(&path, 60).is_some(), "within ttl");
    assert!(carapace_core::cache::load(&path, 0).is_none(), "zero revalidates");
    assert!(carapace_core::cache::load(&path, -1).is_some(), "negative never expires");
}

#[test]
fn timeout_falls_back_after_deadline() {
    let slow = Action::callback(|_| {
        std::thread::sleep(std::time::Duration::from_millis(500));
        Action::values(["slow"])
    });
    let fallback = Action::values(["fast"]);
    let action = slow.timeout(0, fallback);
    let invoked = action.invoke(&Context::default());
    assert_eq!(invoked.values[0].value, "fast");
}

#[test]
fn timeout_passes_through_when_fast_enough() {
    let fast = Action::values(["quick"]);
    let action = fast.timeout(5, Action::values(["fallback"]));
    let invoked = action.invoke(&Context::default());
    assert_eq!(invoked.values[0].value, "quick");
}
