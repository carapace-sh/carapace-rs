use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use carapace_core::{Action, Context, batch};

#[test]
fn batch_merges_all_results() {
    let a = Action::values(["1", "2"]);
    let b = Action::values(["2", "3"]);
    let c = Action::message("batched");
    let invoked = batch(vec![a, b, c]).invoke(&Context::default());
    let values: Vec<String> = invoked.values.iter().map(|v| v.value.clone()).collect();
    assert_eq!(values, vec!["1", "2", "3"]);
    assert_eq!(invoked.meta.messages, vec!["batched"]);
}

#[test]
fn batch_runs_callbacks_concurrently() {
    let gate = Arc::new(std::sync::Barrier::new(3));
    let counter = Arc::new(AtomicUsize::new(0));
    let actions: Vec<Action> = (0..3)
        .map(|index| {
            let gate = gate.clone();
            let counter = counter.clone();
            Action::callback(move |_| {
                counter.fetch_add(1, Ordering::SeqCst);
                gate.wait(); // deadlocks unless all three run concurrently
                Action::values([format!("x{index}")])
            })
        })
        .collect();
    let invoked = batch(actions).invoke(&Context::default());
    assert_eq!(invoked.values.len(), 3);
}

#[test]
fn batch_is_lazy() {
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = counter.clone();
    let action = batch(vec![Action::callback(move |_| {
        counter_clone.fetch_add(1, Ordering::SeqCst);
        Action::default()
    })]);
    assert_eq!(counter.load(Ordering::SeqCst), 0);
    let _ = action.invoke(&Context::default());
    assert_eq!(counter.load(Ordering::SeqCst), 1);
}

#[test]
fn batch_single() {
    let invoked = batch(vec![Action::values(["A", "B"])]).invoke(&Context::default());
    let values: Vec<String> = invoked.values.iter().map(|v| v.value.clone()).collect();
    assert_eq!(values, vec!["A", "B"]);
}

#[test]
fn batch_none() {
    let invoked = batch(Vec::<Action>::new()).invoke(&Context::default());
    assert!(invoked.values.is_empty());
    assert!(invoked.meta.messages.is_empty());
}

#[test]
fn batch_to_action_roundtrip() {
    // Go TestBatchToA: converting the batch back to a lazy action and
    // invoking later yields the same merged result
    let action = batch(vec![
        Action::values(["A", "B"]),
        Action::values(["B", "C"]),
        Action::values(["C", "D"]),
    ]);
    let values: Vec<String> = action
        .invoke(&Context::default())
        .values
        .into_iter()
        .map(|v| v.value)
        .collect();
    assert_eq!(values, vec!["A", "B", "C", "D"]);
}
