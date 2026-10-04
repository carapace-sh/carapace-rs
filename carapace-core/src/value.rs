//! Raw completion values produced by invoking an [`Action`](crate::Action).

use std::cmp::Ordering;

/// A single completion candidate.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RawValue {
    /// Inserted value.
    pub value: String,
    /// Displayed value (defaults to `value`).
    pub display: String,
    /// Description shown alongside the value.
    pub description: String,
    /// Style reference (e.g. `blue`).
    pub style: String,
    /// Tag used for grouping (e.g. `flags`).
    pub tag: String,
    /// Unique identifier (experimental, `uid://` scheme).
    pub uid: Option<String>,
    /// Whether no space should be inserted after this value.
    pub nospace: Option<bool>,
}

impl RawValue {
    pub fn new(value: impl Into<String>) -> Self {
        let value = value.into();
        Self {
            display: value.clone(),
            value,
            ..Default::default()
        }
    }

    pub fn described(value: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            description: description.into(),
            ..Self::new(value)
        }
    }

    pub fn styled(value: impl Into<String>, style: impl Into<String>) -> Self {
        Self {
            style: style.into(),
            ..Self::new(value)
        }
    }

    pub fn styled_described(
        value: impl Into<String>,
        description: impl Into<String>,
        style: impl Into<String>,
    ) -> Self {
        Self {
            description: description.into(),
            style: style.into(),
            ..Self::new(value)
        }
    }

    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = value.into();
        self
    }

    pub fn display(mut self, display: impl Into<String>) -> Self {
        self.display = display.into();
        self
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    pub fn style(mut self, style: impl Into<String>) -> Self {
        self.style = style.into();
        self
    }

    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.tag = tag.into();
        self
    }

    pub fn uid(mut self, uid: impl Into<String>) -> Self {
        self.uid = Some(uid.into());
        self
    }

    pub fn nospace(mut self, nospace: bool) -> Self {
        self.nospace = Some(nospace);
        self
    }
}

pub type RawValues = Vec<RawValue>;

/// Sort values by display, then value.
pub fn sort(values: &mut RawValues) {
    values.sort_by(|a, b| {
        by_display(a, b).then_with(|| a.value.cmp(&b.value))
    });
}

/// Compare by display falling back to value (matches Go `ByDisplay`).
pub fn by_display(a: &RawValue, b: &RawValue) -> Ordering {
    match a.display.cmp(&b.display) {
        Ordering::Equal => a.value.cmp(&b.value),
        other => other,
    }
}

/// Deduplicate values by `(value, uid)` key, keeping first occurrence.
pub fn unique(values: &mut RawValues) {
    let mut seen = std::collections::HashSet::new();
    values.retain(|v| seen.insert((v.value.clone(), v.uid.clone())));
}
