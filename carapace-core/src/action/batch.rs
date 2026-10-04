//! Parallel invocation of multiple actions (port of Go `Batch`).

use std::sync::Arc;

use super::{Action, InvokedAction, merge_all};
use crate::context::Context;

/// Invoke all actions and merge their results (parallelized across threads).
pub fn batch(actions: Vec<Action>) -> Action {
    let actions: Arc<[Action]> = actions.into();
    Action::callback(move |context: &Context| {
        let invoked: Vec<InvokedAction> = std::thread::scope(|scope| {
            let handles: Vec<_> = actions
                .iter()
                .map(|action| scope.spawn(|| action.invoke(context)))
                .collect();
            handles
                .into_iter()
                .map(|handle| handle.join().unwrap_or_default())
                .collect()
        });
        merge_all(invoked).to_action()
    })
}
