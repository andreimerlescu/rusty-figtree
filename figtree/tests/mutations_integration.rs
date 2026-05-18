//! Integration tests for the mutation channel end-to-end.
//!
//! Tests that mutations are emitted correctly from parse(), store(),
//! and pollinate(), that they carry the right values and sources,
//! that the typed receiver converts them correctly, and that curse()
//! and recall() work as documented.

use figtree::{
    FigSource, FigValue, Options, Tree,
    mutation::mutation_channel,
};
use std::time::Duration;

// ── Basic emission ────────────────────────────────────────────────────────────

#[test]
fn parse_emits_first_resolution_mutation() {
    let mut tree = Tree::grow();
    let rx       = tree.mutations().unwrap();

    tree.new_int("workers", 4, "");
    tree.parse().unwrap();

    let m = rx.try_recv().unwrap();
    assert_eq!(m.key, "workers");
    assert_eq!(m.new, FigValue::Int(4));
    assert!(m.old.is_none()); // first resolution
}

#[test]
fn store_emits_changed_mutation() {
    let mut tree = Tree::grow();
    let rx       = tree.mutations().unwrap();

    tree.new_int("workers", 4, "");
    tree.parse().unwrap();
    // drain the parse mutation
    let _ = rx.try_recv();

    tree.store("workers", FigValue::Int(16)).unwrap();

    let m = rx.try_recv().unwrap();
    assert_eq!(m.key, "workers");
    assert_eq!(m.new, FigValue::Int(16));
    assert_eq!(m.old, Some(FigValue::Int(4)));
}

#[test]
fn multiple_keys_each_emit_their_own_mutation() {
    let mut tree = Tree::grow();
    let rx       = tree.mutations().unwrap();

    tree.new_int("workers",    4,     "")
        .new_bool("debug",     false, "")
        .new_string("host",    "x",   "");
    tree.parse().unwrap();

    let mut keys = Vec::new();
    while let Some(m) = rx.try_recv() {
        keys.push(m.key.clone());
    }
    keys.sort();

    assert!(keys.contains(&"workers".to_string()));
    assert!(keys.contains(&"debug".to_string()));
    assert!(keys.contains(&"host".to_string()));
}

// ── Mutation carries correct source ──────────────────────────────────────────

#[test]
fn mutation_source_is_default_when_no_override() {
    let mut tree = Tree::grow();
    let rx       = tree.mutations().unwrap();

    tree.new_int("workers", 4, "");
    tree.parse().unwrap();

    let m = rx.try_recv().unwrap();
    assert_eq!(m.source, FigSource::Default);
}

#[test]
fn mutation_source_is_environment_when_env_var_set() {
    let key = "FIGTREE_TEST_MUTATION_SOURCE_XYZ";
    std::env::set_var(key, "32");

    let mut tree = Tree::grow();
    let rx       = tree.mutations().unwrap();

    tree.new_int(key, 4, "");
    tree.parse().unwrap();

    let m = rx.try_recv().unwrap();
    assert!(matches!(m.source, FigSource::Environment(_)));
    assert_eq!(m.new, FigValue::Int(32));

    std::env::remove_var(key);
}

#[test]
fn mutation_source_is_programmatic_on_store() {
    let mut tree = Tree::grow();
    let rx       = tree.mutations().unwrap();

    tree.new_int("workers", 4, "");
    tree.parse().unwrap();
    let _ = rx.try_recv(); // drain parse mutation

    tree.store("workers", FigValue::Int(8)).unwrap();

    let m = rx.try_recv().unwrap();
    assert_eq!(m.source, FigSource::Programmatic);
}

// ── No mutation tracking without grow() ──────────────────────────────────────

#[test]
fn new_tree_has_no_mutation_channel() {
    let mut tree = Tree::new();
    assert!(tree.mutations().is_none());
}

#[test]
fn store_on_non_tracking_tree_does_not_panic() {
    let mut tree = Tree::new();
    tree.new_int("workers", 4, "");
    tree.parse().unwrap();
    // store should succeed silently even with no tracking
    tree.store("workers", FigValue::Int(8)).unwrap();
    assert_eq!(tree.integer("workers").unwrap(), 8);
}

// ── curse() and recall() ──────────────────────────────────────────────────────

#[test]
fn curse_closes_mutation_channel() {
    let mut tree = Tree::grow();
    let rx       = tree.mutations().unwrap();

    tree.curse();

    // channel is closed — recv returns None
    assert!(rx.recv().is_none());
}

#[test]
fn recall_reopens_mutation_channel() {
    let mut tree = Tree::grow();
    let _rx      = tree.mutations().unwrap();

    tree.curse();
    assert!(tree.mutation_tx.is_none());

    tree.recall();
    assert!(tree.mutation_tx.is_some());
}

#[test]
fn store_after_curse_does_not_emit() {
    let mut tree = Tree::grow();
    let rx       = tree.mutations().unwrap();

    tree.new_int("workers", 4, "");
    tree.parse().unwrap();

    tree.curse();
    tree.store("workers", FigValue::Int(8)).unwrap();

    // channel is closed, rx.recv() returns None
    // the mutation from parse may or may not have been received
    // but no store mutation was sent since curse() dropped the tx
    assert!(rx.recv().is_none());
}

#[test]
fn store_after_recall_emits_to_new_receiver() {
    let mut tree = Tree::grow();
    let _old_rx  = tree.mutations().unwrap();

    tree.new_int("workers", 4, "");
    tree.parse().unwrap();

    tree.curse();
    tree.recall();

    // get a new receiver after recall
    let new_rx = tree.mutations().unwrap();

    tree.store("workers", FigValue::Int(16)).unwrap();

    let m = new_rx.try_recv().unwrap();
    assert_eq!(m.key, "workers");
    assert_eq!(m.new, FigValue::Int(16));
}

// ── iter() on MutationReceiver ────────────────────────────────────────────────

#[test]
fn mutation_iter_yields_all_emitted_mutations() {
    let mut tree = Tree::grow();
    let rx       = tree.mutations().unwrap();

    tree.new_int("workers", 4, "");
    tree.new_bool("debug",  false, "");
    tree.parse().unwrap();

    tree.store("workers", FigValue::Int(8)).unwrap();
    tree.store("workers", FigValue::Int(16)).unwrap();

    // drop tx so iter() terminates
    tree.curse();

    let mutations: Vec<_> = rx.iter().collect();
    // at minimum: parse(workers), parse(debug), store(8), store(16)
    assert!(mutations.len() >= 4);
}

// ── Pollination emits mutations ───────────────────────────────────────────────

#[test]
fn pollinate_emits_mutation_when_env_changes() {
    let key = "FIGTREE_TEST_POLLINATE_MUTATION_XYZ";
    std::env::set_var(key, "4");

    let mut tree = Tree::with(Options {
        tracking:  true,
        pollinate: true,
        ..Options::default()
    });
    let rx = tree.mutations().unwrap();

    tree.new_int(key, 0, "");
    tree.parse().unwrap();
    let _ = rx.try_recv(); // drain parse mutation

    // change the env var
    std::env::set_var(key, "99");
    tree.pollinate().unwrap();

    let m = rx.try_recv().unwrap();
    assert_eq!(m.key, key);
    assert_eq!(m.new, FigValue::Int(99));
    assert_eq!(m.old, Some(FigValue::Int(4)));

    std::env::remove_var(key);
}

#[test]
fn pollinate_does_not_emit_when_value_unchanged() {
    let key = "FIGTREE_TEST_POLLINATE_NO_CHANGE_XYZ";
    std::env::set_var(key, "4");

    let mut tree = Tree::with(Options {
        tracking:  true,
        pollinate: true,
        ..Options::default()
    });
    let rx = tree.mutations().unwrap();

    tree.new_int(key, 0, "");
    tree.parse().unwrap();
    let _ = rx.try_recv(); // drain parse mutation

    // pollinate without changing the env var
    tree.pollinate().unwrap();

    // no new mutation emitted
    assert!(rx.try_recv().is_none());

    std::env::remove_var(key);
}

#[test]
fn pollinate_is_noop_when_disabled() {
    let key = "FIGTREE_TEST_POLLINATE_NOOP_XYZ";
    std::env::set_var(key, "4");

    let mut tree = Tree::grow(); // pollinate = false by default
    let rx       = tree.mutations().unwrap();

    tree.new_int(key, 0, "");
    tree.parse().unwrap();
    let _ = rx.try_recv(); // drain parse mutation

    std::env::set_var(key, "99");
    tree.pollinate().unwrap(); // should be a no-op

    // no mutation emitted because pollinate is disabled
    assert!(rx.try_recv().is_none());

    std::env::remove_var(key);
}

// ── Duration mutations ────────────────────────────────────────────────────────

#[test]
fn duration_field_emits_mutation_on_store() {
    let mut tree = Tree::grow();
    let rx       = tree.mutations().unwrap();

    tree.new_duration("timeout", Duration::from_secs(30), "");
    tree.parse().unwrap();
    let _ = rx.try_recv(); // drain parse mutation

    tree.store("timeout", FigValue::Duration(Duration::from_secs(60))).unwrap();

    let m = rx.try_recv().unwrap();
    assert_eq!(m.key, "timeout");
    assert_eq!(m.new, FigValue::Duration(Duration::from_secs(60)));
    assert_eq!(m.old, Some(FigValue::Duration(Duration::from_secs(30))));
}

// ── int128 mutations ──────────────────────────────────────────────────────────

#[test]
fn int128_field_emits_correct_typed_mutation() {
    let big: i128 = i64::MAX as i128 + 1;
    let mut tree  = Tree::grow();
    let rx        = tree.mutations().unwrap();

    tree.new_int128("huge_id", 0, "");
    tree.parse().unwrap();
    let _ = rx.try_recv(); // drain parse mutation

    tree.store("huge_id", FigValue::Int128(big)).unwrap();

    let m = rx.try_recv().unwrap();
    assert_eq!(m.key, "huge_id");
    assert_eq!(m.new, FigValue::Int128(big));
    assert_eq!(m.old, Some(FigValue::Int128(0)));
}
