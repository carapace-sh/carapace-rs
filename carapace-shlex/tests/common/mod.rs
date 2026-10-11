#![allow(dead_code)]

use carapace_shlex::{complete, split, Format, TokenSliceExt};

/// `Split(...).Words().Strings()`
pub fn split_words(s: &str, format: Format) -> Vec<String> {
    split(s, format).unwrap().words().strings()
}

/// `Split(...).Words().Strings()` of the current pipeline.
pub fn pipeline_words(s: &str, format: Format) -> Vec<String> {
    split(s, format)
        .unwrap()
        .current_pipeline()
        .words()
        .strings()
}

/// `len(Split(...).pipelines())`
pub fn pipeline_count(s: &str, format: Format) -> usize {
    split(s, format).unwrap().pipelines().len()
}
