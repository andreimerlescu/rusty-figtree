use std::time::Duration;
use crate::fig::{FigSource, FigValue};
use crate::priority::Source;

/// A configuration source that reads from environment variables.
///
/// Keys are looked up by name as-is. The caller is responsible for
/// passing the correct key name — EnvSource does not uppercase or
/// transform the key before calling os::var.
///
/// Values are returned as FigValue::String. The Tree is responsible
/// for parsing the string into the correct mutagenesis type by
/// consulting the Fig's declared type before recording the value.
///
/// For typed parsing, use EnvSource::get_typed() which attempts to
/// parse the environment variable string into the requested FigValue
/// variant based on a reference value's type.
pub struct EnvSource;

impl EnvSource {
    pub fn new() -> Self {
        EnvSource
    }

    /// Attempts to parse a raw environment variable string into a
    /// FigValue of the same mutagenesis type as `hint`.
    ///
    /// The hint is used only to determine which variant to parse into
    /// — its value is not used. Returns None if the variable is not
    /// set or if parsing fails.
    pub fn get_typed(&self, key: &str, hint: &FigValue) -> Option<FigValue> {
        let raw = std::env::var(key).ok()?;
        parse_env_value(&raw, hint)
    }
}

impl Default for EnvSource {
    fn default() -> Self {
        EnvSource::new()
    }
}

impl Source for EnvSource {
    /// Returns the raw environment variable as FigValue::String.
    /// The Tree calls get_typed() when it knows the target type.
    fn get(&self, key: &str) -> Option<FigValue> {
        std::env::var(key).ok().map(FigValue::String)
    }

    fn source_name(&self) -> String {
        "env".into()
    }

    fn as_fig_source(&self) -> FigSource {
        FigSource::Environment("env".into())
    }
}

/// Parses a raw string into a FigValue matching the mutagenesis
/// variant of `hint`. Used by EnvSource::get_typed() and by the
/// Tree when resolving environment variables into typed Figs.
pub fn parse_env_value(raw: &str, hint: &FigValue) -> Option<FigValue> {
    match hint {
        FigValue::String(_) => {
            Some(FigValue::String(raw.to_string()))
        }
        FigValue::Int(_) => {
            raw.trim().parse::<i32>().ok().map(FigValue::Int)
        }
        FigValue::Int64(_) => {
            raw.trim().parse::<i64>().ok().map(FigValue::Int64)
        }
        FigValue::Int128(_) => {
            raw.trim().parse::<i128>().ok().map(FigValue::Int128)
        }
        FigValue::Float64(_) => {
            raw.trim().parse::<f64>().ok().map(FigValue::Float64)
        }
        FigValue::Float128(_) => {
            raw.trim().parse::<f64>().ok().map(FigValue::Float128)
        }
        FigValue::Bool(_) => {
            match raw.trim().to_lowercase().as_str() {
                "true" | "1" | "yes" | "on"  => Some(FigValue::Bool(true)),
                "false" | "0" | "no" | "off" => Some(FigValue::Bool(false)),
                _                             => None,
            }
        }
        FigValue::Duration(_) => {
            parse_duration(raw.trim())
        }
        // List variants — comma-separated values
        FigValue::ListString(_) => {
            let items = split_csv(raw);
            Some(FigValue::ListString(items))
        }
        FigValue::ListInt(_) => {
            let items: Option<Vec<i32>> = split_csv(raw)
                .iter()
                .map(|s| s.parse::<i32>().ok())
                .collect();
            items.map(FigValue::ListInt)
        }
        FigValue::ListInt64(_) => {
            let items: Option<Vec<i64>> = split_csv(raw)
                .iter()
                .map(|s| s.parse::<i64>().ok())
                .collect();
            items.map(FigValue::ListInt64)
        }
        FigValue::ListInt128(_) => {
            let items: Option<Vec<i128>> = split_csv(raw)
                .iter()
                .map(|s| s.parse::<i128>().ok())
                .collect();
            items.map(FigValue::ListInt128)
        }
        FigValue::ListFloat64(_) => {
            let items: Option<Vec<f64>> = split_csv(raw)
                .iter()
                .map(|s| s.parse::<f64>().ok())
                .collect();
            items.map(FigValue::ListFloat64)
        }
        FigValue::ListFloat128(_) => {
            let items: Option<Vec<f64>> = split_csv(raw)
                .iter()
                .map(|s| s.parse::<f64>().ok())
                .collect();
            items.map(FigValue::ListFloat128)
        }
        FigValue::ListBool(_) => {
            let items: Option<Vec<bool>> = split_csv(raw)
                .iter()
                .map(|s| match s.to_lowercase().as_str() {
                    "true" | "1" | "yes" | "on"  => Some(true),
                    "false" | "0" | "no" | "off" => Some(false),
                    _                             => None,
                })
                .collect();
            items.map(FigValue::ListBool)
        }
        // Map variants — KEY=VALUE,KEY=VALUE pairs
        FigValue::MapString(_) => {
            parse_map_string(raw)
        }
        FigValue::MapInt(_) => {
            parse_map_typed(raw, |v| v.parse::<i32>().ok())
                .map(FigValue::MapInt)
        }
        FigValue::MapInt64(_) => {
            parse_map_typed(raw, |v| v.parse::<i64>().ok())
                .map(FigValue::MapInt64)
        }
        FigValue::MapInt128(_) => {
            parse_map_typed(raw, |v| v.parse::<i128>().ok())
                .map(FigValue::MapInt128)
        }
        FigValue::MapFloat64(_) => {
            parse_map_typed(raw, |v| v.parse::<f64>().ok())
                .map(FigValue::MapFloat64)
        }
        FigValue::MapFloat128(_) => {
            parse_map_typed(raw, |v| v.parse::<f64>().ok())
                .map(FigValue::MapFloat128)
        }
        FigValue::MapBool(_) => {
            parse_map_typed(raw, |v| match v.to_lowercase().as_str() {
                "true" | "1" | "yes" | "on"  => Some(true),
                "false" | "0" | "no" | "off" => Some(false),
                _                             => None,
            })
            .map(FigValue::MapBool)
        }
    }
}

/// Splits a comma-separated string into trimmed parts.
fn split_csv(raw: &str) -> Vec<String> {
    raw.split(',').map(|s| s.trim().to_string()).collect()
}

/// Parses KEY=VALUE,KEY=VALUE into HashMap<String, String>.
fn parse_map_string(raw: &str) -> Option<FigValue> {
    use std::collections::HashMap;
    let mut map = HashMap::new();
    for pair in raw.split(',') {
        let mut parts = pair.splitn(2, '=');
        let key = parts.next()?.trim().to_string();
        let val = parts.next()?.trim().to_string();
        map.insert(key, val);
    }
    Some(FigValue::MapString(map))
}

/// Parses KEY=VALUE,KEY=VALUE into HashMap<String, T> using a
/// value parser function.
fn parse_map_typed<T, F>(raw: &str, parser: F) -> Option<std::collections::HashMap<String, T>>
where
    F: Fn(&str) -> Option<T>,
{
    let mut map = std::collections::HashMap::new();
    for pair in raw.split(',') {
        let mut parts = pair.splitn(2, '=');
        let key = parts.next()?.trim().to_string();
        let val = parser(parts.next()?.trim())?;
        map.insert(key, val);
    }
    Some(map)
}

/// Parses duration strings into std::time::Duration.
///
/// Supports formats: "30s", "5m", "2h", "1d", "500ms"
/// Pure numeric strings are treated as seconds.
fn parse_duration(raw: &str) -> Option<FigValue> {
    if let Ok(secs) = raw.parse::<u64>() {
        return Some(FigValue::Duration(Duration::from_secs(secs)));
    }
    if raw.ends_with("ms") {
        let n: u64 = raw.trim_end_matches("ms").parse().ok()?;
        return Some(FigValue::Duration(Duration::from_millis(n)));
    }
    if raw.ends_with('s') {
        let n: u64 = raw.trim_end_matches('s').parse().ok()?;
        return Some(FigValue::Duration(Duration::from_secs(n)));
    }
    if raw.ends_with('m') {
        let n: u64 = raw.trim_end_matches('m').parse().ok()?;
        return Some(FigValue::Duration(Duration::from_secs(n * 60)));
    }
    if raw.ends_with('h') {
        let n: u64 = raw.trim_end_matches('h').parse().ok()?;
        return Some(FigValue::Duration(Duration::from_secs(n * 3600)));
    }
    if raw.ends_with('d') {
        let n: u64 = raw.trim_end_matches('d').parse().ok()?;
        return Some(FigValue::Duration(Duration::from_secs(n * 86400)));
    }
    None
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::time::Duration;

    fn hint(v: FigValue) -> FigValue { v }

    // ── parse_env_value ───────────────────────────────────────────────────────

    #[test]
    fn test_parses_string() {
        let result = parse_env_value("hello", &hint(FigValue::String(String::new())));
        assert_eq!(result, Some(FigValue::String("hello".into())));
    }

    #[test]
    fn test_parses_int() {
        let result = parse_env_value("42", &hint(FigValue::Int(0)));
        assert_eq!(result, Some(FigValue::Int(42)));
    }

    #[test]
    fn test_parses_int64() {
        let result = parse_env_value("9999999999", &hint(FigValue::Int64(0)));
        assert_eq!(result, Some(FigValue::Int64(9999999999)));
    }

    #[test]
    fn test_parses_int128_beyond_i64_max() {
        let big = (i64::MAX as i128) + 1;
        let result = parse_env_value(&big.to_string(), &hint(FigValue::Int128(0)));
        assert_eq!(result, Some(FigValue::Int128(big)));
    }

    #[test]
    fn test_parses_float64() {
        let result = parse_env_value("3.14", &hint(FigValue::Float64(0.0)));
        assert_eq!(result, Some(FigValue::Float64(3.14)));
    }

    #[test]
    fn test_parses_bool_true_variants() {
        for s in &["true", "1", "yes", "on"] {
            assert_eq!(
                parse_env_value(s, &hint(FigValue::Bool(false))),
                Some(FigValue::Bool(true)),
                "failed for '{}'", s
            );
        }
    }

    #[test]
    fn test_parses_bool_false_variants() {
        for s in &["false", "0", "no", "off"] {
            assert_eq!(
                parse_env_value(s, &hint(FigValue::Bool(true))),
                Some(FigValue::Bool(false)),
                "failed for '{}'", s
            );
        }
    }

    #[test]
    fn test_invalid_int_returns_none() {
        assert!(parse_env_value("notanint", &hint(FigValue::Int(0))).is_none());
    }

    // ── duration parsing ──────────────────────────────────────────────────────

    #[test]
    fn test_parses_duration_seconds() {
        let result = parse_env_value("30s", &hint(FigValue::Duration(Duration::ZERO)));
        assert_eq!(result, Some(FigValue::Duration(Duration::from_secs(30))));
    }

    #[test]
    fn test_parses_duration_minutes() {
        let result = parse_env_value("5m", &hint(FigValue::Duration(Duration::ZERO)));
        assert_eq!(result, Some(FigValue::Duration(Duration::from_secs(300))));
    }

    #[test]
    fn test_parses_duration_hours() {
        let result = parse_env_value("2h", &hint(FigValue::Duration(Duration::ZERO)));
        assert_eq!(result, Some(FigValue::Duration(Duration::from_secs(7200))));
    }

    #[test]
    fn test_parses_duration_days() {
        let result = parse_env_value("1d", &hint(FigValue::Duration(Duration::ZERO)));
        assert_eq!(result, Some(FigValue::Duration(Duration::from_secs(86400))));
    }

    #[test]
    fn test_parses_duration_milliseconds() {
        let result = parse_env_value("500ms", &hint(FigValue::Duration(Duration::ZERO)));
        assert_eq!(result, Some(FigValue::Duration(Duration::from_millis(500))));
    }

    #[test]
    fn test_parses_duration_bare_number_as_seconds() {
        let result = parse_env_value("60", &hint(FigValue::Duration(Duration::ZERO)));
        assert_eq!(result, Some(FigValue::Duration(Duration::from_secs(60))));
    }

    // ── list parsing ──────────────────────────────────────────────────────────

    #[test]
    fn test_parses_list_string() {
        let result = parse_env_value("a,b,c", &hint(FigValue::ListString(vec![])));
        assert_eq!(result, Some(FigValue::ListString(vec![
            "a".into(), "b".into(), "c".into()
        ])));
    }

    #[test]
    fn test_parses_list_int() {
        let result = parse_env_value("1,2,3", &hint(FigValue::ListInt(vec![])));
        assert_eq!(result, Some(FigValue::ListInt(vec![1, 2, 3])));
    }

    #[test]
    fn test_parses_list_int128() {
        let big = i64::MAX as i128 + 1;
        let raw = format!("{},{}", big, big);
        let result = parse_env_value(&raw, &hint(FigValue::ListInt128(vec![])));
        assert_eq!(result, Some(FigValue::ListInt128(vec![big, big])));
    }

    #[test]
    fn test_parses_list_bool() {
        let result = parse_env_value("true,false,1,0", &hint(FigValue::ListBool(vec![])));
        assert_eq!(result, Some(FigValue::ListBool(vec![true, false, true, false])));
    }

    #[test]
    fn test_list_with_spaces_is_trimmed() {
        let result = parse_env_value(" a , b , c ", &hint(FigValue::ListString(vec![])));
        assert_eq!(result, Some(FigValue::ListString(vec![
            "a".into(), "b".into(), "c".into()
        ])));
    }

    // ── map parsing ───────────────────────────────────────────────────────────

    #[test]
    fn test_parses_map_string() {
        let result = parse_env_value(
            "env=prod,version=1.0",
            &hint(FigValue::MapString(HashMap::new())),
        );
        if let Some(FigValue::MapString(m)) = result {
            assert_eq!(m.get("env"),     Some(&"prod".to_string()));
            assert_eq!(m.get("version"), Some(&"1.0".to_string()));
        } else {
            panic!("expected MapString");
        }
    }

    #[test]
    fn test_parses_map_int() {
        let result = parse_env_value(
            "port=8080,workers=4",
            &hint(FigValue::MapInt(HashMap::new())),
        );
        if let Some(FigValue::MapInt(m)) = result {
            assert_eq!(m.get("port"),    Some(&8080i32));
            assert_eq!(m.get("workers"), Some(&4i32));
        } else {
            panic!("expected MapInt");
        }
    }

    #[test]
    fn test_parses_map_bool() {
        let result = parse_env_value(
            "debug=true,verbose=false",
            &hint(FigValue::MapBool(HashMap::new())),
        );
        if let Some(FigValue::MapBool(m)) = result {
            assert_eq!(m.get("debug"),   Some(&true));
            assert_eq!(m.get("verbose"), Some(&false));
        } else {
            panic!("expected MapBool");
        }
    }

    // ── EnvSource trait impl ──────────────────────────────────────────────────

    #[test]
    fn test_env_source_returns_none_for_unset_var() {
        let src = EnvSource::new();
        assert!(src.get("FIGTREE_DEFINITELY_NOT_SET_XYZ").is_none());
    }

    #[test]
    fn test_env_source_name() {
        assert_eq!(EnvSource::new().source_name(), "env");
    }

    #[test]
    fn test_env_source_as_fig_source() {
        assert!(matches!(
            EnvSource::new().as_fig_source(),
            FigSource::Environment(_)
        ));
    }
}
