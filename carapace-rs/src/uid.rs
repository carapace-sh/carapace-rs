//! URL-based unique identifiers (port of Go `pkg/uid`).
//!
//! UIDs are URL strings: `cmd://root/sub?flag=name` for commands and flags,
//! arbitrary schemes (e.g. `file://`) for values.

/// Escape a value for use as a UID path: segments are escaped
/// individually and slashes are preserved as separators (Go
/// `uid.PathEscape`).
pub fn path_escape(value: &str) -> String {
    value
        .split('/')
        .map(segment_escape)
        .collect::<Vec<_>>()
        .join("/")
}

/// Escape a single path segment (Go `url.PathEscape`): unreserved
/// characters and the reserved `: @ & = + $` stay, everything else
/// percent-encodes.
fn segment_escape(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    for byte in value.as_bytes() {
        match byte {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'~'
            | b':'
            | b'@'
            | b'&'
            | b'='
            | b'+'
            | b'$' => result.push(*byte as char),
            _ => result.push_str(&format!("%{byte:02X}")),
        }
    }
    result
}

/// Escape a query component (Go `url.QueryEscape` via `Values.Encode`):
/// spaces become `+`, unreserved characters stay.
pub fn query_escape(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    for byte in value.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                result.push(*byte as char)
            }
            b' ' => result.push('+'),
            _ => result.push_str(&format!("%{byte:02X}")),
        }
    }
    result
}

/// Decode a query component: `+` becomes a space before percent-decoding
/// (Go `url.QueryUnescape`).
pub fn query_unescape(value: &str) -> String {
    percent_unescape(&value.replace('+', " "))
}

/// UID of a command (Go `uid.Command`): `cmd://<root>/<subcommand-path>`;
/// an empty path renders without the trailing separator.
pub fn command(root: &str, path: &[impl AsRef<str>]) -> String {
    let mut url = format!("cmd://{}", path_escape(root));
    if !path.is_empty() {
        let joined = path
            .iter()
            .map(|segment| path_escape(segment.as_ref()))
            .collect::<Vec<_>>()
            .join("/");
        if !joined.is_empty() {
            url.push('/');
            url.push_str(&joined);
        }
    }
    url
}

/// UID of a flag (Go `uid.Flag`): `cmd://<root>/<subcommand-path>?flag=<name>`.
pub fn flag(root: &str, path: &[impl AsRef<str>], flag_name: &str) -> String {
    format!("{}?flag={}", command(root, path), query_escape(flag_name))
}

/// Build a UID with alternating key-value opts (empty values skipped),
/// mirroring Go `uid.UidF`; query pairs are sorted by key like
/// Go `url.Values.Encode`.
pub fn with_opts(scheme: &str, host: &str, value: &str, opts: &[(&str, &str)]) -> String {
    let mut url = format!("{scheme}://{}", path_escape(host));
    let escaped = path_escape(value);
    if !escaped.is_empty() {
        if !escaped.starts_with('/') {
            url.push('/');
        }
        url.push_str(&escaped);
    }
    let mut query: Vec<(&str, &str)> = opts
        .iter()
        .filter(|(_, opt_value)| !opt_value.is_empty())
        .copied()
        .collect();
    if !query.is_empty() {
        query.sort_by(|a, b| a.0.cmp(b.0));
        url.push('?');
        url.push_str(
            &query
                .iter()
                .map(|(key, opt_value)| {
                    format!("{}={}", query_escape(key), query_escape(opt_value))
                })
                .collect::<Vec<_>>()
                .join("&"),
        );
    }
    url
}

/// Inverse of [`path_escape`]: decode `%XX` percent-escapes.
pub fn path_unescape(value: &str) -> String {
    percent_unescape(value)
}

fn percent_unescape(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut result = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if let Some(byte) = bytes
                .get(index + 1..index + 3)
                .and_then(|hex| std::str::from_utf8(hex).ok())
                .and_then(|hex| u8::from_str_radix(hex, 16).ok())
            {
                result.push(byte);
                index += 3;
                continue;
            }
        }
        result.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&result).into_owned()
}
