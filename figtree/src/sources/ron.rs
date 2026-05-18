#[cfg(feature = "ron")]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use crate::error::{FigtreeError, FigtreeResult};
use crate::fig::{FigSource, FigValue};
use crate::priority::Source;

/// A configuration source backed by a RON file.
///
/// RON (Rusty Object Notation) is a configuration format designed to
/// map directly to Rust data structures. It is used as the primary
/// asset and configuration format by the Bevy game engine and has a
/// growing community in the Rust ecosystem.
///
/// RON's syntax advantages over JSON for Rust users:
///   - Trailing commas allowed
///   - Comments supported
///   - Optional field names in structs
///   - Rust-style enums as values
///   - Named tuple structs
///
/// RonSource expects the RON file to contain a top-level map of
/// string keys to values, equivalent to a Rust HashMap<String, ron::Value>.
///
/// RonSource is a typed-origin source. Integer, float, boolean, list,
/// and map values are preserved from the file without string parsing.
pub struct RonSource {
    path:   PathBuf,
    values: HashMap<String, ron::Value>,
}

impl RonSource {
    /// Loads and parses a RON file.
    pub fn load(path: impl AsRef<Path>) -> FigtreeResult<Self> {
        let path     = path.as_ref().to_path_buf();
        let contents = std::fs::read_to_string(&path).map_err(|_| {
            FigtreeError::FileNotFound(path.display().to_string())
        })?;
        let values: HashMap<String, ron::Value> =
            ron::from_str(&contents).map_err(|e| {
                FigtreeError::FileParseFailed {
                    path:   path.display().to_string(),
                    reason: e.to_string(),
                }
            })?;
        Ok(RonSource { path, values })
    }

    /// Returns a typed FigValue for the given key.
    pub fn get_typed(&self, key: &str, hint: &FigValue) -> Option<FigValue> {
        let val = self.values.get(key)?;
        ron_to_figvalue(val, hint)
    }
}

impl Source for RonSource {
    fn get(&self, key: &str) -> Option<FigValue> {
        let v = self.values.get(key)?;
        ron_scalar_to_string(v).map(FigValue::String)
    }

    fn source_name(&self) -> String {
        format!("file({})", self.path.display())
    }

    fn as_fig_source(&self) -> FigSource {
        FigSource::File(self.path.display().to_string())
    }
}

fn ron_to_figvalue(v: &ron::Value, hint: &FigValue) -> Option<FigValue> {
    match hint {
        FigValue::String(_)   => ron_scalar_to_string(v).map(FigValue::String),
        FigValue::Int(_)      => {
            if let ron::Value::Number(n) = v {
                n.as_i64().and_then(|n| i32::try_from(n).ok()).map(FigValue::Int)
            } else { None }
        }
        FigValue::Int64(_)    => {
            if let ron::Value::Number(n) = v {
                n.as_i64().map(FigValue::Int64)
            } else { None }
        }
        FigValue::Int128(_)   => {
            if let ron::Value::Number(n) = v {
                n.as_i64().map(|n| FigValue::Int128(n as i128))
            } else { None }
        }
        FigValue::Float64(_)  => {
            if let ron::Value::Number(n) = v {
                n.as_f64().map(FigValue::Float64)
            } else { None }
        }
        FigValue::Float128(_) => {
            if let ron::Value::Number(n) = v {
                n.as_f64().map(FigValue::Float128)
            } else { None }
        }
        FigValue::Bool(_)     => {
            if let ron::Value::Bool(b) = v { Some(FigValue::Bool(*b)) } else { None }
        }
        FigValue::Duration(_) => {
            ron_scalar_to_string(v)
                .and_then(|s| crate::sources::env::parse_env_value(&s, hint))
        }
        FigValue::ListString(_) => {
            if let ron::Value::Seq(arr) = v {
                let items: Option<Vec<String>> = arr.iter()
                    .map(|i| ron_scalar_to_string(i))
                    .collect();
                items.map(FigValue::ListString)
            } else { None }
        }
        FigValue::ListInt(_) => {
            if let ron::Value::Seq(arr) = v {
                let items: Option<Vec<i32>> = arr.iter()
                    .map(|i| if let ron::Value::Number(n) = i {
                        n.as_i64().and_then(|n| i32::try_from(n).ok())
                    } else { None })
                    .collect();
                items.map(FigValue::ListInt)
            } else { None }
        }
        FigValue::ListInt64(_) => {
            if let ron::Value::Seq(arr) = v {
                let items: Option<Vec<i64>> = arr.iter()
                    .map(|i| if let ron::Value::Number(n) = i { n.as_i64() } else { None })
                    .collect();
                items.map(FigValue::ListInt64)
            } else { None }
        }
        FigValue::ListInt128(_) => {
            if let ron::Value::Seq(arr) = v {
                let items: Option<Vec<i128>> = arr.iter()
                    .map(|i| if let ron::Value::Number(n) = i {
                        n.as_i64().map(|n| n as i128)
                    } else { None })
                    .collect();
                items.map(FigValue::ListInt128)
            } else { None }
        }
        FigValue::ListFloat64(_) => {
            if let ron::Value::Seq(arr) = v {
                let items: Option<Vec<f64>> = arr.iter()
                    .map(|i| if let ron::Value::Number(n) = i { n.as_f64() } else { None })
                    .collect();
                items.map(FigValue::ListFloat64)
            } else { None }
        }
        FigValue::ListFloat128(_) => {
            if let ron::Value::Seq(arr) = v {
                let items: Option<Vec<f64>> = arr.iter()
                    .map(|i| if let ron::Value::Number(n) = i { n.as_f64() } else { None })
                    .collect();
                items.map(FigValue::ListFloat128)
            } else { None }
        }
        FigValue::ListBool(_) => {
            if let ron::Value::Seq(arr) = v {
                let items: Option<Vec<bool>> = arr.iter()
                    .map(|i| if let ron::Value::Bool(b) = i { Some(*b) } else { None })
                    .collect();
                items.map(FigValue::ListBool)
            } else { None }
        }
        FigValue::MapString(_) => {
            if let ron::Value::Map(m) = v {
                let mut result = HashMap::new();
                for (k, val) in m {
                    let key = ron_scalar_to_string(k)?;
                    let val = ron_scalar_to_string(val)?;
                    result.insert(key, val);
                }
                Some(FigValue::MapString(result))
            } else { None }
        }
        FigValue::MapInt(_) => {
            if let ron::Value::Map(m) = v {
                let mut result = HashMap::new();
                for (k, val) in m {
                    let key = ron_scalar_to_string(k)?;
                    let val = if let ron::Value::Number(n) = val {
                        n.as_i64().and_then(|n| i32::try_from(n).ok())?
                    } else { return None; };
                    result.insert(key, val);
                }
                Some(FigValue::MapInt(result))
            } else { None }
        }
        FigValue::MapInt64(_) => {
            if let ron::Value::Map(m) = v {
                let mut result = HashMap::new();
                for (k, val) in m {
                    let key = ron_scalar_to_string(k)?;
                    let val = if let ron::Value::Number(n) = val { n.as_i64()? } else { return None; };
                    result.insert(key, val);
                }
                Some(FigValue::MapInt64(result))
            } else { None }
        }
        FigValue::MapInt128(_) => {
            if let ron::Value::Map(m) = v {
                let mut result = HashMap::new();
                for (k, val) in m {
                    let key = ron_scalar_to_string(k)?;
                    let val = if let ron::Value::Number(n) = val {
                        n.as_i64().map(|n| n as i128)?
                    } else { return None; };
                    result.insert(key, val);
                }
                Some(FigValue::MapInt128(result))
            } else { None }
        }
        FigValue::MapFloat64(_) => {
            if let ron::Value::Map(m) = v {
                let mut result = HashMap::new();
                for (k, val) in m {
                    let key = ron_scalar_to_string(k)?;
                    let val = if let ron::Value::Number(n) = val { n.as_f64()? } else { return None; };
                    result.insert(key, val);
                }
                Some(FigValue::MapFloat64(result))
            } else { None }
        }
        FigValue::MapFloat128(_) => {
            if let ron::Value::Map(m) = v {
                let mut result = HashMap::new();
                for (k, val) in m {
                    let key = ron_scalar_to_string(k)?;
                    let val = if let ron::Value::Number(n) = val { n.as_f64()? } else { return None; };
                    result.insert(key, val);
                }
                Some(FigValue::MapFloat128(result))
            } else { None }
        }
        FigValue::MapBool(_) => {
            if let ron::Value::Map(m) = v {
                let mut result = HashMap::new();
                for (k, val) in m {
                    let key = ron_scalar_to_string(k)?;
                    let val = if let ron::Value::Bool(b) = val { *b } else { return None; };
                    result.insert(key, val);
                }
                Some(FigValue::MapBool(result))
            } else { None }
        }
    }
}

fn ron_scalar_to_string(v: &ron::Value) -> Option<String> {
    match v {
        ron::Value::String(s) => Some(s.clone()),
        ron::Value::Number(n) => n.as_f64().map(|f| f.to_string()),
        ron::Value::Bool(b)   => Some(b.to_string()),
        _                     => None,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn ron_source(contents: &str) -> RonSource {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(contents.as_bytes()).unwrap();
        RonSource::load(f.path()).unwrap()
    }

    #[test]
    fn test_load_valid_ron() {
        let src = ron_source(r#"{"workers": 10, "endpoint": "http://localhost"}"#);
        assert!(src.get("workers").is_some());
    }

    #[test]
    fn test_get_typed_int() {
        let src = ron_source(r#"{"workers": 10}"#);
        assert_eq!(src.get_typed("workers", &FigValue::Int(0)), Some(FigValue::Int(10)));
    }

    #[test]
    fn test_get_typed_bool() {
        let src = ron_source(r#"{"debug": true}"#);
        assert_eq!(src.get_typed("debug", &FigValue::Bool(false)), Some(FigValue::Bool(true)));
    }

    #[test]
    fn test_get_typed_float64() {
        let src = ron_source(r#"{"threshold": 0.75}"#);
        assert_eq!(
            src.get_typed("threshold", &FigValue::Float64(0.0)),
            Some(FigValue::Float64(0.75))
        );
    }

    #[test]
    fn test_get_typed_list_string() {
        let src = ron_source(r#"{"servers": ["a", "b", "c"]}"#);
        assert_eq!(
            src.get_typed("servers", &FigValue::ListString(vec![])),
            Some(FigValue::ListString(vec!["a".into(), "b".into(), "c".into()]))
        );
    }

    #[test]
    fn test_get_returns_none_for_missing() {
        let src = ron_source(r#"{"workers": 10}"#);
        assert!(src.get("missing").is_none());
    }

    #[test]
    fn test_load_nonexistent_returns_error() {
        assert!(matches!(
            RonSource::load("/no/such/file.ron"),
            Err(FigtreeError::FileNotFound(_))
        ));
    }

    #[test]
    fn test_as_fig_source_is_file_variant() {
        let src = ron_source(r#"{"a": 1}"#);
        assert!(matches!(src.as_fig_source(), FigSource::File(_)));
    }
}
