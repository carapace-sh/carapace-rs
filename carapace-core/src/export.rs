//! The export JSON wire format (port of Go `internal/export`).
//!
//! Used for bridging, embedding and caching completions across process
//! boundaries.

use serde::{Deserialize, Serialize};

use crate::meta::Meta;
use crate::value::RawValue;

/// Serializable form of an invoked action.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Export {
    pub version: String,
    #[serde(rename = "Meta")]
    pub meta: ExportMeta,
    #[serde(rename = "Values")]
    pub values: Vec<ExportValue>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ExportMeta {
    #[serde(rename = "Messages", default)]
    pub messages: Vec<String>,
    #[serde(rename = "Nospace", default)]
    pub nospace: Vec<char>,
    #[serde(rename = "Usage", default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ExportValue {
    #[serde(rename = "Value")]
    pub value: String,
    #[serde(rename = "Display")]
    pub display: String,
    #[serde(rename = "Description")]
    pub description: String,
    #[serde(rename = "Style", default)]
    pub style: String,
    #[serde(rename = "Tag", default)]
    pub tag: String,
    #[serde(rename = "Nospace", default, skip_serializing_if = "Option::is_none")]
    pub nospace: Option<bool>,
}

impl Export {
    pub const VERSION: &'static str = "0.1.0";

    /// Serialize an invoked action into the export JSON format.
    pub fn from_parts(meta: &Meta, values: &[RawValue]) -> Self {
        Self {
            version: Self::VERSION.to_owned(),
            meta: ExportMeta {
                messages: meta.messages.clone(),
                nospace: meta.nospace.clone(),
                usage: meta.usage.clone(),
            },
            values: values
                .iter()
                .map(|value| ExportValue {
                    value: value.value.clone(),
                    display: value.display.clone(),
                    description: value.description.clone(),
                    style: value.style.clone(),
                    tag: value.tag.clone(),
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
                uid: None,
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
