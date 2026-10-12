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
