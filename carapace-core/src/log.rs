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

/// `YYYY/MM/DD HH:MM:SS.microseconds` in UTC (Go's `log.Lmicroseconds`).
fn timestamp() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let seconds = now.as_secs();
    let micros = now.subsec_micros();

    let days = seconds / 86_400;
    let seconds_of_day = seconds % 86_400;
    let (year, month, day) = civil_from_days(days as i64);
    format!(
        "{:04}/{:02}/{:02} {:02}:{:02}:{:02}.{:06}",
        year,
        month,
        day,
        seconds_of_day / 3600,
        (seconds_of_day % 3600) / 60,
        seconds_of_day % 60,
        micros
    )
}

/// Days since the unix epoch to the civil date (Howard Hinnant's algorithm).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if month <= 2 { year + 1 } else { year }, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_dates() {
        // day 0 = 1970-01-01
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(59), (1970, 3, 1)); // non-leap year
        assert_eq!(civil_from_days(365), (1971, 1, 1));
        assert_eq!(civil_from_days(19_723), (2024, 1, 1));
    }
}
