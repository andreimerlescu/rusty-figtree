use figtree::Tree;
use std::io::Write;
use tempfile::NamedTempFile;

// ── Environment variable source ───────────────────────────────────────────────

#[test]
fn env_source_overrides_default() {
    std::env::set_var("FIGTREE_TEST_WORKERS_ABC", "32");
    let mut tree = Tree::new();
    tree.new_int("FIGTREE_TEST_WORKERS_ABC", 4, "");
    tree.parse().unwrap();
    assert_eq!(tree.integer("FIGTREE_TEST_WORKERS_ABC").unwrap(), 32);
    std::env::remove_var("FIGTREE_TEST_WORKERS_ABC");
}

#[test]
fn env_source_parses_bool() {
    std::env::set_var("FIGTREE_TEST_DEBUG_XYZ", "true");
    let mut tree = Tree::new();
    tree.new_bool("FIGTREE_TEST_DEBUG_XYZ", false, "");
    tree.parse().unwrap();
    assert_eq!(tree.boolean("FIGTREE_TEST_DEBUG_XYZ").unwrap(), true);
    std::env::remove_var("FIGTREE_TEST_DEBUG_XYZ");
}

#[test]
fn env_source_parses_int128_beyond_i64_max() {
    let big: i128 = i64::MAX as i128 + 1;
    std::env::set_var("FIGTREE_TEST_BIGNUM_XYZ", big.to_string());
    let mut tree = Tree::new();
    tree.new_int128("FIGTREE_TEST_BIGNUM_XYZ", 0, "");
    tree.parse().unwrap();
    assert_eq!(tree.int128("FIGTREE_TEST_BIGNUM_XYZ").unwrap(), big);
    std::env::remove_var("FIGTREE_TEST_BIGNUM_XYZ");
}

#[test]
fn env_source_absent_falls_through_to_default() {
    std::env::remove_var("FIGTREE_TEST_DEFINITELY_NOT_SET_XYZ");
    let mut tree = Tree::new();
    tree.new_int("FIGTREE_TEST_DEFINITELY_NOT_SET_XYZ", 42, "");
    tree.parse().unwrap();
    assert_eq!(tree.integer("FIGTREE_TEST_DEFINITELY_NOT_SET_XYZ").unwrap(), 42);
}

// ── YAML source ───────────────────────────────────────────────────────────────

#[cfg(feature = "yaml")]
#[test]
fn yaml_source_provides_values() {
    let mut f = NamedTempFile::new().unwrap();
    writeln!(f, "workers: 16").unwrap();
    writeln!(f, "debug: true").unwrap();
    writeln!(f, "endpoint: https://api.example.com").unwrap();

    let mut tree = Tree::new();
    tree.new_int("workers",     4,                        "")
        .new_bool("debug",      false,                    "")
        .new_string("endpoint", "http://localhost",       "");
    tree.with_yaml_file(f.path().to_str().unwrap()).unwrap();
    tree.parse().unwrap();

    assert_eq!(tree.integer("workers").unwrap(), 16);
    assert_eq!(tree.boolean("debug").unwrap(),   true);
    assert_eq!(tree.string("endpoint").unwrap(), "https://api.example.com");
}

#[cfg(feature = "yaml")]
#[test]
fn yaml_source_falls_through_for_missing_keys() {
    let mut f = NamedTempFile::new().unwrap();
    writeln!(f, "workers: 16").unwrap();

    let mut tree = Tree::new();
    tree.new_int("workers",     4,                  "")
        .new_string("endpoint", "http://localhost", "");
    tree.with_yaml_file(f.path().to_str().unwrap()).unwrap();
    tree.parse().unwrap();

    assert_eq!(tree.integer("workers").unwrap(),  16);
    assert_eq!(tree.string("endpoint").unwrap(),  "http://localhost");
}

#[cfg(feature = "yaml")]
#[test]
fn yaml_document_start_marker_handled() {
    let mut f = NamedTempFile::new().unwrap();
    writeln!(f, "---").unwrap();
    writeln!(f, "workers: 8").unwrap();

    let mut tree = Tree::new();
    tree.new_int("workers", 4, "");
    tree.with_yaml_file(f.path().to_str().unwrap()).unwrap();
    tree.parse().unwrap();

    assert_eq!(tree.integer("workers").unwrap(), 8);
}

#[cfg(feature = "yaml")]
#[test]
fn yaml_yml_extension_accepted() {
    use tempfile::Builder;

    let mut f = Builder::new().suffix(".yml").tempfile().unwrap();
    writeln!(f, "workers: 12").unwrap();

    let mut tree = Tree::new();
    tree.new_int("workers", 4, "");
    tree.with_yaml_file(f.path().to_str().unwrap()).unwrap();
    tree.parse().unwrap();

    assert_eq!(tree.integer("workers").unwrap(), 12);
}

// ── JSON source ───────────────────────────────────────────────────────────────

#[cfg(feature = "json")]
#[test]
fn json_source_provides_values() {
    let mut f = NamedTempFile::new().unwrap();
    writeln!(f, r#"{{"workers": 8, "debug": true, "threshold": 0.9}}"#).unwrap();

    let mut tree = Tree::new();
    tree.new_int("workers",       4,     "")
        .new_bool("debug",        false, "")
        .new_float64("threshold", 0.5,   "");
    tree.with_json_file(f.path().to_str().unwrap()).unwrap();
    tree.parse().unwrap();

    assert_eq!(tree.integer("workers").unwrap(),   8);
    assert_eq!(tree.boolean("debug").unwrap(),     true);
    assert_eq!(tree.float64("threshold").unwrap(), 0.9);
}

// ── TOML source ───────────────────────────────────────────────────────────────

#[cfg(feature = "toml-fmt")]
#[test]
fn toml_source_provides_values() {
    let mut f = NamedTempFile::new().unwrap();
    writeln!(f, "workers = 20").unwrap();
    writeln!(f, "debug = true").unwrap();
    writeln!(f, r#"endpoint = "https://api.example.com""#).unwrap();

    let mut tree = Tree::new();
    tree.new_int("workers",     4,                  "")
        .new_bool("debug",      false,              "")
        .new_string("endpoint", "http://localhost", "");
    tree.with_toml_file(f.path().to_str().unwrap()).unwrap();
    tree.parse().unwrap();

    assert_eq!(tree.integer("workers").unwrap(),  20);
    assert_eq!(tree.boolean("debug").unwrap(),    true);
    assert_eq!(tree.string("endpoint").unwrap(),  "https://api.example.com");
}

#[cfg(feature = "toml-fmt")]
#[test]
fn toml_nested_key_via_dotted_access() {
    let mut f = NamedTempFile::new().unwrap();
    writeln!(f, "[database]").unwrap();
    writeln!(f, "host = \"db.example.com\"").unwrap();
    writeln!(f, "port = 5432").unwrap();

    let mut tree = Tree::new();
    tree.new_string("database.host", "localhost", "")
        .new_int("database.port",    5432,        "");
    tree.with_toml_file(f.path().to_str().unwrap()).unwrap();
    tree.parse().unwrap();

    assert_eq!(tree.string("database.host").unwrap(),  "db.example.com");
    assert_eq!(tree.integer("database.port").unwrap(), 5432);
}

// ── Multiple file sources ─────────────────────────────────────────────────────

#[cfg(feature = "yaml")]
#[test]
fn first_file_source_wins_over_second() {
    let mut base = NamedTempFile::new().unwrap();
    writeln!(base, "workers: 10").unwrap();

    let mut local = NamedTempFile::new().unwrap();
    writeln!(local, "workers: 20").unwrap();

    let mut tree = Tree::new();
    tree.new_int("workers", 4, "");
    tree.with_yaml_file(base.path().to_str().unwrap()).unwrap();
    tree.with_yaml_file(local.path().to_str().unwrap()).unwrap();
    tree.parse().unwrap();

    assert_eq!(tree.integer("workers").unwrap(), 10);
}

#[cfg(feature = "yaml")]
#[test]
fn second_file_used_when_first_lacks_key() {
    let mut base = NamedTempFile::new().unwrap();
    writeln!(base, "workers: 10").unwrap();

    let mut local = NamedTempFile::new().unwrap();
    writeln!(local, "endpoint: https://local.example.com").unwrap();

    let mut tree = Tree::new();
    tree.new_int("workers",     4,                  "")
        .new_string("endpoint", "http://localhost", "");
    tree.with_yaml_file(base.path().to_str().unwrap()).unwrap();
    tree.with_yaml_file(local.path().to_str().unwrap()).unwrap();
    tree.parse().unwrap();

    assert_eq!(tree.integer("workers").unwrap(), 10);
    assert_eq!(tree.string("endpoint").unwrap(), "https://local.example.com");
}

// ── PEMDAS: env beats file beats default ──────────────────────────────────────

#[cfg(feature = "yaml")]
#[test]
fn env_beats_file_beats_default() {
    let env_key = "FIGTREE_TEST_PEMDAS_WORKERS";

    let mut f = NamedTempFile::new().unwrap();
    writeln!(f, "{}: 30", env_key).unwrap();

    std::env::set_var(env_key, "50");

    let mut tree = Tree::new();
    tree.new_int(env_key, 10, "");
    tree.with_yaml_file(f.path().to_str().unwrap()).unwrap();
    tree.parse().unwrap();

    assert_eq!(tree.integer(env_key).unwrap(), 50);

    std::env::remove_var(env_key);
}

#[cfg(feature = "yaml")]
#[test]
fn file_beats_default_when_no_env() {
    let key = "FIGTREE_TEST_PEMDAS_FILE_KEY";
    std::env::remove_var(key);

    let mut f = NamedTempFile::new().unwrap();
    writeln!(f, "{}: 30", key).unwrap();

    let mut tree = Tree::new();
    tree.new_int(key, 10, "");
    tree.with_yaml_file(f.path().to_str().unwrap()).unwrap();
    tree.parse().unwrap();

    assert_eq!(tree.integer(key).unwrap(), 30);
}

// ── Dotenv source ─────────────────────────────────────────────────────────────

#[cfg(feature = "dotenv")]
#[test]
fn dotenv_source_provides_values() {
    let mut f = NamedTempFile::new().unwrap();
    writeln!(f, "DOTENV_WORKERS=24").unwrap();
    writeln!(f, "DOTENV_DEBUG=true").unwrap();

    let mut tree = Tree::new();
    tree.new_int("DOTENV_WORKERS", 4,     "")
        .new_bool("DOTENV_DEBUG",  false, "");
    tree.with_dotenv_file(f.path().to_str().unwrap()).unwrap();
    tree.parse().unwrap();

    assert_eq!(tree.integer("DOTENV_WORKERS").unwrap(), 24);
    assert_eq!(tree.boolean("DOTENV_DEBUG").unwrap(),   true);
}

#[cfg(feature = "dotenv")]
#[test]
fn dotenv_does_not_leak_into_process_environment() {
    let isolation_key = "FIGTREE_DOTENV_ISOLATION_TEST_XYZ_789";
    let mut f = NamedTempFile::new().unwrap();
    writeln!(f, "{}=leaked_value", isolation_key).unwrap();

    let mut tree = Tree::new();
    tree.new_string(isolation_key, "default", "");
    tree.with_dotenv_file(f.path().to_str().unwrap()).unwrap();
    tree.parse().unwrap();

    assert_eq!(tree.string(isolation_key).unwrap(), "leaked_value");
    assert!(std::env::var(isolation_key).is_err());
}
