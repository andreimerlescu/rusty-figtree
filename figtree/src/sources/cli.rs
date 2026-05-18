#[cfg(feature = "cli")]

use std::collections::HashMap;
use crate::fig::{FigSource, FigValue};
use crate::priority::Source;
use crate::sources::env::parse_env_value;

/// A configuration source backed by parsed clap ArgMatches.
///
/// The CliSource holds a snapshot of all flag values provided on
/// the command line at parse time. It does not interact with clap
/// at resolution time — the ArgMatches are consumed once at
/// construction and stored internally as a flat map of
/// key → raw string value.
///
/// Type conversion from raw string to FigValue uses the same
/// parse_env_value logic as EnvSource, since both are string-origin
/// sources. The hint (the Fig's declared type) drives parsing.
pub struct CliSource {
    values: HashMap<String, String>,
}

impl CliSource {
    /// Creates a CliSource from a map of flag name → raw string value.
    /// Typically constructed by the Tree after calling clap's parse().
    pub fn new(values: HashMap<String, String>) -> Self {
        CliSource { values }
    }

    /// Creates a CliSource from clap's ArgMatches.
    /// Extracts all present string-valued arguments into the internal map.
    #[cfg(feature = "cli")]
    pub fn from_arg_matches(matches: &clap::ArgMatches, keys: &[&str]) -> Self {
        let mut values = HashMap::new();
        for key in keys {
            if let Some(val) = matches.get_one::<String>(key) {
                values.insert(key.to_string(), val.clone());
            }
        }
        CliSource { values }
    }

    /// Returns a typed FigValue for the given key, using `hint` to
    /// determine which mutagenesis variant to parse into.
    pub fn get_typed(&self, key: &str, hint: &FigValue) -> Option<FigValue> {
        let raw = self.values.get(key)?;
        parse_env_value(raw, hint)
    }
}

impl Source for CliSource {
    /// Returns the raw flag value as FigValue::String.
    /// The Tree calls get_typed() when it knows the target type.
    fn get(&self, key: &str) -> Option<FigValue> {
        self.values
            .get(key)
            .map(|v| FigValue::String(v.clone()))
    }

    fn source_name(&self) -> String {
        "flag".into()
    }

    fn as_fig_source(&self) -> FigSource {
        FigSource::Flag("flag".into())
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn source_with(key: &str, val: &str) -> CliSource {
        let mut m = HashMap::new();
        m.insert(key.to_string(), val.to_string());
        CliSource::new(m)
    }

    #[test]
    fn test_get_returns_string_value() {
        let src = source_with("workers", "20");
        assert_eq!(src.get("workers"), Some(FigValue::String("20".into())));
    }

    #[test]
    fn test_get_returns_none_for_missing_key() {
        let src = CliSource::new(HashMap::new());
        assert!(src.get("workers").is_none());
    }

    #[test]
    fn test_get_typed_parses_int() {
        let src = source_with("workers", "20");
        assert_eq!(
            src.get_typed("workers", &FigValue::Int(0)),
            Some(FigValue::Int(20))
        );
    }

    #[test]
    fn test_get_typed_parses_bool() {
        let src = source_with("debug", "true");
        assert_eq!(
            src.get_typed("debug", &FigValue::Bool(false)),
            Some(FigValue::Bool(true))
        );
    }

    #[test]
    fn test_source_name() {
        assert_eq!(CliSource::new(HashMap::new()).source_name(), "flag");
    }

    #[test]
    fn test_as_fig_source() {
        assert!(matches!(
            CliSource::new(HashMap::new()).as_fig_source(),
            FigSource::Flag(_)
        ));
    }
}
