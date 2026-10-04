//! Zsh value formatting (port of Go `internal/shell/zsh/action.go`).

use crate::meta::Meta;
use crate::value::RawValue;
use crate::value::RawValues;
use crate::shell::sgr;

/// Remove newline, carriage return and tab.
fn sanitize(value: &str) -> String {
    value.replace(['\n', '\r', '\t'], "")
}

/// `'` handling in single-quoted context.
fn quoting_replacer(value: &str) -> String {
    value.replace('\'', "'\\''")
}

/// Escape inside double quotes.
fn quoting_escaping_replacer(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    for c in value.chars() {
        if matches!(c, '\\' | '"' | '$' | '`') {
            result.push('\\');
        }
        result.push(c);
    }
    result
}

/// Escape for unquoted zsh context.
fn default_replacer(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    for c in value.chars() {
        if matches!(
            c,
            '\\' | '&' | '<' | '>' | '`' | '\'' | '"' | '{' | '}' | '$' | '#' | '|' | '?' | '('
                | ')' | ';' | ' ' | '[' | ']' | '*' | '~'
        ) {
            result.push('\\');
        }
        result.push(c);
    }
    result
}

/// Additional escaping for `_describe` display arrays (colon is the separator).
fn describe_replacer(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    for c in value.chars() {
        if matches!(c, '\\' | ':') {
            result.push('\\');
        }
        result.push(c);
    }
    result
}

/// Quote a value, preserving `~/` expansion.
fn quote_value(value: &str) -> String {
    if value.starts_with("~/") {
        format!("~{}", default_replacer(&value[1..]))
    } else {
        default_replacer(value)
    }
}

/// Quoting state of the current word, derived from `CARAPACE_COMPLINE`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Default,
    /// Word starts with `"`: values must end with `"`.
    QuotingEscaping,
    /// Word starts with `'`: values must end with `'`.
    Quoting,
    /// Word starts and ends with `"`: nospace workaround.
    FullQuotingEscaping,
    /// Word starts and ends with `'`: nospace workaround.
    FullQuoting,
}

fn quoting_state(current_word: &str) -> State {
    let mut chars = current_word.chars();
    match chars.next() {
        Some('\'') => {
            if current_word.len() > 1 && current_word.ends_with('\'') {
                State::FullQuoting
            } else {
                State::Quoting
            }
        }
        Some('"') => {
            if current_word.len() > 1 && current_word.ends_with('"') {
                State::FullQuotingEscaping
            } else {
                State::QuotingEscaping
            }
        }
        _ => State::Default,
    }
}

/// The raw current word as zsh passes it: last token of `CARAPACE_COMPLINE`.
///
/// A full shlex re-lex replaces this approximation; zsh does not split on
/// `:` or `=` so the raw token preserves quoting characters.
fn raw_current_word(current_word: &str) -> String {
    match std::env::var("CARAPACE_COMPLINE") {
        Ok(compline) if !compline.is_empty() => compline
            .split_whitespace()
            .last()
            .unwrap_or(current_word)
            .to_owned(),
        _ => current_word.to_owned(),
    }
}

fn matches_nospace(meta: &Meta, value: &RawValue) -> bool {
    if value.nospace == Some(true) {
        return true;
    }
    meta.nospace
        .iter()
        .any(|suffix| *suffix == '*' || value.value.ends_with(*suffix))
}

/// Escape a display string for zstyle patterns.
fn zstyle_replacer(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    for c in value.chars() {
        if matches!(c, '#' | '*' | '(' | ')' | '[' | ']' | '|' | '~') {
            result.push('\\');
        }
        result.push(c);
    }
    result
}

/// Generate `zstyle list-colors` patterns for the values.
fn zstyles(values: &RawValues) -> String {
    let mut formatted = Vec::new();
    for value in values {
        let display = zstyle_replacer(&sanitize(&value.display));
        let value_sgr = if value.style.is_empty() {
            sgr("default")
        } else {
            sgr(&value.style)
        };
        formatted.push(format!(
            "=(#b)({display})([ ]## -- *)=0={value_sgr}={}",
            sgr("default")
        ));
        formatted.push(format!("=(#b)({display})=0={value_sgr}"));
    }
    formatted.push(format!("=(#b)(-- *)=0={}", sgr("default")));
    formatted.join(":")
}

/// Render messages for `_message -r` with SGR styling.
fn message(meta: &Meta) -> String {
    let mut formatted = Vec::new();
    for message in &meta.messages {
        let msg = message.replace(['\n', '\r', '\t', '\u{b}', '\u{c}', '\u{8}'], "");
        formatted.push(format!("\x1b[{}m{}\x1b[{}m", sgr("underlined"), msg, sgr("default")));
    }
    if let Some(usage) = &meta.usage {
        let usage = usage.replace(['\n', '\r', '\t', '\u{b}', '\u{c}', '\u{8}'], "");
        formatted.push(format!("\x1b[{}m{}\x1b[{}m", sgr("italic"), usage, sgr("default")));
    }
    formatted.join("\n")
}

/// Format values for zsh: `zstyle\001message\001noprefix\001tag-groups\001`.
///
/// Each tag group is `tag\003displays\003values`, groups separated by `\002`.
pub fn action_raw_values(current_word: &str, meta: &Meta, values: &RawValues) -> String {
    let state = quoting_state(&raw_current_word(current_word));

    let mut tag_groups: Vec<(String, Vec<String>, Vec<String>)> = Vec::new();
    for value in values {
        let tag = if value.tag.is_empty() {
            "default".to_owned()
        } else {
            value.tag.clone()
        };

        let mut zsh_value = sanitize(&value.value);
        match state {
            State::QuotingEscaping => {
                zsh_value = format!(
                    "{}\"",
                    describe_replacer(&quoting_escaping_replacer(&zsh_value))
                )
            }
            State::Quoting => {
                zsh_value =
                    format!("{}'", describe_replacer(&quoting_replacer(&zsh_value)))
            }
            State::FullQuotingEscaping => {
                zsh_value = describe_replacer(&quoting_escaping_replacer(&zsh_value))
            }
            State::FullQuoting => {
                zsh_value = describe_replacer(&quoting_replacer(&zsh_value))
            }
            State::Default => {
                zsh_value = describe_replacer(&quote_value(&zsh_value))
            }
        }

        if !matches_nospace(meta, value) && !matches!(state, State::FullQuotingEscaping | State::FullQuoting) {
            zsh_value.push(' ');
        }

        let display = describe_replacer(&sanitize(&value.display));
        let description = sanitize(&value.description);

        let entry = if description.trim().is_empty() {
            display
        } else {
            format!("{display}:{description}")
        };

        if let Some(group) = tag_groups.iter_mut().find(|(t, _, _)| *t == tag) {
            group.1.push(entry);
            group.2.push(zsh_value);
        } else {
            tag_groups.push((tag, vec![entry], vec![zsh_value]));
        }
    }

    let blocks: Vec<String> = tag_groups
        .into_iter()
        .map(|(tag, displays, vals)| format!("{tag}\u{3}{}\u{3}{}", displays.join("\n"), vals.join("\n")))
        .collect();

    let tag_groups = format!("{}\u{2}", blocks.join("\u{2}"));
    format!(
        "{}\u{1}{}\u{1}{}\u{1}{}\u{1}",
        zstyles(values),
        message(meta),
        false,
        tag_groups
    )
}

/// Generate the zsh completion snippet for a binary.
pub fn snippet(binary: &str) -> String {
    format!(
        r#"#compdef {binary}
function _{binary}_completion {{
  local compline=${{words[@]:0:$CURRENT}}
  local IFS=$'\n'
  local lines

  # shellcheck disable=SC2086,SC2154,SC2155
  lines="$(echo "${{compline}}''" | CARAPACE_COMPLINE="${{compline}}" xargs {binary} _carapace zsh 2>/dev/null)"
  if [ $? -eq 1 ]; then
    lines="$(echo "${{compline}}'" | CARAPACE_COMPLINE="${{compline}}" xargs {binary} _carapace zsh 2>/dev/null)"
    if [ $? -eq 1 ]; then
      lines="$(echo "${{compline}}\"" | CARAPACE_COMPLINE="${{compline}}" xargs {binary} _carapace zsh 2>/dev/null)"
    fi
  fi

  local zstyle message data
  IFS=$'\001' read -r -d '' zstyle message data <<<"${{lines}}"
  zstyle ":completion:${{curcontext}}:*" list-colors "${{zstyle}}"
  zstyle ":completion:${{curcontext}}:*" group-name ''
  [ -z "$message" ] || _message -r "${{message}}"

  local block tag displays values displaysArr valuesArr
  while IFS=$'\002' read -r -d $'\002' block; do
    IFS=$'\003' read -r -d '' tag displays values <<<"${{block}}"
    # shellcheck disable=SC2034
    IFS=$'\n' read -r -d $'\004' -A displaysArr <<<"${{displays}}"$'\004'
    IFS=$'\n' read -r -d $'\004' -A valuesArr <<<"${{values}}"$'\004'

    [[ ${{#valuesArr[@]}} -gt 1 ]] && _describe -t "${{tag}}" "${{tag}}" displaysArr valuesArr -Q -S ''
  done <<<"${{data}}"
}}
compquote '' 2>/dev/null && _{binary}_completion
compdef _{binary}_completion {binary}
"#
    )
}
