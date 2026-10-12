//! Fish value formatting (port of Go `internal/shell/fish/action.go`).

use super::{Flags, trimmed_description};
use crate::value::RawValues;

/// Remove newline, carriage return and tab (would break the line format).
fn sanitize(value: &str) -> String {
    value.replace(['\n', '\r', '\t'], "")
}

/// Format values for fish: `value\tdescription` lines joined by newline.
///
/// Fish has no nospace mechanism; every completion gets a trailing space.
pub fn action_raw_values(values: &RawValues, flags: &Flags) -> String {
    values
        .iter()
        .map(|value| {
            format!(
                "{}\t{}",
                sanitize(&value.value),
                trimmed_description(&value.description, flags.description_length)
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
