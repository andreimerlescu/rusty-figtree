use figtree::{FigtreeError, FigValue, Tree};

// ── Construction ──────────────────────────────────────────────────────────────

#[test]
fn tree_new_starts_empty_and_unresolved() {
    let tree = Tree::new();
    assert!(tree.is_empty());
    assert!(!tree.is_resolved());
}

#[test]
fn tree_grow_enables_tracking() {
    let mut tree = Tree::grow();
    assert!(tree.mutations().is_some());
}

// ── Full lifecycle: register → parse → read ───────────────────────────────────

#[test]
fn full_lifecycle_all_scalar_types() {
    let big: i128 = i64::MAX as i128 + 1;
    let mut tree  = Tree::new();

    tree.new_string("endpoint",  "http://localhost", "api endpoint")
        .new_int("workers",       4,                  "worker count")
        .new_int64("big_int",     i64::MAX,            "large integer")
        .new_int128("huge",       big,                 "very large integer")
        .new_float64("threshold", 0.75,                "match threshold")
        .new_bool("debug",        false,               "debug mode");

    tree.parse().unwrap();

    assert_eq!(tree.string("endpoint").unwrap(),   "http://localhost");
    assert_eq!(tree.integer("workers").unwrap(),   4);
    assert_eq!(tree.int64("big_int").unwrap(),     i64::MAX);
    assert_eq!(tree.int128("huge").unwrap(),       big);
    assert_eq!(tree.float64("threshold").unwrap(), 0.75);
    assert_eq!(tree.boolean("debug").unwrap(),     false);
}

#[test]
fn full_lifecycle_list_types() {
    let mut tree = Tree::new();
    tree.new_list_string("tags",    vec!["a".into(), "b".into()], "tags")
        .new_list_int("ports",      vec![8080, 9090],             "ports")
        .new_list_int64("ids",      vec![i64::MAX],               "ids")
        .new_list_int128("big_ids", vec![i64::MAX as i128 + 1],   "big ids")
        .new_list_bool("flags",     vec![true, false],            "flags");

    tree.parse().unwrap();

    assert_eq!(tree.list_string("tags").unwrap(),  vec!["a".to_string(), "b".to_string()]);
    assert_eq!(tree.list_int("ports").unwrap(),    vec![8080i32, 9090i32]);
}

#[test]
fn full_lifecycle_map_types() {
    use std::collections::HashMap;

    let mut metadata = HashMap::new();
    metadata.insert("env".to_string(),     "prod".to_string());
    metadata.insert("version".to_string(), "1.0".to_string());

    let mut tree = Tree::new();
    tree.new_map_string("metadata", metadata.clone(), "deployment metadata");
    tree.parse().unwrap();

    let result = tree.map_string("metadata").unwrap();
    assert_eq!(result.get("env"),     Some(&"prod".to_string()));
    assert_eq!(result.get("version"), Some(&"1.0".to_string()));
}

// ── Required keys ─────────────────────────────────────────────────────────────

#[test]
fn required_key_missing_returns_error() {
    let mut tree = Tree::new();
    tree.new_string_required("api_key", "required api key");
    assert!(matches!(
        tree.parse(),
        Err(FigtreeError::MissingRequired(_))
    ));
}

#[test]
fn required_key_provided_via_env_succeeds() {
    std::env::set_var("FIGTREE_TEST_API_KEY_XYZ", "secret-value");
    let mut tree = Tree::new();
    tree.new_string_required("FIGTREE_TEST_API_KEY_XYZ", "api key from env");
    tree.parse().unwrap();
    assert_eq!(
        tree.string("FIGTREE_TEST_API_KEY_XYZ").unwrap(),
        "secret-value"
    );
    std::env::remove_var("FIGTREE_TEST_API_KEY_XYZ");
}

// ── Store ─────────────────────────────────────────────────────────────────────

#[test]
fn store_updates_value_and_is_readable() {
    let mut tree = Tree::new();
    tree.new_int("workers", 4, "");
    tree.parse().unwrap();
    tree.store("workers", FigValue::Int(16)).unwrap();
    assert_eq!(tree.integer("workers").unwrap(), 16);
}

#[test]
fn store_type_mismatch_returns_error_and_preserves_value() {
    let mut tree = Tree::new();
    tree.new_int("workers", 4, "");
    tree.parse().unwrap();
    let result = tree.store("workers", FigValue::String("oops".into()));
    assert!(matches!(result, Err(FigtreeError::TypeMismatch { .. })));
    assert_eq!(tree.integer("workers").unwrap(), 4);
}

#[test]
fn store_unknown_key_returns_error() {
    let mut tree = Tree::new();
    assert!(matches!(
        tree.store("nonexistent", FigValue::Int(1)),
        Err(FigtreeError::UnknownKey(_))
    ));
}

// ── Type mismatches ───────────────────────────────────────────────────────────

#[test]
fn getter_type_mismatch_returns_error() {
    let mut tree = Tree::new();
    tree.new_string("endpoint", "http://localhost", "");
    tree.parse().unwrap();
    assert!(matches!(
        tree.integer("endpoint"),
        Err(FigtreeError::TypeMismatch { .. })
    ));
}

#[test]
fn getter_unknown_key_returns_error() {
    let mut tree = Tree::new();
    tree.parse().unwrap();
    assert!(matches!(
        tree.integer("nonexistent"),
        Err(FigtreeError::UnknownKey(_))
    ));
}

// ── Duplicate registration ────────────────────────────────────────────────────

#[test]
fn duplicate_registration_first_wins() {
    let mut tree = Tree::new();
    tree.new_int("workers", 4,  "first");
    tree.new_int("workers", 99, "second");
    tree.parse().unwrap();
    assert_eq!(tree.integer("workers").unwrap(), 4);
}

// ── load() vs parse() ─────────────────────────────────────────────────────────

#[cfg(feature = "cli")]
#[test]
fn load_skips_flag_source_and_preserves_it() {
    use std::collections::HashMap;
    use figtree::sources::CliSource;

    let mut tree = Tree::new();
    tree.new_int("workers", 4, "");

    let mut flags = HashMap::new();
    flags.insert("workers".to_string(), "99".to_string());
    tree.with_flag_source(Box::new(CliSource::new(flags)));

    // load() skips flags — default of 4 should win
    tree.load().unwrap();
    assert_eq!(tree.integer("workers").unwrap(), 4);

    // parse() uses flags — should now be 99
    tree.parse().unwrap();
    assert_eq!(tree.integer("workers").unwrap(), 99);
}

#[cfg(feature = "cli")]
#[test]
fn parse_uses_flag_source() {
    use std::collections::HashMap;
    use figtree::sources::CliSource;

    let mut tree = Tree::new();
    tree.new_int("workers", 4, "");

    let mut flags = HashMap::new();
    flags.insert("workers".to_string(), "99".to_string());
    tree.with_flag_source(Box::new(CliSource::new(flags)));

    tree.parse().unwrap();
    assert_eq!(tree.integer("workers").unwrap(), 99);
}

// ── Diagnostics ───────────────────────────────────────────────────────────────

#[test]
fn usage_contains_all_registered_keys() {
    let mut tree = Tree::new();
    tree.new_int("workers",     4,                  "number of workers")
        .new_string("endpoint", "http://localhost", "api endpoint")
        .new_bool("debug",      false,              "debug mode");
    tree.parse().unwrap();

    let usage = tree.usage();
    assert!(usage.contains("workers"));
    assert!(usage.contains("endpoint"));
    assert!(usage.contains("debug"));
    assert!(usage.contains("number of workers"));
    assert!(usage.contains("api endpoint"));
}

#[test]
fn problems_empty_when_all_clean() {
    let mut tree = Tree::new();
    tree.new_int("workers", 4, "");
    tree.parse().unwrap();
    assert!(tree.problems().is_empty());
}

#[test]
fn history_records_full_lifecycle() {
    let mut tree = Tree::new();
    tree.new_int("workers", 4, "");
    tree.parse().unwrap();
    tree.store("workers", FigValue::Int(8)).unwrap();
    tree.store("workers", FigValue::Int(16)).unwrap();

    let history = tree.history("workers").unwrap();
    assert_eq!(history[0].state_index, 0);
    assert!(history.len() >= 3);
}

#[test]
fn history_log_is_human_readable_multiline() {
    let mut tree = Tree::new();
    tree.new_int("workers", 4, "");
    tree.parse().unwrap();
    tree.store("workers", FigValue::Int(8)).unwrap();

    let log = tree.history_log("workers").unwrap();
    assert!(log.contains("state 0"));
    assert!(log.contains("initialized"));
}

// ── fig() raw access ──────────────────────────────────────────────────────────

#[test]
fn fig_returns_correct_key_and_source_after_parse() {
    let mut tree = Tree::new();
    tree.new_int("workers", 4, "");
    tree.parse().unwrap();

    let fig = tree.fig("workers").unwrap();
    assert_eq!(fig.key, "workers");
    assert!(fig.is_resolved());
}

#[test]
fn fig_returns_none_for_unknown_key() {
    let tree = Tree::new();
    assert!(tree.fig("nonexistent").is_none());
}

// ── int128 boundary ───────────────────────────────────────────────────────────

#[test]
fn int128_beyond_i64_max_round_trips() {
    let big: i128 = i64::MAX as i128 + 999_999;
    let mut tree  = Tree::new();
    tree.new_int128("huge_id", big, "");
    tree.parse().unwrap();
    assert_eq!(tree.int128("huge_id").unwrap(), big);
}

#[test]
fn int128_store_and_read_beyond_i64_max() {
    let mut tree    = Tree::new();
    let start: i128 = 0;
    let end: i128   = i64::MAX as i128 + 1;
    tree.new_int128("counter", start, "");
    tree.parse().unwrap();
    tree.store("counter", FigValue::Int128(end)).unwrap();
    assert_eq!(tree.int128("counter").unwrap(), end);
}
