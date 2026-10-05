//! Parsing of spec flag keys like `-v, --verbose`, `--file=`, `--tag*`,
//! `--output?`, `--secret&`, `--config!`.

use carapace_core::model::{ArgumentStyle, Flag, Nargs};

use super::FlagSpec;

/// Parse a flag key with modifier suffixes into a model flag.
///
/// Returns `None` when the key cannot be mapped (e.g. non-POSIX shorthands).
pub fn parse(key: &str, spec: &FlagSpec) -> Option<Flag> {
    let key = key.trim();
    if key.is_empty() {
        return None;
    }

    let (flag_part, suffixes) = split_suffixes(key);
    if suffixes.contains(&'*') && suffixes.contains(&'?') && suffixes.contains(&'=') {
        // unsupported combination of repeatable + optarg
        return None;
    }

    #[allow(clippy::assign_op_pattern)]
    let mut takes_value = suffixes.contains(&'=');
    let optional = suffixes.contains(&'?');
    let nargs = match spec.nargs {
        Some(2..=i64::MAX) => {
            takes_value = true;
            Nargs::Exact(spec.nargs.unwrap() as usize)
        }
        Some(-1) => {
            takes_value = true;
            Nargs::Any
        }
        Some(..=-2) => {
            takes_value = true;
            Nargs::AtLeastOne
        }
        _ => Nargs::Exact(1),
    };

    let mut flag = Flag {
        takes_value,
        optional,
        nargs,
        repeatable: suffixes.contains(&'*'),
        hidden: suffixes.contains(&'&'),
        required: suffixes.contains(&'!'),
        description: spec.description.clone(),
        argument_style: ArgumentStyle::ALL,
        optarg_delimiter: '=',
        ..Default::default()
    };

    let mut longs: Vec<&str> = Vec::new();
    let mut shorthands: Vec<char> = Vec::new();
    for part in flag_part.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some(long) = part.strip_prefix("--") {
            longs.push(long);
            continue;
        }
        let shorthand = part.strip_prefix('-')?;
        let mut chars = shorthand.chars();
        match (chars.next(), chars.as_str().is_empty()) {
            (Some(single), true) => shorthands.push(single),
            _ => return None, // non-POSIX shorthand series unsupported
        }
    }

    if let Some(first_long) = longs.first() {
        flag.name = (*first_long).to_owned();
    }
    for alias in longs.iter().skip(1) {
        flag.aliases.push((*alias).to_owned());
    }
    flag.shorthand = shorthands.first().copied();
    // additional shorthands are aliases in POSIX mode
    for shorthand in shorthands.iter().skip(1) {
        flag.aliases.push(shorthand.to_string());
    }

    if flag.name.is_empty() && flag.shorthand.is_none() {
        return None;
    }
    let id = flag
        .name
        .is_empty()
        .then(|| flag.shorthand.map(|c| c.to_string()))
        .flatten()
        .unwrap_or_else(|| flag.name.clone());
    flag.id = id;
    Some(flag)
}

/// Split trailing modifier suffix characters off the key.
fn split_suffixes(key: &str) -> (&str, Vec<char>) {
    let trimmed = key.trim_end();
    let mut end = trimmed.len();
    let mut suffixes = Vec::new();
    while let Some(char) = trimmed[..end].chars().last() {
        if matches!(char, '=' | '*' | '?' | '&' | '!') {
            suffixes.push(char);
            end -= char.len_utf8();
        } else {
            break;
        }
    }
    (trimmed[..end].trim_end(), suffixes)
}
