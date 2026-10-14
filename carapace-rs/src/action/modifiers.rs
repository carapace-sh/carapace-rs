//! Chained [`Action`] modifiers, ported from the Go carapace modifier set.

use std::sync::Arc;

use crate::action::{Action, InvokedAction};
use crate::context::Context;
use crate::meta::Meta;
use crate::value::RawValue;
use carapace_rs_shlex::TokenSliceExt;

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
            let case_insensitive = c.config.case_insensitive;
            let value = if crate::config::has_prefix(&c.value, &prefix, case_insensitive) {
                crate::config::trim_prefix(&c.value, &prefix, case_insensitive).to_owned()
            } else if crate::config::has_prefix(&prefix, &c.value, case_insensitive) {
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

    /// Set a per-value style. The closure receives the invocation context
    /// (Go `style.Context`), so styles may resolve dynamically at invoke
    /// time (e.g. `shell::for_path`, config-backed styles).
    pub fn style_f<F>(self, f: F) -> Self
    where
        F: Fn(&Context, &RawValue) -> String + Send + Sync + 'static,
    {
        self.map_invoked(move |context, mut invoked| {
            for value in &mut invoked.values {
                value.style = f(context, value);
            }
            invoked
        })
    }

    /// Set a style read at invocation time from the given cell (Go
    /// `Action.StyleR`): runtime changes to the cell apply to later
    /// completions.
    pub fn style_r(self, style: std::sync::Arc<std::sync::RwLock<String>>) -> Self {
        Self::callback(move |c| {
            let style = style
                .read()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .clone();
            self.invoke(c).to_action().style(style)
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

    /// Set the usage via a function (Go `Action.UsageF`); an empty
    /// string leaves the usage untouched.
    pub fn usage_f<F>(self, f: F) -> Self
    where
        F: Fn() -> String + Send + Sync + 'static,
    {
        Self::callback(move |c| {
            let mut invoked = self.invoke(c);
            let usage = f();
            if !usage.is_empty() {
                invoked.meta.usage = Some(usage);
            }
            invoked.to_action()
        })
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

    /// Complete a value that is itself a shell-parsed command line
    /// (port of Go `Action.Split`).
    ///
    /// The value is re-lexed; the wrapped action is invoked with the
    /// preceding words as args and the current word as value. Candidates
    /// are shell-quoted (open quotes closed), spaced unless the inner
    /// action claims nospace, and the raw prefix before the current word
    /// is re-attached. A `$`-prefixed current word completes environment
    /// variable names instead.
    pub fn split(self, format: crate::Format) -> Self {
        self.split_impl(format, false)
    }

    /// Like [`split`](Self::split) but supports pipelines (port of Go
    /// `Action.SplitP`): the wrapped action's context is the current
    /// pipeline's words with redirects filtered, and the cursor completing
    /// a redirect target completes files instead.
    pub fn split_p(self, format: crate::Format) -> Self {
        self.split_impl(format, true)
    }

    fn split_impl(self, format: crate::Format, pipelines: bool) -> Self {
        let inner = self;
        Self::callback(move |c| {
            let completion = carapace_rs_shlex::complete(&c.value, format);
            // rune span to byte offset
            let byte_offset = |rune_index: usize| {
                c.value
                    .char_indices()
                    .nth(rune_index)
                    .map(|(i, _)| i)
                    .unwrap_or(c.value.len())
            };

            // Go: context = NewContext(words...) — Value is the last word,
            // Args everything before it. Split uses the words of ALL tokens
            // (redirects included); SplitP the current pipeline's filtered
            // words.
            let mut words = if pipelines {
                completion.words.clone()
            } else {
                completion.tokens.words().strings()
            };
            if words.is_empty() {
                words.push(String::new());
            }
            let current_word = words.pop().unwrap();

            let mut context = c.clone();
            context.parts = Vec::new();
            context.value = current_word;
            context.args = words;

            // `$name` completes environment variable names (the reference's
            // replaced region is the variable's span, e.g. the naive
            // COMP_WORDS suffix for bash)
            if let Some(variable) = &completion.variable {
                let prefix = c.value[..byte_offset(variable.span().start)].to_owned();
                context.value = variable.name.clone();
                let names: Vec<String> = context
                    .env
                    .keys()
                    .filter(|key| key.starts_with(&context.value))
                    .cloned()
                    .collect();
                let values = names
                    .iter()
                    .map(|name| crate::value::RawValue::new(variable.insert(name)))
                    .collect();
                return InvokedAction {
                    values,
                    ..Default::default()
                }
                .prefix(&prefix)
                .to_action()
                .nospace([]);
            }

            // SplitP completes files for a redirect target: the filtered
            // words lost the empty target word, so the value is reset to
            // the completion's current word (as in Go)
            let action = if pipelines && completion.is_redirect {
                context.value = completion.current_word.clone();
                Self::files(Vec::<String>::new())
            } else {
                inner.clone()
            };

            let prefix = c.value[..byte_offset(completion.span.start)].to_owned();
            let invoked = action.invoke(&context);
            let values = invoked
                .values
                .into_iter()
                .map(|mut value| {
                    let nospace = crate::shell::matches_nospace(&invoked.meta, &value);
                    if !nospace || value.value.contains(' ') {
                        value.value = completion.quote(&value.value);
                    }
                    if !nospace {
                        value.value.push(' ');
                    }
                    value
                })
                .collect();
            InvokedAction { values, ..invoked }
                .prefix(&prefix)
                .to_action()
                .nospace([])
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

    /// Suppress error messages matching the given regular expressions
    /// (port of Go `Action.Suppress`). An invalid expression surfaces as
    /// a message instead.
    pub fn suppress(self, exprs: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let exprs: Vec<String> = exprs.into_iter().map(Into::into).collect();
        Self::callback(move |c| {
            let invoked = self.invoke(c);
            let mut messages = invoked.meta.messages.clone();
            for expr in &exprs {
                match regex::Regex::new(expr) {
                    Err(error) => return Self::message(error.to_string()),
                    Ok(expr) => messages.retain(|message| !expr.is_match(message)),
                }
            }
            let mut meta = invoked.meta.clone();
            meta.messages = messages;
            InvokedAction { meta, ..invoked }.to_action()
        })
    }

    /// Complete parts of an argument separated by `divider`, resolving
    /// placeholder segments of `pattern` via `f` (port of Go
    /// `Action.MultiPartsP`). Static segments must match the entered parts;
    /// a placeholder segment (matching `pattern`) is completed by `f` with
    /// the data entered for all placeholders seen so far.
    pub fn multipartsp<F>(self, divider: &str, pattern: &str, f: F) -> Self
    where
        F: Fn(&str, &std::collections::HashMap<String, String>) -> Self + Send + Sync + 'static,
    {
        let divider = divider.to_owned();
        let pattern = match regex::Regex::new(pattern) {
            Ok(pattern) => Some(pattern),
            Err(error) => return Self::message(error.to_string()),
        };
        let f = std::sync::Arc::new(f);
        Self::callback(move |c| {
            let Some(pattern) = pattern.clone() else {
                return Self::message("invalid pattern");
            };
            let invoked = self.invoke(c);

            // split the value like `ActionMultiParts` (Go delegates to it)
            let mut context = c.clone();
            let mut prefix = String::new();
            let splitted: Vec<String> = split_n(&context.value, &divider, -1);
            if splitted.len() > 1 {
                context.value = splitted.last().cloned().unwrap_or_default();
                context.parts = splitted[..splitted.len() - 1].to_vec();
                prefix = format!("{}{divider}", context.parts.join(&divider));
            } else {
                context.parts = Vec::new();
            }

            let mut entered: std::collections::HashMap<String, String> =
                std::collections::HashMap::new();
            let mut static_matches: std::collections::BTreeSet<usize> =
                std::collections::BTreeSet::new();
            let mut candidates: std::collections::BTreeMap<String, crate::value::RawValue> =
                std::collections::BTreeMap::new();

            'path: for value in &invoked.values {
                let segments: Vec<&str> = value.value.split(&divider).collect();
                for (index, segment) in segments.iter().enumerate() {
                    if index >= context.parts.len() {
                        break;
                    } else if *segment != context.parts[index] {
                        if !pattern.is_match(segment) {
                            continue 'path;
                        }
                        entered.insert((*segment).to_owned(), context.parts[index].clone());
                    } else {
                        static_matches.insert(index);
                    }
                }

                if segments.len() < context.parts.len() + 1 {
                    continue 'path;
                }
                for index in &static_matches {
                    if *index < segments.len() && segments[*index] != context.parts[*index] {
                        continue 'path;
                    }
                }

                if segments.len() == context.parts.len() + 1 {
                    candidates.insert(segments[context.parts.len()].to_owned(), value.clone());
                } else {
                    candidates.insert(
                        format!("{}{divider}", segments[context.parts.len()]),
                        crate::value::RawValue::default(),
                    );
                }
            }

            let mut actions: Vec<Self> = Vec::new();
            for (key, value) in candidates {
                let trimmed = key.strip_suffix(&divider).unwrap_or(&key);
                if pattern.is_match(trimmed) {
                    let suffix = if key.ends_with(&divider) {
                        &divider
                    } else {
                        ""
                    };
                    let entered = entered.clone();
                    let trimmed = trimmed.to_owned();
                    let suffix = suffix.to_owned();
                    let f_clone = f.clone();
                    actions.push(Self::callback(move |c| {
                        let mut invoked = f_clone(&trimmed, &entered).invoke(c).suffix(&suffix);
                        for value in &mut invoked.values {
                            value.display.push_str(&suffix);
                        }
                        invoked.to_action()
                    }));
                } else {
                    actions.push(Self::raw(vec![crate::value::RawValue {
                        value: key.clone(),
                        display: key,
                        ..value
                    }]));
                }
            }
            let action = crate::action::batch::batch(actions)
                .invoke(&context)
                .prefix(&prefix)
                .to_action();
            let mut meta = action.meta.clone();
            meta.merge(&invoked.meta);
            // Go's MultiPartsP wraps `ActionMultiParts`, whose tail adds the
            // divider's last character (`*` when empty) to the nospace matcher
            meta = meta.nospace([divider.chars().last().unwrap_or('*')]);
            Self { meta, ..action }
        })
    }

    /// Parse export JSON back into an action (port of `ActionImport`).
    pub fn import(output: &str) -> Self {
        match crate::export::Export::from_json(output) {
            Some(export) => Self {
                meta: export.meta.to_meta(),
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

    /// Add query metadata via a function (experimental, Go
    /// `Action.QueryF`). The current value rides the query as a
    /// `C_VALUE` opt when non-empty.
    pub fn query_f<F>(self, f: F) -> Self
    where
        F: Fn(&Context) -> Result<crate::meta::Query, String> + Send + Sync + 'static,
    {
        Self::callback(move |c| {
            let mut query = match f(c) {
                Ok(query) => query,
                Err(error) => return Self::message(error),
            };
            if !c.value.is_empty() {
                query.opts.push("C_VALUE".to_owned());
                query.opts.push(c.value.clone());
            }
            let mut invoked = self.invoke(c);
            invoked.meta.queries.push(query);
            invoked.to_action()
        })
    }

    /// Change the working directory for the invocation.
    ///
    /// Relative directories resolve against the context directory and the
    /// target must be an existing directory (Go `Action.Chdir` via
    /// `Context.Abs`).
    pub fn chdir(self, dir: impl Into<String>) -> Self {
        let dir = std::sync::Arc::new(dir.into());
        self.chdir_f(move |_| (*dir).clone())
    }

    /// Change the working directory for the invocation via a function.
    pub fn chdir_f<F>(self, f: F) -> Self
    where
        F: Fn(&Context) -> String + Send + Sync + 'static,
    {
        Self::callback(move |c| {
            let abs = match c.abs(&f(c)) {
                Ok(abs) => abs,
                Err(err) => return Self::message(err),
            };
            let path = std::path::PathBuf::from(&abs);
            match std::fs::metadata(&path) {
                Err(err) => Self::message(err.to_string()),
                Ok(metadata) if !metadata.is_dir() => {
                    Self::message(format!("not a directory: {abs}"))
                }
                Ok(_) => {
                    let mut context = c.clone();
                    context.dir = path;
                    self.invoke(&context).to_action()
                }
            }
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
            let Some(cache_file) = crate::cache::file(c, &site, &keys) else {
                return self.invoke(c).to_action();
            };
            if let Some(cached) = crate::cache::load(&cache_file, timeout_secs) {
                return Action {
                    meta: cached.meta.to_meta(),
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
