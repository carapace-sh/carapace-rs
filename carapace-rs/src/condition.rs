//! Condition predicates for actions (port of Go `pkg/condition`).

use crate::context::Context;

/// Whether `Context.Value` has a path prefix (Go `util.HasPathPrefix`):
/// a leading `.`, `/`, `~` or (on Windows) volume prefix.
pub fn completing_path(context: &Context) -> bool {
    has_path_prefix(&context.value)
}

/// Like [`completing_path`] but also true when the value contains a `/`
/// (Go `CompletingPathS`).
pub fn completing_path_s(context: &Context) -> bool {
    completing_path(context) || context.value.contains('/')
}

/// Whether the value looks like a path (Go `util.HasPathPrefix`).
pub fn has_path_prefix(value: &str) -> bool {
    value.starts_with('.') || value.starts_with('/') || value.starts_with('~')
}
