//! Chained [`Action`] modifiers, ported from the Go carapace modifier set.

use std::sync::Arc;

use crate::action::Action;
use crate::context::Context;
use crate::meta::Meta;
use crate::value::RawValue;

impl Action {
    /// Remove given values.
    pub fn filter(self, values: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let values: Vec<String> = values.into_iter().map(Into::into).collect();
        self.map_invoked(move |_, invoked| invoked.filter(values.iter().cloned()))
    }

    /// Filter values that appear in the context arguments.
    pub fn filter_args(self) -> Self {
        self.map_invoked(|context, invoked| invoked.filter(context.args.iter().cloned()))
    }

    /// Filter values that appear in the context parts.
    pub fn filter_parts(self) -> Self {
        self.map_invoked(|context, invoked| invoked.filter(context.parts.iter().cloned()))
    }

    /// Keep only given values.
    pub fn retain(self, values: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let values: Vec<String> = values.into_iter().map(Into::into).collect();
        self.map_invoked(move |_, invoked| invoked.retain(values.iter().cloned()))
    }

    /// Deduplicate values by `(value, uid)` key.
    pub fn unique(self) -> Self {
        self.map_invoked(|_, invoked| invoked.unique())
    }

    /// Prepend a prefix to inserted values (not display).
    ///
    /// Skips invocation when the current value cannot extend the prefix.
    pub fn prefix(self, prefix: impl Into<String>) -> Self {
        let prefix = prefix.into();
        Self::callback(move |c| {
            let value = if crate::config::has_prefix(&c.value, &prefix) {
                crate::config::trim_prefix(&c.value, &prefix).to_owned()
            } else if crate::config::has_prefix(&prefix, &c.value) {
                String::new()
            } else {
                return Self::raw(Vec::new());
            };
            let mut context = c.clone();
            context.value = value;
            self.invoke(&context).prefix(&prefix).to_action()
        })
    }

    /// Append a suffix to inserted values (not display).
    pub fn suffix(self, suffix: impl Into<String>) -> Self {
        let suffix = suffix.into();
        self.map_invoked(move |_, invoked| invoked.suffix(&suffix))
    }

    /// Disable space suffix for the given characters (all when none given).
    pub fn nospace(self, suffixes: impl IntoIterator<Item = char>) -> Self {
        let suffixes: Vec<char> = suffixes.into_iter().collect();
        let mut meta = Meta::default();
        if suffixes.is_empty() {
            meta.nospace.push('*');
        }
        meta.nospace.extend(suffixes);
        self.with_meta(meta)
    }

    /// Prevent common prefix insertion for the given prefixes (all when none given).
    pub fn no_prefix(self, prefixes: impl IntoIterator<Item = char>) -> Self {
        let mut meta = Meta::default();
        let prefixes: Vec<char> = prefixes.into_iter().collect();
        if prefixes.is_empty() {
            meta.no_prefix.push('*');
        }
        meta.no_prefix.extend(prefixes);
        self.with_meta(meta)
    }

    /// Set a static style on all values.
    pub fn style(self, style: impl Into<String>) -> Self {
        let style = style.into();
        self.map_invoked(move |_, mut invoked| {
            for value in &mut invoked.values {
                value.style = style.clone();
            }
            invoked
        })
    }

    /// Set a per-value style.
    pub fn style_f<F>(self, f: F) -> Self
    where
        F: Fn(&RawValue) -> String + Send + Sync + 'static,
    {
        self.map_invoked(move |_, mut invoked| {
            for value in &mut invoked.values {
                value.style = f(value);
            }
            invoked
        })
    }

    /// Set a tag on all values.
    pub fn tag(self, tag: impl Into<String>) -> Self {
        let tag = tag.into();
        self.tag_f(move |_| tag.clone())
    }

    /// Set a per-value tag.
    pub fn tag_f<F>(self, f: F) -> Self
    where
        F: Fn(&RawValue) -> String + Send + Sync + 'static,
    {
        self.map_invoked(move |_, mut invoked| {
            for value in &mut invoked.values {
                value.tag = f(value);
            }
            invoked
        })
    }

    /// Set a usage string.
    pub fn usage(self, usage: impl Into<String>) -> Self {
        self.with_meta(Meta::default().usage(usage))
    }

    /// Skip invocation if the condition holds.
    pub fn unless(self, condition: bool) -> Self {
        if condition {
            Self::raw(Vec::new())
        } else {
            self
        }
    }

    /// Skip invocation if the callback returns true.
    pub fn unless_f<F>(self, f: F) -> Self
    where
        F: Fn(&Context) -> bool + Send + Sync + 'static,
    {
        Self::callback(move |c| {
            if f(c) {
                Self::raw(Vec::new())
            } else {
                self.clone()
            }
        })
    }

    /// Shift positional arguments left `n` times.
    pub fn shift(self, n: usize) -> Self {
        Self::callback(move |c| {
            let mut context = c.clone();
            let n = n.min(context.args.len());
            context.args.drain(..n);
            self.invoke(&context).to_action()
        })
    }

    /// Wrap in multipart completion: complete one segment of a delimited value.
    pub fn list(self, divider: &str) -> Self {
        let inner = self;
        Self::multiparts_n(divider, -1, move |c| {
            inner.invoke(c).to_action().nospace([])
        })
    }

    /// Like [`list`](Self::list) but filters values already present as parts.
    pub fn unique_list(self, divider: &str) -> Self {
        let inner = self;
        Self::multiparts_n(divider, -1, move |c| {
            inner
                .clone()
                .filter_parts()
                .invoke(c)
                .to_action()
                .nospace([])
        })
    }

    /// Like [`unique_list`](Self::unique_list) with a transform applied to parts first.
    pub fn unique_list_f<F>(self, divider: &str, f: F) -> Self
    where
        F: Fn(String) -> String + Send + Sync + 'static,
    {
        let inner = self;
        Self::multiparts_n(divider, -1, move |c| {
            let parts: Vec<String> = c.parts.iter().map(|p| f(p.clone())).collect();
            inner
                .clone()
                .filter(parts)
                .invoke(c)
                .to_action()
                .nospace([])
        })
    }

    /// Complete parts of an argument separated by `divider`.
    pub fn multiparts<F>(divider: &str, f: F) -> Self
    where
        F: Fn(&Context) -> Action + Send + Sync + 'static,
    {
        Self::multiparts_n(divider, -1, f)
    }

    /// Like [`multiparts`](Self::multiparts) but limits the number of parts.
    ///
    /// `n < 0` means unlimited, `n == 1` degenerates to plain invocation.
    pub fn multiparts_n<F>(divider: &str, n: isize, f: F) -> Self
    where
        F: Fn(&Context) -> Action + Send + Sync + 'static,
    {
        let divider = divider.to_owned();
        Self::callback(move |c| {
            if n == 0 {
                return Self::message(format!("invalid value for n [ActionMultiParts]: {n}"));
            }
            let mut context = c.clone();
            let mut prefix = String::new();
            if n == 1 {
                return f(&context).invoke(&context).to_action();
            }
            let splitted: Vec<String> = split_n(&context.value, &divider, n);
            if divider.is_empty() {
                prefix = context.value.clone();
                context.parts = prefix.chars().map(String::from).collect();
                match n {
                    n if n < 0 => context.value = String::new(),
                    n => {
                        let cut = (n as usize - 1).min(prefix.chars().count());
                        let cut_index = prefix
                            .char_indices()
                            .nth(cut)
                            .map(|(i, _)| i)
                            .unwrap_or(prefix.len());
                        context.value = prefix[cut_index..].to_owned();
                        prefix.truncate(cut_index);
                    }
                }
            } else if splitted.len() > 1 {
                context.value = splitted.last().cloned().unwrap_or_default();
                context.parts = splitted[..splitted.len() - 1].to_vec();
                prefix = format!("{}{divider}", context.parts.join(&divider));
            } else {
                context.parts = Vec::new();
            }
            let nospace = divider.chars().last().unwrap_or('*');
            f(&context)
                .invoke(&context)
                .prefix(&prefix)
                .to_action()
                .nospace([nospace])
        })
    }

    /// Parse export JSON back into an action (port of `ActionImport`).
    pub fn import(output: &str) -> Self {
        match crate::export::Export::from_json(output) {
            Some(export) => Self {
                meta: Meta {
                    messages: export.meta.messages.clone(),
                    nospace: export.meta.nospace.clone(),
                    no_prefix: Vec::new(),
                    usage: export.meta.usage.clone(),
                    queries: Vec::new(),
                },
                kind: crate::action::ActionKind::Raw(export.to_raw_values()),
            },
            None => Self::message(format!(
                "failed to parse export: {}",
                output.chars().take(80).collect::<String>()
            )),
        }
    }

    /// Set per-value UIDs with alternating key-value opts (experimental).
    pub fn uid(self, scheme: &str, host: &str, opts: Vec<(&'static str, String)>) -> Self {
        let scheme = scheme.to_owned();
        let host = host.to_owned();
        self.map_invoked(move |_, mut invoked| {
            for value in &mut invoked.values {
                let opts: Vec<(&str, &str)> = opts
                    .iter()
                    .map(|(key, value)| (*key, value.as_str()))
                    .collect();
                value.uid = Some(crate::uid::with_opts(&scheme, &host, &value.value, &opts));
            }
            invoked
        })
    }

    /// Set per-value UIDs via a function (experimental).
    pub fn uid_f<F>(self, f: F) -> Self
    where
        F: Fn(&str) -> String + Send + Sync + 'static,
    {
        self.map_invoked(move |_, mut invoked| {
            for value in &mut invoked.values {
                value.uid = Some(f(&value.value));
            }
            invoked
        })
    }

    /// Add query metadata via alternating key-value opts (experimental).
    pub fn query(
        self,
        scheme: &str,
        host: &str,
        path: &str,
        opts: Vec<(&'static str, String)>,
    ) -> Self {
        self.with_meta(
            Meta::default().query(crate::meta::Query {
                scheme: scheme.to_owned(),
                host: host.to_owned(),
                path: path.to_owned(),
                opts: opts
                    .iter()
                    .flat_map(|(key, value)| [key.to_string(), value.clone()])
                    .collect(),
            }),
        )
    }

    /// Change the working directory for the invocation.
    pub fn chdir(self, dir: impl Into<String>) -> Self {
        let dir = std::sync::Arc::new(dir.into());
        Self::callback(move |c| {
            let mut context = c.clone();
            context.dir = (*dir).clone().into();
            self.invoke(&context).to_action()
        })
    }

    /// Change the working directory for the invocation via a function.
    pub fn chdir_f<F>(self, f: F) -> Self
    where
        F: Fn(&Context) -> String + Send + Sync + 'static,
    {
        Self::callback(move |c| {
            let mut context = c.clone();
            context.dir = f(c).into();
            self.invoke(&context).to_action()
        })
    }

    /// Cache callback results to disk.
    ///
    /// `site` identifies the call site (Rust has no `runtime.Caller`),
    /// `timeout_secs` follows the Go semantics: negative never expires,
    /// zero always revalidates, positive expires after the given seconds.
    /// Error states (messages) are not cached.
    pub fn cache(self, site: &str, timeout_secs: i64, keys: Vec<String>) -> Self {
        if !self.is_callback() {
            return self;
        }
        let site = site.to_owned();
        Self::callback(move |c| {
            let Some(cache_file) = crate::cache::file(&site, &keys) else {
                return self.invoke(c).to_action();
            };
            if let Some(cached) = crate::cache::load(&cache_file, timeout_secs) {
                let meta = Meta {
                    messages: cached.meta.messages.clone(),
                    nospace: cached.meta.nospace.clone(),
                    no_prefix: Vec::new(),
                    usage: cached.meta.usage.clone(),
                    queries: Vec::new(),
                };
                return Action {
                    meta,
                    kind: crate::action::ActionKind::Raw(cached.to_raw_values()),
                };
            }
            let invoked = self.invoke(c);
            if invoked.meta.messages.is_empty() {
                let _ = crate::cache::write(
                    &cache_file,
                    &crate::export::Export::from_parts(&invoked.meta, &invoked.values),
                );
            }
            invoked.to_action()
        })
    }

    /// Use the alternative action when invocation exceeds `secs` seconds.
    ///
    /// The invocation runs on a detached thread and is not cancelled on
    /// timeout (Go's version has the same limitation since callbacks don't
    /// check for cancellation): it finishes in the background and its
    /// result is discarded.
    pub fn timeout(self, secs: u64, alternative: Self) -> Self {
        let inner = Arc::new(self);
        let alternative = Arc::new(alternative);
        Self::callback(move |c| {
            let context = c.clone();
            let inner = inner.clone();
            let alternative = alternative.clone();
            let (sender, receiver) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let _ = sender.send(inner.invoke(&context));
            });
            match receiver.recv_timeout(std::time::Duration::from_secs(secs)) {
                Ok(invoked) => invoked.to_action(),
                Err(_) => alternative.invoke(c).to_action(),
            }
        })
    }

    /// Merge the given meta into the action.
    pub(crate) fn with_meta(self, meta: Meta) -> Self {
        let mut merged = self.meta.clone();
        merged.merge(&meta);
        Self {
            meta: merged,
            ..self
        }
    }
}

fn split_n(value: &str, sep: &str, n: isize) -> Vec<String> {
    if n < 0 {
        return value.split(sep).map(String::from).collect();
    }
    value.splitn(n as usize, sep).map(String::from).collect()
}
