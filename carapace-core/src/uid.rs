//! URL-based unique identifiers (port of Go `pkg/uid`).
//!
//! UIDs are URL strings: `cmd://root/sub?flag=name` for commands and flags,
//! arbitrary schemes (e.g. `file://`) for values.

/// Escape a value for use as a UID path segment: `/` becomes `-` before
/// percent-encoding, since `/` is the path separator.
pub fn path_escape(value: &str) -> String {
    let replaced = value.replace('/', "-");
    let mut result = String::with_capacity(replaced.len());
    for byte in replaced.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                result.push(*byte as char)
            }
            _ => result.push_str(&format!("%{byte:02X}")),
        }
    }
    result
}

/// UID of a command: `cmd://<root>/<subcommand-path>`.
pub fn command(root: &str, path: &[&str]) -> String {
    let mut url = format!(
        "cmd://{}/{}",
        path_escape(root),
        path_escape(path.first().unwrap_or(&""))
    );
    for segment in path.iter().skip(1) {
        url.push('/');
        url.push_str(&path_escape(segment));
    }
    url
}

/// UID of a flag: `cmd://<root>/<subcommand-path>?flag=<name>`.
pub fn flag(root: &str, path: &[&str], flag_name: &str) -> String {
    format!("{}?flag={}", command(root, path), path_escape(flag_name))
}

/// Build a UID with alternating key-value opts (empty values skipped),
/// mirroring Go `uid.UidF`.
pub fn with_opts(scheme: &str, host: &str, value: &str, opts: &[(&str, &str)]) -> String {
    let mut url = format!("{scheme}://{}/{}", path_escape(host), path_escape(value));
    let mut query = Vec::new();
    for (key, opt_value) in opts {
        if !opt_value.is_empty() {
            query.push(format!("{}={}", path_escape(key), path_escape(opt_value)));
        }
    }
    if !query.is_empty() {
        url.push('?');
        url.push_str(&query.join("&"));
    }
    url
}
