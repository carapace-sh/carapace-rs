#![allow(dead_code)]

use carapace_rs_shlex::{Format, TokenSliceExt, split};

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

/// `Split(...).CurrentPipeline().FilterRedirects().Words().Strings()`
pub fn filtered_pipeline_words(s: &str, format: Format) -> Vec<String> {
    split(s, format)
        .unwrap()
        .current_pipeline()
        .filter_redirects()
        .words()
        .strings()
}

/// `len(Split(...).pipelines())`
pub fn pipeline_count(s: &str, format: Format) -> usize {
    split(s, format).unwrap().pipelines().len()
}
