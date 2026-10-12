//! Materialized completions: the result of invoking an [`Action`].

use crate::meta::Meta;
use crate::value::{RawValues, sort, unique};

/// An [`Action`](super::Action) after invocation: values are resolved.
#[derive(Clone, Debug, Default)]
pub struct InvokedAction {
    pub meta: Meta,
    pub values: RawValues,
}

impl InvokedAction {
    /// Remove given values.
    pub fn filter(mut self, values: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let values: Vec<String> = values.into_iter().map(Into::into).collect();
        self.values.retain(|v| !values.contains(&v.value));
        self
    }

    /// Keep only given values.
    pub fn retain(mut self, values: impl IntoIterator<Item = impl Into<String>>) -> Self {
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
