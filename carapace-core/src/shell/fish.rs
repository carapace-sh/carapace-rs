//! Fish value formatting (port of Go `internal/shell/fish/action.go`).

use crate::value::RawValues;

/// Remove newline, carriage return and tab (would break the line format).
fn sanitize(value: &str) -> String {
    value.replace(['\n', '\r', '\t'], "")
}

/// Description trimmed per `CARAPACE_DESCRIPTION_LENGTH` (default 80).
fn trimmed_description(description: &str) -> String {
    let max_length = std::env::var("CARAPACE_DESCRIPTION_LENGTH")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(80);
    let first_line = description.split('\n').next().unwrap_or_default();
    let trimmed = first_line.trim();
    let runes: Vec<char> = trimmed.chars().collect();
    if runes.len() > max_length {
        if max_length <= 3 {
            return runes[..max_length].iter().collect();
        }
        let mut result: String = runes[..max_length - 3].iter().collect();
        result.push_str("...");
        return result;
    }
    trimmed.to_owned()
}

/// Format values for fish: `value\tdescription` lines joined by newline.
///
/// Fish has no nospace mechanism; every completion gets a trailing space.
pub fn action_raw_values(values: &RawValues) -> String {
    values
        .iter()
        .map(|value| {
            format!(
                "{}\t{}",
                sanitize(&value.value),
                trimmed_description(&value.description)
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Generate the fish completion snippet for a binary.
pub fn snippet(binary: &str) -> String {
    format!(
        r#"function _{binary}_completion
  set --local data
  IFS='' set data (echo (commandline -cp)'' | sed "s/ $/ ''/" | xargs {binary} _carapace fish 2>/dev/null)
  if [ $status -eq 1 ]
    IFS='' set data (echo (commandline -cp)"'" | sed "s/ $/ ''/" | xargs {binary} _carapace fish 2>/dev/null)
    if [ $status -eq 1 ]
      IFS='' set data (echo (commandline -cp)'"' | sed "s/ $/ ''/" | xargs {binary} _carapace fish 2>/dev/null)
    end
  end
  echo $data
end

complete -e '{binary}'
complete -c '{binary}' -f -a '(_{binary}_completion)' -r
"#
    )
}
