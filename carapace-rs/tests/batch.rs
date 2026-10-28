use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use carapace_rs::{Action, Context, batch};

#[test]
fn batch_merges_all_results() {
    let a = Action::values(["1", "2"]);
    let b = Action::values(["2", "3"]);
    let c = Action::message("batched");
    let invoked = batch(vec![a, b, c]).invoke(&Context::default());
    let values: Vec<String> = invoked
        .to_export()
        .values
        .iter()
        .map(|v| v.value.clone())
        .collect();
    assert_eq!(values, vec!["1", "2", "3"]);
    assert_eq!(invoked.to_export().meta.messages, vec!["batched"]);
}

#[test]
fn batch_runs_callbacks_concurrently() {
    // the barrier deadlocks unless all three callbacks run concurrently;
    // invoking on a side thread bounds the wait so a regression fails the
    // test instead of hanging it (the thread is abandoned on timeout)
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::scope(|scope| {
        scope.spawn(move || {
            let gate = Arc::new(std::sync::Barrier::new(3));
            let counter = Arc::new(AtomicUsize::new(0));
            let actions: Vec<Action> = (0..3)
                .map(|index| {
                    let gate = gate.clone();
                    let counter = counter.clone();
                    Action::callback(move |_| {
                        counter.fetch_add(1, Ordering::SeqCst);
                        gate.wait(); // blocks unless all three run concurrently
                        Action::values([format!("x{index}")])
                    })
                })
                .collect();
            let invoked = batch(actions).invoke(&Context::default());
            sender
                .send(invoked.to_export().values.len())
                .expect("test thread still waiting");
        });
        let count = receiver
            .recv_timeout(std::time::Duration::from_secs(10))
            .expect("batch must run its callbacks concurrently");
        assert_eq!(count, 3);
    });
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
    let values: Vec<String> = invoked
        .to_export()
        .values
        .iter()
        .map(|v| v.value.clone())
        .collect();
    assert_eq!(values, vec!["A", "B"]);
}

#[test]
fn batch_none() {
    let invoked = batch([]).invoke(&Context::default());
    assert!(invoked.to_export().values.is_empty());
    assert!(invoked.to_export().meta.messages.is_empty());
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
        .to_export()
        .values
        .iter()
        .cloned()
        .map(|v| v.value)
        .collect();
    assert_eq!(values, vec!["A", "B", "C", "D"]);
}
