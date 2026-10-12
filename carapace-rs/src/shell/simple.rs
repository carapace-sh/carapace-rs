//! Simple line-based formatters: oil, tcsh, cmd-clink, ion, bash-ble.

use super::trimmed_description;
use super::{Flags, common_display_prefix, common_value_prefix, matches_nospace};
use crate::meta::Meta;
use crate::value::{RawValue, RawValues};

/// Oil: values with inline `\u{1}` nospace indicator, single value plain.
pub mod oil {
    use super::*;

    fn sanitize(value: &str) -> String {
        value.replace(['\n', '\t'], "")
    }

    pub fn action_raw_values(
        current_word: &str,
        meta: &Meta,
        values: &RawValues,
        flags: &Flags,
    ) -> String {
        let _ = (current_word, flags);
        values
            .iter()
            .map(|value| {
                let mut value = value.clone();
                if matches_nospace(meta, &value) {
                    value.value.push('\u{1}');
                }
                if values.len() == 1 {
                    sanitize(&value.value)
                } else if !value.description.is_empty() {
                    format!(
                        "{} ({})",
                        value.value,
                        sanitize(&trimmed_description(
                            &value.description,
                            flags.description_length,
                        ))
                    )
                } else {
                    value.value
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Generate the oil completion snippet (Go `oil.SnippetSingle`).
    pub fn snippet(binary: &str) -> String {
        format!(
            r#"#!/bin/osh
_{binary}_completion() {{
  local compline="${{COMP_LINE:0:${{COMP_POINT}}}}"
  local IFS=$'\n'
  mapfile -t COMPREPLY < <(echo "$compline" | sed -e "s/ \$/ ''/" -e 's/"/\"/g' | xargs {binary} _carapace oil)
  [[ "${{COMPREPLY[@]}}" == "" ]] && COMPREPLY=() # fix for mapfile creating a non-empty array from empty command output
  [[ ${{COMPREPLY[0]}} == *[/=@:.,$'\001'] ]] && compopt -o nospace
  # TODO use mapfile
  # shellcheck disable=SC2206
  [[ ${{#COMPREPLY[@]}} -eq 1 ]] && COMPREPLY=(${{COMPREPLY[@]%%$'\001'}})
}}

complete -F _{binary}_completion {binary}
"#
        )
    }
}

/// Tcsh: backslash-escaped values with `_(description)` annotation.
pub mod tcsh {
    use super::*;

    fn sanitize(value: &str) -> String {
        value.replace(['\n', '\r', '\t'], "")
    }

    fn quoter(value: &str) -> String {
        let mut result = String::with_capacity(value.len());
        for c in value.chars() {
            if matches!(
                c,
                '\\' | '&'
                    | '<'
                    | '>'
                    | '`'
                    | '\''
                    | '"'
                    | '$'
                    | '#'
                    | '|'
                    | '?'
                    | '('
                    | ')'
                    | ';'
                    | ' '
                    | '['
                    | ']'
                    | '*'
            ) {
                result.push('\\');
            }
            result.push(c);
        }
        result
    }

    pub fn action_raw_values(
        current_word: &str,
        meta: &Meta,
        values: &RawValues,
        flags: &Flags,
    ) -> String {
        let _ = meta;
        let mut last_segment = current_word.to_owned();
        if let Ok(wordbreaks) = std::env::var("COMP_WORDBREAKS") {
            let wordbreaks = wordbreaks.replace(' ', "");
            if let Some(index) = current_word.rfind(|c: char| wordbreaks.contains(c)) {
                last_segment = current_word[index + 1..].to_owned();
            }
        }

        let mut values = values.clone();
        if values.len() > 1 {
            let display_prefix = common_display_prefix(&values);
            if !display_prefix.is_empty() {
                let value_prefix = common_value_prefix(&values);
                if last_segment != value_prefix {
                    values = vec![RawValue::new(value_prefix)];
                } else {
                    values[0].display = format!(" {}", values[0].display);
                }
            }
        }

        values
            .iter()
            .map(|value| {
                if values.len() == 1 || value.description.is_empty() {
                    quoter(&sanitize(&value.value))
                } else {
                    let description = quoter(
                        &sanitize(&trimmed_description(
                            &value.description,
                            flags.description_length,
                        ))
                        .replace(' ', "_"),
                    );
                    format!("{}_({})", quoter(&sanitize(&value.value)), description)
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Generate the tcsh completion snippet (Go `tcsh.SnippetSingle`).
    pub fn snippet(binary: &str) -> String {
        // TODO initial version - needs to handle open quotes
        format!(
            "complete \"{binary}\" 'p@*@`echo \"$COMMAND_LINE'\"''\"'\" | xargs {binary} _carapace tcsh `@@' ;"
        )
    }
}

/// cmd-clink: tab-separated `value\tdisplay\tdesc\tappendchar`.
pub mod cmd_clink {
    use super::*;

    fn sanitize(value: &str) -> String {
        value.replace(['\n', '\t'], "")
    }

    /// Re-lex `CARAPACE_COMPLINE` with cmd.exe rules and drop the binary
    /// name (port of Go `cmd_clink.Patch`).
    pub fn patch(args: Vec<String>) -> Vec<String> {
        let Ok(compline) = std::env::var("CARAPACE_COMPLINE") else {
            return args;
        };
        unsafe { std::env::remove_var("CARAPACE_COMPLINE") };
        if compline.is_empty() {
            return args;
        }
        let mut words =
            carapace_rs_shlex::complete(&compline, carapace_rs_shlex::Format::Cmd).words;
        if !words.is_empty() {
            words.remove(0); // the binary name
        }
        crate::log::log(&format!("patching args to {words:?}"));
        words
    }

    pub fn action_raw_values(
        current_word: &str,
        meta: &Meta,
        values: &RawValues,
        flags: &Flags,
    ) -> String {
        let _ = (current_word, flags);
        values
            .iter()
            .map(|value| {
                let append_char = if matches_nospace(meta, value) {
                    ""
                } else {
                    " "
                };
                format!(
                    "{}\t{}\t{}\t{}",
                    sanitize(&value.value),
                    sanitize(&value.display),
                    sanitize(&trimmed_description(
                        &value.description,
                        flags.description_length
                    )),
                    append_char
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Generate the cmd-clink lua snippet (Go `cmd_clink.Snippet`).
    pub fn snippet(binary: &str) -> String {
        let sanitized = binary.replace('-', "__");
        format!(
            r#"local function {sanitized}_completion(word, word_index, line_state, match_builder)
    match_builder:setnosort()
    match_builder:setvolatile()
    os.setenv('CARAPACE_COMPLINE', line_state:getline():sub(1, line_state:getcursor()))

    local file, pclose = io.popenyield('{binary} _carapace cmd-clink {binary}')

    if not file then
        return false
    end

    for line in file:lines() do
        local matches = string.explode(line, '\t')

        if matches[1] then
            match_builder:addmatch({{
                match       = matches[1],
                display     = matches[2],
                description = matches[3],
                type        = 'word',
                appendchar  = matches[4] or ''
            }})
        end
    end

    if pclose then
        pclose()
    else
        file:close()
    end

    return not match_builder:isempty()
end

clink.argmatcher(50, '{binary}', '{binary}.exe'):addarg({{nowordbreakchars="'`=+;,", {sanitized}_completion}}):loop(1)
"#
        )
    }
}

/// Ion: JSON `{Value, Display}` suggestions.
pub mod ion {
    use super::*;
    use serde::Serialize;

    fn sanitize(value: &str) -> String {
        value.replace(['\n', '\r'], "")
    }

    #[derive(Serialize)]
    struct Suggestion {
        #[serde(rename = "Value")]
        value: String,
        #[serde(rename = "Display")]
        display: String,
    }

    pub fn action_raw_values(
        _current_word: &str,
        meta: &Meta,
        values: &RawValues,
        _flags: &Flags,
    ) -> String {
        let suggestions: Vec<Suggestion> = values
            .iter()
            .map(|value| {
                let mut suggestion = RawValue { ..value.clone() };
                if !matches_nospace(meta, &suggestion) {
                    suggestion.value.push(' ');
                }
                Suggestion {
                    value: sanitize(&suggestion.value),
                    display: sanitize(&suggestion.display),
                }
            })
            .collect();
        serde_json::to_string(&suggestions).expect("ion serialization cannot fail")
    }
}

/// bash-ble: tab-delimited with `\x1c`-separated suffix field.
pub mod bash_ble {
    use super::*;

    pub fn action_raw_values(
        _current_word: &str,
        meta: &Meta,
        values: &RawValues,
        flags: &Flags,
    ) -> String {
        values
            .iter()
            .map(|value| {
                let suffix = if matches_nospace(meta, value) {
                    ""
                } else {
                    " "
                };
                format!(
                    "{}\t{}\x1c{}\x1c{}\x1c{}",
                    value.value,
                    value.display,
                    "",
                    suffix,
                    trimmed_description(&value.description, flags.description_length)
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Generate the bash-ble completion snippet (Go `bash_ble.SnippetSingle`).
    ///
    /// Wraps the bash snippet: Go strips the registration line with a
    /// `complete -F` regex that does not match the `complete -o noquote -F`
    /// form, so the bash body passes through unchanged.
    pub fn snippet(binary: &str) -> String {
        let bash_snippet = crate::shell::bash::snippet(binary);
        format!(
            r#"{bash_snippet}
_{binary}_completion_ble() {{
  if [[ ${{BLE_ATTACHED-}} ]]; then
    [[ :$comp_type: == *:auto:* ]] && return

    compopt -o ble/no-default
    bleopt complete_menu_style=desc

    local compline="${{COMP_LINE:0:${{COMP_POINT}}}}"
    local IFS=$'\n'
    local c
    mapfile -t c < <(echo "$compline" | sed -e "s/ \$/ ''/" -e 's/"/\"/g' | xargs {binary} _carapace bash-ble)
    [[ "${{c[*]}}" == "" ]] && c=() # fix for mapfile creating a non-empty array from empty command output

    local cand
    for cand in "${{c[@]}}"; do
      [ ! -z "$cand" ] && ble/complete/cand/yield mandb "${{cand%%$'\t'*}}" "${{cand##*$'\t'}}"
    done
  else
    complete -F _{binary}_completion {binary}
  fi
}}

complete -F _{binary}_completion_ble {binary}
"#
        )
    }
}
