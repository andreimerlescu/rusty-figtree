use figtree::{
    FigtreeError, FigValue, Tree,
    validators::{
        assure_string_not_empty,
        assure_string_has_prefix,
        assure_string_contains,
        assure_int_positive,
        assure_int_in_range,
        assure_int128_positive,
        assure_duration_positive,
        assure_duration_min,
        assure_list_not_empty,
        assure_map_not_empty,
        assure_map_has_key,
        assure_bool_true,
    },
};
use std::time::Duration;

// ── String validators through Tree ────────────────────────────────────────────

#[test]
fn string_not_empty_passes_on_valid_default() {
    let mut tree = Tree::new();
    tree.new_string("host", "localhost", "");
    tree.with_validator("host", "not_empty", assure_string_not_empty).unwrap();
    assert!(tree.parse().is_ok());
}

#[test]
fn string_not_empty_fails_on_empty_default() {
    let mut tree = Tree::new();
    tree.new_string("host", "", "");
    tree.with_validator("host", "not_empty", assure_string_not_empty).unwrap();
    assert!(matches!(
        tree.parse(),
        Err(FigtreeError::ValidationFailed { .. })
    ));
}

#[test]
fn string_has_prefix_passes() {
    let mut tree = Tree::new();
    tree.new_string("endpoint", "https://api.example.com", "");
    tree.with_validator("endpoint", "https_prefix", assure_string_has_prefix("https")).unwrap();
    assert!(tree.parse().is_ok());
}

#[test]
fn string_has_prefix_fails() {
    let mut tree = Tree::new();
    tree.new_string("endpoint", "ftp://api.example.com", "");
    tree.with_validator("endpoint", "https_prefix", assure_string_has_prefix("https")).unwrap();
    assert!(matches!(
        tree.parse(),
        Err(FigtreeError::ValidationFailed { .. })
    ));
}

#[test]
fn multiple_validators_all_must_pass() {
    let mut tree = Tree::new();
    tree.new_string("endpoint", "https://api.example.com", "");
    tree.with_validator("endpoint", "not_empty", assure_string_not_empty).unwrap();
    tree.with_validator("endpoint", "has_https",  assure_string_has_prefix("https")).unwrap();
    tree.with_validator("endpoint", "has_dot",    assure_string_contains(".")).unwrap();
    assert!(tree.parse().is_ok());
}

#[test]
fn multiple_validators_first_failure_stops_chain() {
    use std::sync::{Arc, Mutex};

    let second_ran = Arc::new(Mutex::new(false));
    let second_ref = second_ran.clone();

    let mut tree = Tree::new();
    tree.new_string("endpoint", "", "");
    tree.with_validator("endpoint", "not_empty", assure_string_not_empty).unwrap();
    tree.with_validator("endpoint", "second", move |_| {
        *second_ref.lock().unwrap() = true;
        Ok(())
    }).unwrap();

    assert!(tree.parse().is_err());
    assert!(!*second_ran.lock().unwrap());
}

#[test]
fn validator_on_store_blocks_invalid_value() {
    let mut tree = Tree::new();
    tree.new_int("workers", 4, "");
    tree.with_validator("workers", "range", assure_int_in_range(1, 64)).unwrap();
    tree.parse().unwrap();

    let result = tree.store("workers", FigValue::Int(0));
    assert!(matches!(result, Err(FigtreeError::ValidationFailed { .. })));
    assert_eq!(tree.integer("workers").unwrap(), 4);
}

#[test]
fn validator_on_store_allows_valid_value() {
    let mut tree = Tree::new();
    tree.new_int("workers", 4, "");
    tree.with_validator("workers", "range", assure_int_in_range(1, 64)).unwrap();
    tree.parse().unwrap();
    tree.store("workers", FigValue::Int(32)).unwrap();
    assert_eq!(tree.integer("workers").unwrap(), 32);
}

// ── Int validators through Tree ───────────────────────────────────────────────

#[test]
fn int_positive_passes() {
    let mut tree = Tree::new();
    tree.new_int("count", 5, "");
    tree.with_validator("count", "positive", assure_int_positive).unwrap();
    assert!(tree.parse().is_ok());
}

#[test]
fn int_positive_fails_on_zero() {
    let mut tree = Tree::new();
    tree.new_int("count", 0, "");
    tree.with_validator("count", "positive", assure_int_positive).unwrap();
    assert!(matches!(
        tree.parse(),
        Err(FigtreeError::ValidationFailed { .. })
    ));
}

#[test]
fn int_in_range_passes_on_boundary() {
    let mut tree = Tree::new();
    tree.new_int("port", 1, "");
    tree.with_validator("port", "range", assure_int_in_range(1, 65535)).unwrap();
    assert!(tree.parse().is_ok());
}

#[test]
fn int128_positive_passes_beyond_i64_max() {
    let big: i128 = i64::MAX as i128 + 1;
    let mut tree  = Tree::new();
    tree.new_int128("id", big, "");
    tree.with_validator("id", "positive", assure_int128_positive).unwrap();
    assert!(tree.parse().is_ok());
}

// ── Duration validators through Tree ─────────────────────────────────────────

#[test]
fn duration_positive_passes() {
    let mut tree = Tree::new();
    tree.new_duration("timeout", Duration::from_secs(30), "");
    tree.with_validator("timeout", "positive", assure_duration_positive).unwrap();
    assert!(tree.parse().is_ok());
}

#[test]
fn duration_min_fails_when_below_minimum() {
    let mut tree = Tree::new();
    tree.new_duration("timeout", Duration::from_secs(1), "");
    tree.with_validator("timeout", "min_5s", assure_duration_min(Duration::from_secs(5))).unwrap();
    assert!(matches!(
        tree.parse(),
        Err(FigtreeError::ValidationFailed { .. })
    ));
}

// ── List validators through Tree ──────────────────────────────────────────────

#[test]
fn list_not_empty_passes() {
    let mut tree = Tree::new();
    tree.new_list_string("servers", vec!["server1".into()], "");
    tree.with_validator("servers", "not_empty", assure_list_not_empty).unwrap();
    assert!(tree.parse().is_ok());
}

#[test]
fn list_not_empty_fails_on_empty_list() {
    let mut tree = Tree::new();
    tree.new_list_string("servers", vec![], "");
    tree.with_validator("servers", "not_empty", assure_list_not_empty).unwrap();
    assert!(matches!(
        tree.parse(),
        Err(FigtreeError::ValidationFailed { .. })
    ));
}

// ── Map validators through Tree ───────────────────────────────────────────────

#[test]
fn map_has_key_passes() {
    use std::collections::HashMap;
    let mut m = HashMap::new();
    m.insert("env".to_string(), "prod".to_string());

    let mut tree = Tree::new();
    tree.new_map_string("metadata", m, "");
    tree.with_validator("metadata", "has_env", assure_map_has_key("env")).unwrap();
    assert!(tree.parse().is_ok());
}

#[test]
fn map_not_empty_fails_on_empty_map() {
    use std::collections::HashMap;
    let mut tree = Tree::new();
    tree.new_map_string("metadata", HashMap::new(), "");
    tree.with_validator("metadata", "not_empty", assure_map_not_empty).unwrap();
    assert!(matches!(
        tree.parse(),
        Err(FigtreeError::ValidationFailed { .. })
    ));
}

// ── Bool validators through Tree ──────────────────────────────────────────────

#[test]
fn bool_true_passes() {
    let mut tree = Tree::new();
    tree.new_bool("enabled", true, "");
    tree.with_validator("enabled", "must_be_true", assure_bool_true).unwrap();
    assert!(tree.parse().is_ok());
}

#[test]
fn bool_true_fails_on_false() {
    let mut tree = Tree::new();
    tree.new_bool("enabled", false, "");
    tree.with_validator("enabled", "must_be_true", assure_bool_true).unwrap();
    assert!(matches!(
        tree.parse(),
        Err(FigtreeError::ValidationFailed { .. })
    ));
}

// ── with_validator unknown key ────────────────────────────────────────────────

#[test]
fn with_validator_unknown_key_returns_error() {
    let mut tree = Tree::new();
    assert!(matches!(
        tree.with_validator("nonexistent", "v", |_| Ok(())),
        Err(FigtreeError::UnknownKey(_))
    ));
}
