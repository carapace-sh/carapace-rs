use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use carapace_rs::{Action, Context};

/// A context whose mock directs the cache into a unique, cleaned-up temp dir.
fn sandbox_context(name: &str) -> (Context, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!(
        "carapace-cache-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let mock = carapace_rs::mock::Mock {
        dir: dir.clone(),
        replies: Default::default(),
    };
    (Context::default().mock(mock), dir)
}

#[test]
fn cache_stores_and_reuses_results() {
    let (context, dir) = sandbox_context("store");

    let calls = Arc::new(AtomicUsize::new(0));
    let calls_clone = calls.clone();

    let action = Action::callback(move |_| {
        calls_clone.fetch_add(1, Ordering::SeqCst);
        Action::values(["cached-value"])
    })
    .cache("test-site", 60, vec!["k1".into()]);

    let first = action.invoke(&context);
    let second = action.invoke(&context);

    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "second invocation must hit the cache"
    );
    assert_eq!(first.values[0].value, "cached-value");
    assert_eq!(second.values[0].value, "cached-value");

    let file = carapace_rs::cache::file(&context, "test-site", &["k1".to_owned()]).unwrap();
    assert!(file.exists());

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn cache_keys_distinguish_entries() {
    let (context, dir) = sandbox_context("keys");
    let action = Action::values(["static"]); // raw actions skip caching
    let first = action.clone().invoke(&context.clone().env("X", "1"));
    let second = action.invoke(&context.env("X", "2"));
    assert_eq!(first.values, second.values);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn cache_skips_error_states() {
    let (context, dir) = sandbox_context("errors");
    let action = Action::message("error state").cache("error-site", 60, vec![]);
    let invoked = action.invoke(&context);
    assert_eq!(invoked.meta.messages, vec!["error state"]);
    let file = carapace_rs::cache::file(&context, "error-site", &[]).unwrap();
    assert!(!file.exists(), "messages must not be cached");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn cache_expires_after_ttl() {
    let (context, dir) = sandbox_context("ttl");
    let path = carapace_rs::cache::file(&context, "ttl-site", &[]).unwrap();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let export = carapace_rs::export::Export::from_parts(
        &carapace_rs::Meta::default(),
        &[carapace_rs::RawValue::new("old")],
    );
    carapace_rs::cache::write(&path, &export).unwrap();

    assert!(carapace_rs::cache::load(&path, 60).is_some(), "within ttl");
    assert!(
        carapace_rs::cache::load(&path, 0).is_none(),
        "zero revalidates"
    );
    assert!(
        carapace_rs::cache::load(&path, -1).is_some(),
        "negative never expires"
    );
    std::fs::remove_dir_all(&dir).ok();
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
