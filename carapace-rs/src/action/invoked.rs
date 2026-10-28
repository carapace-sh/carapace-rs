//! Materialized completions: the result of invoking an [`Action`].

use crate::meta::Meta;
use crate::value::{RawValues, sort, unique};

/// An [`Action`](super::Action) after invocation: values are resolved.
#[derive(Clone, Debug, Default)]
pub struct InvokedAction {
    pub(crate) meta: Meta,
    pub(crate) values: RawValues,
}

impl InvokedAction {
    /// Convert into the export wire format (Go `InvokedAction.MarshalJSON`).
    ///
    /// This is the only sanctioned way to read an invoked action's
    /// contents: values and metadata are observable exclusively through
    /// the wire format, mirroring Go where `MarshalJSON` is the public
    /// read path and the fields stay unexported.
    ///
    /// Values are sorted by `Value` for deterministic output.
    pub fn to_export(&self) -> crate::export::Export {
        crate::export::Export::from_parts(&self.meta, &self.values)
    }

    /// Create an [`Action`](super::Action) completing segments of the
    /// values split by `dividers` (Go `InvokedAction.ToMultiPartsA`):
    /// values with the current prefix contribute their next segment,
    /// candidates keep the entered prefix, and the last divider character
    /// (or `*` for an empty divider) joins the nospace matcher.
    pub fn to_multiparts_a(&self, dividers: &[&str]) -> super::Action {
        let dividers: Vec<String> = dividers.iter().map(|divider| divider.to_string()).collect();
        let values = self.values.clone();
        super::Action::callback(move |c| {
            let splitted_value = tokenize(&c.value, &dividers);

            let mut unique: std::collections::BTreeMap<String, crate::value::RawValue> =
                std::collections::BTreeMap::new();
            for value in &values {
                if !value.value.starts_with(&c.value) {
                    continue;
                }
                let splitted = tokenize(&value.value, &dividers);
                if splitted.len() < splitted_value.len() {
                    continue;
                }
                let joined = splitted[..splitted_value.len()].join("");
                let display = splitted[splitted_value.len() - 1].clone();
                if splitted.len() == splitted_value.len() {
                    unique.insert(
                        joined.clone(),
                        crate::value::RawValue {
                            value: joined.clone(),
                            display,
                            description: value.description.clone(),
                            style: value.style.clone(),
                            tag: value.tag.clone(),
                            ..value.clone()
                        },
                    );
                } else {
                    unique.insert(
                        joined.clone(),
                        crate::value::RawValue {
                            value: joined,
                            display,
                            tag: value.tag.clone(),
                            ..Default::default()
                        },
                    );
                }
            }

            let mut action = super::Action::raw(unique.into_values().collect());
            for divider in &dividers {
                match divider.chars().last() {
                    None => {
                        action = action.nospace(['*']);
                        break;
                    }
                    Some(last) => action = action.nospace([last]),
                }
            }
            action
        })
    }

    /// Remove given values.
    ///
    /// Use [`filter_with`](Self::filter_with) when the values are only known
    /// at runtime.
    pub fn filter<const N: usize>(self, values: [&str; N]) -> Self {
        self.filter_with(values)
    }

    /// Remove a value list built at runtime.
    pub fn filter_with(mut self, values: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let values: Vec<String> = values.into_iter().map(Into::into).collect();
        self.values.retain(|v| !values.contains(&v.value));
        self
    }

    /// Keep only given values.
    ///
    /// Use [`retain_with`](Self::retain_with) when the values are only known
    /// at runtime.
    pub fn retain<const N: usize>(self, values: [&str; N]) -> Self {
        self.retain_with(values)
    }

    /// Keep only a value list built at runtime.
    pub fn retain_with(mut self, values: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let values: Vec<String> = values.into_iter().map(Into::into).collect();
        self.values.retain(|v| values.contains(&v.value));
        self
    }

    /// Prepend a prefix to inserted values (not display).
    pub fn prefix(mut self, prefix: &str) -> Self {
        for value in &mut self.values {
            value.value = format!("{prefix}{}", value.value);
        }
        self
    }

    /// Append a suffix to inserted values (not display).
    pub fn suffix(mut self, suffix: &str) -> Self {
        for value in &mut self.values {
            value.value = format!("{}{suffix}", value.value);
        }
        self
    }

    /// Deduplicate values by `(value, uid)` key.
    pub fn unique(self) -> Self {
        let mut values = self.values;
        unique(&mut values);
        Self { values, ..self }
    }

    /// Sort values by display.
    pub fn sort(mut self) -> Self {
        sort(&mut self.values);
        self
    }

    /// Merge other invoked actions into this one, deduplicating.
    pub fn merge(mut self, others: impl IntoIterator<Item = InvokedAction>) -> Self {
        for other in others {
            self.meta.merge(&other.meta);
            self.values.extend(other.values);
        }
        let mut values = self.values;
        unique(&mut values);
        Self { values, ..self }
    }

    /// Convert into a lazy [`Action`](super::Action) again.
    pub fn to_action(self) -> super::Action {
        self.into()
    }
}

/// Merge multiple invoked actions into one.
pub fn merge_all(actions: impl IntoIterator<Item = InvokedAction>) -> InvokedAction {
    let mut iter = actions.into_iter();
    match iter.next() {
        Some(first) => first.merge(iter),
        None => InvokedAction::default(),
    }
}

/// Split `s` into tokens that keep their trailing divider (Go
/// `invokedAction.tokenize`, `strings.SplitAfter` semantics including the
/// trailing empty token).
fn tokenize(s: &str, dividers: &[String]) -> Vec<String> {
    let Some(first) = dividers.first() else {
        return vec![s.to_owned()];
    };
    let mut result = Vec::new();
    let mut rest = s;
    while let Some(index) = rest.find(first.as_str()) {
        let mut tokens = tokenize(&rest[..index], &dividers[1..]);
        if !tokens.is_empty() {
            let last = tokens.len() - 1;
            tokens[last].push_str(first);
        }
        result.extend(tokens);
        rest = &rest[index + first.len()..];
    }
    result.extend(tokenize(rest, &dividers[1..]));
    result
}
