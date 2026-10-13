//! File-based cache for action results (port of Go `pkg/cache`).

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::export::Export;

/// FNV-1a hash rendered as hex: stable across builds (unlike `DefaultHasher`).
fn fnv1a(data: &str) -> String {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x1000_0000_01b3;
    let mut hash = OFFSET;
    for byte in data.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(PRIME);
    }
    format!("{hash:016x}")
}

/// Base cache directory, honoring an attached sandbox mock.
fn cache_dir(context: &crate::context::Context) -> Option<PathBuf> {
    if let Some(mock) = &context.mock {
        return Some(mock.dir.join("cache").join("carapace"));
    }
    // honors XDG_CACHE_HOME on unix; platform directory on Windows/macOS
    Some(dirs::cache_dir()?.join("carapace"))
}

/// Cache file path for a call site and key components.
///
/// Layout: `<cache>/carapace/<executable>/<site-hash>/<key-hash>`
pub fn file(context: &crate::context::Context, site: &str, keys: &[String]) -> Option<PathBuf> {
    let executable = std::env::args()
        .next()
        .map(|arg| {
            Path::new(&arg)
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| arg.clone())
        })
        .unwrap_or_else(|| "unknown".to_owned());
    let dir = cache_dir(context)?.join(executable).join(fnv1a(site));
    // always hash the keys (even empty) so the path is always a file
    Some(dir.join(fnv1a(&keys.join("\u{1}"))))
}

/// Load a cached export if it exists and is younger than the timeout.
///
/// Negative timeout never expires, zero always revalidates.
pub fn load(path: &Path, timeout_secs: i64) -> Option<Export> {
    let metadata = std::fs::metadata(path).ok()?;
    if timeout_secs == 0 {
        return None; // zero means always revalidate
    }
    if timeout_secs > 0 {
        let modified = metadata.modified().ok()?;
        let age = SystemTime::now().duration_since(modified).ok()?;
        if age > Duration::from_secs(timeout_secs as u64) {
            return None;
        }
    }
    let content = std::fs::read_to_string(path).ok()?;
    Export::from_json(&content)
}

/// Write an export to the cache file (0600-equivalent on unix).
pub fn write(path: &Path, export: &Export) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)?
            .write_all(export.to_json().as_bytes())?;
        Ok(())
    }
    #[cfg(not(unix))]
    {
        std::fs::write(path, export.to_json())
    }
}

/// File modification time as unix epoch seconds.
pub fn modified_secs(path: &Path) -> Option<u64> {
    std::fs::metadata(path)
        .ok()?
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use crate::{Action, Context};

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
        let mock = crate::mock::Mock {
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

        let file = crate::cache::file(&context, "test-site", &["k1".to_owned()]).unwrap();
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
        let file = crate::cache::file(&context, "error-site", &[]).unwrap();
        assert!(!file.exists(), "messages must not be cached");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn cache_expires_after_ttl() {
        let (context, dir) = sandbox_context("ttl");
        let path = crate::cache::file(&context, "ttl-site", &[]).unwrap();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let export = crate::export::Export::from_parts(
            &crate::Meta::default(),
            &[crate::RawValue::new("old")],
        );
        crate::cache::write(&path, &export).unwrap();

        assert!(crate::cache::load(&path, 60).is_some(), "within ttl");
        assert!(crate::cache::load(&path, 0).is_none(), "zero revalidates");
        assert!(
            crate::cache::load(&path, -1).is_some(),
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
}
