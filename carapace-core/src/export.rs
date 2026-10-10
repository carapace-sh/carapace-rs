//! The export JSON wire format (port of Go `internal/export`).
//!
//! Used for bridging, embedding and caching completions across process
//! boundaries. The shape matches Go structurally: the meta is flattened
//! into the top level (Go struct embedding) with lowercase keys, and the
//! nospace/noprefix matchers are sorted character strings with `*`
//! wildcard (Go `SuffixMatcher`/`PrefixMatcher` JSON forms).

use serde::{Deserialize, Serialize};

use crate::meta::{Meta, Query};
use crate::value::RawValue;

/// Serializable form of an invoked action.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Export {
    pub version: String,
    #[serde(flatten)]
    pub meta: ExportMeta,
    #[serde(default)]
    pub values: Vec<ExportValue>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ExportMeta {
    #[serde(default)]
    pub messages: Vec<String>,
    /// Sorted suffix characters, `*` matches all (Go `SuffixMatcher`).
    #[serde(default)]
    pub nospace: String,
    /// Sorted prefix characters, `*` matches all (Go `PrefixMatcher`).
    #[serde(default)]
    pub noprefix: String,
    #[serde(default)]
    pub usage: String,
    /// Query annotations as uid-style URL strings (experimental).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub queries: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ExportValue {
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub display: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub style: String,
    #[serde(default)]
    pub tag: String,
    #[serde(default)]
    pub uid: String,
    /// Per-value nospace (Rust extension; Go consumers ignore unknown keys,
    /// Go-written exports default to None). Keeps cache round-trips faithful
    /// for values like optarg shorthand flags.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nospace: Option<bool>,
}

/// Sorted matcher characters as a string, collapsing to the `*` wildcard
/// (Go `SuffixMatcher.Add`/`PrefixMatcher.Add` semantics).
fn matcher_string(chars: &[char]) -> String {
    if chars.contains(&'*') {
        return "*".to_owned();
    }
    let mut sorted: Vec<char> = chars.to_vec();
    sorted.sort();
    sorted.dedup();
    sorted.into_iter().collect()
}

/// Matcher characters from a Go matcher string.
fn matcher_chars(string: &str) -> Vec<char> {
    string.chars().collect()
}

/// Query annotation as a uid-style URL string (Go `url.URL.String()`).
fn query_string(query: &Query) -> String {
    let mut url = format!(
        "{}://{}/{}",
        query.scheme,
        crate::uid::path_escape(&query.host),
        crate::uid::path_escape(&query.path),
    );
    let pairs: Vec<String> = query
        .opts
        .chunks(2)
        .filter_map(|pair| match pair {
            [key, value] if !value.is_empty() => Some(format!(
                "{}={}",
                crate::uid::path_escape(key),
                crate::uid::path_escape(value)
            )),
            _ => None,
        })
        .collect();
    if !pairs.is_empty() {
        url.push('?');
        url.push_str(&pairs.join("&"));
    }
    url
}

/// Query annotation from a uid-style URL string.
fn parse_query(string: &str) -> Option<Query> {
    let (url, query) = string.split_once('?').map_or((string, ""), |(u, q)| (u, q));
    let (scheme, rest) = url.split_once("://")?;
    let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
    let opts: Vec<String> = query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .flat_map(|pair| pair.splitn(2, '='))
        .map(crate::uid::path_unescape)
        .collect();
    Some(Query {
        scheme: scheme.to_owned(),
        host: crate::uid::path_unescape(host),
        path: crate::uid::path_unescape(path),
        opts,
    })
}

impl Export {
    pub const VERSION: &'static str = "0.1.0";

    /// Serialize an invoked action into the export JSON format.
    ///
    /// Values are sorted by `Value` for deterministic output (Go MarshalJSON).
    pub fn from_parts(meta: &Meta, values: &[RawValue]) -> Self {
        let mut sorted: Vec<RawValue> = values.to_vec();
        sorted.sort_by(|a, b| a.value.cmp(&b.value));
        Self {
            version: Self::VERSION.to_owned(),
            meta: ExportMeta {
                messages: meta.messages.clone(),
                nospace: matcher_string(&meta.nospace),
                noprefix: matcher_string(&meta.no_prefix),
                usage: meta.usage.clone().unwrap_or_default(),
                queries: meta.queries.iter().map(query_string).collect(),
            },
            values: sorted
                .iter()
                .map(|value| ExportValue {
                    value: value.value.clone(),
                    display: value.display.clone(),
                    description: value.description.clone(),
                    style: value.style.clone(),
                    tag: value.tag.clone(),
                    uid: value.uid.clone().unwrap_or_default(),
                    nospace: value.nospace,
                })
                .collect(),
        }
    }

    /// Convert export values back into raw values.
    pub fn to_raw_values(&self) -> Vec<RawValue> {
        self.values
            .iter()
            .map(|value| RawValue {
                value: value.value.clone(),
                display: value.display.clone(),
                description: value.description.clone(),
                style: value.style.clone(),
                tag: value.tag.clone(),
                uid: (!value.uid.is_empty()).then(|| value.uid.clone()),
                nospace: value.nospace,
            })
            .collect()
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("export serialization cannot fail")
    }

    pub fn from_json(json: &str) -> Option<Self> {
        serde_json::from_str(json).ok()
    }
}

impl ExportMeta {
    /// Convert back into [`Meta`] (matcher strings and query URLs restored).
    pub fn to_meta(&self) -> Meta {
        Meta {
            messages: self.messages.clone(),
            nospace: matcher_chars(&self.nospace),
            no_prefix: matcher_chars(&self.noprefix),
            usage: (!self.usage.is_empty()).then(|| self.usage.clone()),
            queries: self.queries.iter().filter_map(|q| parse_query(q)).collect(),
        }
    }
}
