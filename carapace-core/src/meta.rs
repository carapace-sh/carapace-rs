//! Completion metadata carried alongside values.

/// Messages displayed instead of (or alongside) completions, e.g. errors.
pub type Messages = Vec<String>;

/// Metadata of an [`Action`](crate::action::Action) or [`InvokedAction`](crate::action::InvokedAction).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Meta {
    /// Messages to show (some shells render these natively).
    pub messages: Messages,
    /// Characters for which no space is inserted after completion (`*` matches all).
    pub nospace: Vec<char>,
    /// Usage string shown alongside messages (experimental).
    pub usage: Option<String>,
    /// Queries (experimental, tabdance).
    pub queries: Vec<Query>,
}

/// A query annotation (experimental).
#[derive(Clone, Debug, PartialEq)]
pub struct Query {
    pub scheme: String,
    pub host: String,
    pub path: String,
    pub opts: Vec<String>,
}

impl Meta {
    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.messages.push(message.into());
        self
    }

    pub fn nospace(mut self, suffixes: impl IntoIterator<Item = char>) -> Self {
        self.nospace.extend(suffixes);
        self
    }

    pub fn usage(mut self, usage: impl Into<String>) -> Self {
        self.usage = Some(usage.into());
        self
    }

    pub fn query(mut self, query: Query) -> Self {
        self.queries.push(query);
        self
    }

    /// Merge `other` into `self`, concatenating lists and preferring defined scalars.
    pub fn merge(&mut self, other: &Meta) {
        self.messages.extend(other.messages.iter().cloned());
        self.messages.sort();
        self.messages.dedup();
        self.nospace.extend(other.nospace.iter().copied());
        self.nospace.sort_unstable();
        self.nospace.dedup();
        if self.usage.is_none() {
            self.usage = other.usage.clone();
        }
        self.queries.extend(other.queries.iter().cloned());
    }
}
