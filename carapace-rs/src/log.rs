//! Debug logging to `/tmp/carapace/{executable}.log` when `CARAPACE_LOG`
//! is set (port of Go `internal/log`).
//!
//! Lines carry a microsecond timestamp and the detected shell as prefix
//! (Go's `log.Lmsgprefix` with `ps.DetermineShell()`).

use std::fs::File;
use std::io::Write;
use std::sync::{Mutex, OnceLock};

static WRITER: OnceLock<Option<Mutex<File>>> = OnceLock::new();

/// Append a message to the log file; a no-op unless `CARAPACE_LOG` is set.
pub fn log(message: &str) {
    let writer = WRITER.get_or_init(|| {
        if !crate::config::log_enabled() {
            return None;
        }
        let tmpdir = std::env::temp_dir().join("carapace");
        if std::fs::create_dir_all(&tmpdir).is_err() {
            return None;
        }
        let executable = std::env::current_exe()
            .ok()
            .and_then(|path| {
                path.file_name()
                    .map(|name| name.to_string_lossy().into_owned())
            })
            .unwrap_or_else(|| "carapace".to_owned());
        let file = File::options()
            .append(true)
            .create(true)
            .open(tmpdir.join(format!("{executable}.log")))
            .ok()?;
        Some(Mutex::new(file))
    });
    let Some(writer) = writer else {
        return;
    };
    if let Ok(mut file) = writer.lock() {
        let shell = crate::shell::Shell::detect()
            .map(|shell| shell.name())
            .unwrap_or_default();
        let _ = writeln!(file, "{} {shell} {message}", timestamp());
    }
}

/// `YYYY/MM/DD HH:MM:SS.microseconds` in the local timezone
/// (Go's `log.Lmicroseconds`).
fn timestamp() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    let seconds = now.as_secs() as libc::time_t;
    unsafe { libc::localtime_r(&seconds, &mut tm) };
    format!(
        "{:04}/{:02}/{:02} {:02}:{:02}:{:02}.{:06}",
        tm.tm_year + 1900,
        tm.tm_mon + 1,
        tm.tm_mday,
        tm.tm_hour,
        tm.tm_min,
        tm.tm_sec,
        now.subsec_micros()
    )
}
