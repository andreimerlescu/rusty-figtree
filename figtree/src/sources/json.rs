#[cfg(feature = "json")]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use crate::error::{FigtreeError, FigtreeResult};
use crate::fig::{FigSource, FigValue};
use crate::priority::Source;

/// A configuration source backed by a JSON file.
///
/// Follows the same design as YamlSource — file is read and parsed
/// once at construction, held as a snapshot, converted to FigValue
/// on demand. The Tree calls get_typed() for typed resolution.
pub struct JsonSource {
    path:   PathBuf,
    values: HashMap<String, serde_json::Value>,
}

impl JsonSource {
    /// Loads and parses a JSON file.
    pub fn load(path: impl AsRef<Path>) -> FigtreeResult<Self> {
        let path = path.as_ref().to_path_buf();
        let contents = std::fs::read_to_string(&path).map_err(|_| {
            FigtreeError::FileNotFound(path.display().to_string())
        })?;
        let map: HashMap<String, serde_json::Value> =
            serde_json::from_str(&contents).map_err(|e| {
                FigtreeError::FileParseFailed {
                    path:   path.display().to_string(),
                    reason: e.to_string(),
                }
            })?;
        Ok(JsonSource { path, values: map })
    }

    /// Returns a typed FigValue for the given key.
    pub fn get_typed(&self, key: &str, hint: &FigValue) -> Option<FigValue> {
        let json_val = self.values.get(key)?;
        json_to_figvalue(json_val, hint)
    }
}

impl Source for JsonSource {
    fn get(&self, key: &str) -> Option<FigValue> {
        let v = self.values.get(key)?;
        json_scalar_to_string(v).map(FigValue::String)
    }

    fn source_name(&self) -> String {
        format!("file({})", self.path.display())
    }

    fn as_fig_source(&self) -> FigSource {
        FigSource::File(self.path.display().to_string())
    }
}

fn json_to_figvalue(v: &serde_json::Value, hint: &FigValue) -> Option<FigValue> {
    match hint {
        FigValue::String(_)  => json_scalar_to_string(v).map(FigValue::String),
        FigValue::Int(_)     => v.as_i64().and_then(|n| i32::try_from(n).ok()).map(FigValue::Int),
        FigValue::Int64(_)   => v.as_i64().map(FigValue::Int64),
        FigValue::Int128(_)  => v.as_i64().map(|n| FigValue::Int128(n as i128)),
        FigValue::Float64(_) => v.as_f64().map(FigValue::Float64),
        FigValue::Float128(_)=> v.as_f64().map(FigValue::Float128),
        FigValue::Bool(_)    => v.as_bool().map(FigValue::Bool),
        FigValue::Duration(_) => {
            json_scalar_to_string(v)
                .and_then(|s| crate::sources::env::parse_env_value(&s, hint))
        }
        FigValue::ListString(_) => {
            let arr = v.as_array()?;
            let items: Option<Vec<String>> = arr.iter()
                .map(|i| json_scalar_to_string(i))
                .collect();
            items.map(FigValue::ListString)
        }
        FigValue::ListInt(_) => {
            let arr = v.as_array()?;
            let items: Option<Vec<i32>> = arr.iter()
                .map(|i| i.as_i64().and_then(|n| i32::try_from(n).ok()))
                .collect();
            items.map(FigValue::ListInt)
        }
        FigValue::ListInt64(_) => {
            let arr = v.as_array()?;
            let items: Option<Vec<i64>> = arr.iter().map(|i| i.as_i64()).collect();
            items.map(FigValue::ListInt64)
        }
        FigValue::ListInt128(_) => {
            let arr = v.as_array()?;
            let items: Option<Vec<i128>> = arr.iter()
                .map(|i| i.as_i64().map(|n| n as i128))
                .collect();
            items.map(FigValue::ListInt128)
        }
        FigValue::ListFloat64(_) => {
            let arr = v.as_array()?;
            let items: Option<Vec<f64>> = arr.iter().map(|i| i.as_f64()).collect();
            items.map(FigValue::ListFloat64)
        }
        FigValue::ListFloat128(_) => {
            let arr = v.as_array()?;
            let items: Option<Vec<f64>> = arr.iter().map(|i| i.as_f64()).collect();
            items.map(FigValue::ListFloat128)
        }
        FigValue::ListBool(_) => {
            let arr = v.as_array()?;
            let items: Option<Vec<bool>> = arr.iter().map(|i| i.as_bool()).collect();
            items.map(FigValue::ListBool)
        }
        FigValue::MapString(_) => {
            let obj = v.as_object()?;
            let mut m = HashMap::new();
            for (k, val) in obj {
                m.insert(k.clone(), json_scalar_to_string(val)?);
            }
            Some(FigValue::MapString(m))
        }
        FigValue::MapInt(_) => {
            let obj = v.as_object()?;
            let mut m = HashMap::new();
            for (k, val) in obj {
                m.insert(k.clone(), val.as_i64().and_then(|n| i32::try_from(n).ok())?);
            }
            Some(FigValue::MapInt(m))
        }
        FigValue::MapInt64(_) => {
            let obj = v.as_object()?;
            let mut m = HashMap::new();
            for (k, val) in obj { m.insert(k.clone(), val.as_i64()?); }
            Some(FigValue::MapInt64(m))
        }
        FigValue::MapInt128(_) => {
            let obj = v.as_object()?;
            let mut m = HashMap::new();
            for (k, val) in obj {
                m.insert(k.clone(), val.as_i64().map(|n| n as i128)?);
            }
            Some(FigValue::MapInt128(m))
        }
        FigValue::MapFloat64(_) => {
            let obj = v.as_object()?;
            let mut m = HashMap::new();
            for (k, val) in obj { m.insert(k.clone(), val.as_f64()?); }
            Some(FigValue::MapFloat64(m))
        }
        FigValue::MapFloat128(_) => {
            let obj = v.as_object()?;
            let mut m = HashMap::new();
            for (k, val) in obj { m.insert(k.clone(), val.as_f64()?); }
            Some(FigValue::MapFloat128(m))
        }
        FigValue::MapBool(_) => {
            let obj = v.as_object()?;
            let mut m = HashMap::new();
            for (k, val) in obj { m.insert(k.clone(), val.as_bool()?); }
            Some(FigValue::MapBool(m))
        }
    }
}

fn json_scalar_to_string(v: &serde_json::Value) -> Option<String> {
    match v {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        serde_json::Value::Bool(b)   => Some(b.to_string()),
        serde_json::Value::Null      => Some(String::new()),
        _                            => None,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn json_source(contents: &str) -> JsonSource {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(contents.as_bytes()).unwrap();
        JsonSource::load(f.path()).unwrap()
    }

    #[test]
    fn test_load_valid_json() {
        let src = json_source(r#"{"workers": 10}"#);
        assert!(src.get("workers").is_some());
    }

    #[test]
    fn test_get_typed_int() {
        let src = json_source(r#"{"workers": 10}"#);
        assert_eq!(src.get_typed("workers", &FigValue::Int(0)), Some(FigValue::Int(10)));
    }

    #[test]
    fn test_get_typed_bool() {
        let src = json_source(r#"{"debug": true}"#);
        assert_eq!(src.get_typed("debug", &FigValue::Bool(false)), Some(FigValue::Bool(true)));
    }

    #[test]
    fn test_get_typed_list_string() {
        let src = json_source(r#"{"servers": ["a", "b"]}"#);
        assert_eq!(
            src.get_typed("servers", &FigValue::ListString(vec![])),
            Some(FigValue::ListString(vec!["a".into(), "b".into()]))
        );
    }

    #[test]
    fn test_get_returns_none_for_missing_key() {
        let src = json_source(r#"{"workers": 10}"#);
        assert!(src.get("missing").is_none());
    }

    #[test]
    fn test_load_nonexistent_returns_error() {
        assert!(matches!(
            JsonSource::load("/no/such/file.json"),
            Err(FigtreeError::FileNotFound(_))
        ));
    }

    #[test]
    fn test_as_fig_source_is_file_variant() {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(b"{}").unwrap();
        let src = JsonSource::load(f.path()).unwrap();
        assert!(matches!(src.as_fig_source(), FigSource::File(_)));
    }
}
