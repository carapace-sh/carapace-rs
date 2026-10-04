//! Centralized `CARAPACE_*` environment configuration.

/// Read a `CARAPACE_*` env var as a flag (set and not `0`).
pub fn flag(name: &str) -> bool {
    std::env::var(name)
        .ok()
        .is_some_and(|value| !value.is_empty() && value != "0")
}

pub fn log_enabled() -> bool {
    flag("CARAPACE_LOG")
}

pub fn lenient() -> bool {
    flag("CARAPACE_LENIENT")
}

pub fn unfiltered() -> bool {
    flag("CARAPACE_UNFILTERED")
}

pub fn merge_flags() -> bool {
    flag("CARAPACE_MERGEFLAGS")
}

pub fn experimental() -> bool {
    flag("CARAPACE_EXPERIMENTAL")
}

pub fn nospace() -> Option<String> {
    std::env::var("CARAPACE_NOSPACE")
        .ok()
        .filter(|value| !value.is_empty())
}

pub fn color_disabled() -> bool {
    std::env::var("CARAPACE_COLOR").is_ok_and(|value| value == "disabled")
}

/// Description display length (default 80).
pub fn description_length() -> usize {
    std::env::var("CARAPACE_DESCRIPTION_LENGTH")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(80)
}
