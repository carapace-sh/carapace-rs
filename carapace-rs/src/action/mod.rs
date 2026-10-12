//! The Action API: lazy completion descriptions and their materialized form.

mod batch;
mod constructors;
mod invoked;
mod modifiers;

use std::sync::Arc;

use crate::context::Context;
use crate::meta::Meta;
use crate::value::RawValues;

pub use batch::batch;
pub use invoked::{InvokedAction, merge_all};

/// Callback producing an [`Action`] at invocation time.
pub type ActionCallback = Arc<dyn Fn(&Context) -> Action + Send + Sync>;

/// A lazy description of completions.
///
/// Actions are constructed at definition time and do no work until
/// [`Action::invoke`] materializes them into an [`InvokedAction`].
#[derive(Clone, Default)]
pub struct Action {
    pub meta: Meta,
    kind: ActionKind,
}

#[derive(Clone, Default)]
enum ActionKind {
    #[default]
    Empty,
    /// Pre-computed values; no callback.
    Raw(RawValues),
    /// Lazy callback producing another action.
    Callback(ActionCallback),
}

impl std::fmt::Debug for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Action")
            .field("meta", &self.meta)
            .field(
                "kind",
                &match &self.kind {
                    ActionKind::Empty => "empty",
                    ActionKind::Raw(values) => return write!(f, "Raw({} values)", values.len()),
                    ActionKind::Callback(_) => "callback",
                },
            )
            .finish()
    }
}

impl Action {
    /// Create an action from a callback.
    pub fn callback<F>(f: F) -> Self
    where
        F: Fn(&Context) -> Action + Send + Sync + 'static,
    {
        Self {
            kind: ActionKind::Callback(Arc::new(f)),
            ..Default::default()
        }
    }

    /// Create an action producing static values.
    pub fn raw(values: RawValues) -> Self {
        Self {
            kind: ActionKind::Raw(values),
            ..Default::default()
        }
    }

    /// Whether this action carries a callback.
    pub fn is_callback(&self) -> bool {
        matches!(self.kind, ActionKind::Callback(_))
    }

    /// Materialize the action: run the callback chain and collect values.
    pub fn invoke(&self, context: &Context) -> InvokedAction {
        let mut meta = self.meta.clone();
        let values = match &self.kind {
            ActionKind::Empty => RawValues::new(),
            ActionKind::Raw(values) => values.clone(),
            ActionKind::Callback(callback) => {
                let inner = callback(context);
                let invoked = inner.invoke(context);
                meta.merge(&invoked.meta);
                invoked.values
            }
        };
        InvokedAction { meta, values }
    }

    /// Internal helper used by modifiers: wrap a transform of the invoked form.
    pub(crate) fn map_invoked<F>(self, f: F) -> Self
    where
        F: Fn(&Context, InvokedAction) -> InvokedAction + Send + Sync + 'static,
    {
        Self::callback(move |c| f(c, self.invoke(c)).to_action())
    }
}

impl From<InvokedAction> for Action {
    fn from(invoked: InvokedAction) -> Self {
        Self {
            meta: invoked.meta,
            kind: ActionKind::Raw(invoked.values),
        }
    }
}
