#[cfg(feature = "yaml")]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use crate::error::{FigtreeError, FigtreeResult};
use crate::fig::{FigSource, FigValue};
use crate::priority::Source;

/// A configuration source backed by a YAML file.
///
/// The file is read and parsed once at construction time via
/// YamlSource::load(). After construction the source is immutable —
/// it holds a snapshot of the file's contents at load time.
///
/// Values are stored internally as serde_yaml::Value and converted
/// to FigValue on demand in get(). The conversion is best-effort —
/// if the YAML value cannot be represented as a FigValue::String
/// the key is treated as absent.
///
/// For typed resolution the Tree calls get_typed() with a hint,
/// which attempts to convert the YAML value into the correct
/// mutagenesis variant.
pub struct YamlSource {
    path:   PathBuf,
    values: HashMap<String, serde_yaml::Value>,
}

impl YamlSource {
    /// Loads and parses a YAML file. Returns FigtreeError::FileNotFound
    /// if the file does not exist, or FigtreeError::FileParseFailed if
    /// the file exists but cannot be parsed as YAML.
    pub fn load(path: impl AsRef<Path>) -> FigtreeResult<Self> {
        let path = path.as_ref().to_path_buf();
        let contents = std::fs::read_to_string(&path).map_err(|_| {
            FigtreeError::FileNotFound(path.display().to_string())
        })?;
        let map: HashMap<String, serde_yaml::Value> =
            serde_yaml::from_str(&contents).map_err(|e| {
                FigtreeError::FileParseFailed {
                    path:   path.display().to_string(),
                    reason: e.to_string(),
                }
            })?;
        Ok(YamlSource { path, values: map })
    }

    /// Returns a typed FigValue for the given key using `hint` to
    /// determine which mutagenesis variant to convert into.
    pub fn get_typed(&self, key: &str, hint: &FigValue) -> Option<FigValue> {
        let yaml_val = self.values.get(key)?;
        yaml_to_figvalue(yaml_val, hint)
    }
}

impl Source for YamlSource {
    /// Returns the YAML value as FigValue::String where possible.
    fn get(&self, key: &str) -> Option<FigValue> {
        let v = self.values.get(key)?;
        yaml_scalar_to_string(v).map(FigValue::String)
    }

    fn source_name(&self) -> String {
        format!("file({})", self.path.display())
    }

    fn as_fig_source(&self) -> FigSource {
        FigSource::File(self.path.display().to_string())
    }
}

/// Converts a serde_yaml::Value to a FigValue matching the hint's
/// mutagenesis variant. Returns None if conversion is not possible.
fn yaml_to_figvalue(v: &serde_yaml::Value, hint: &FigValue) -> Option<FigValue> {
    match hint {
        FigValue::String(_) => {
            yaml_scalar_to_string(v).map(FigValue::String)
        }
        FigValue::Int(_) => {
            v.as_i64().and_then(|n| i32::try_from(n).ok()).map(FigValue::Int)
        }
        FigValue::Int64(_) => {
            v.as_i64().map(FigValue::Int64)
        }
        FigValue::Int128(_) => {
            v.as_i64().map(|n| FigValue::Int128(n as i128))
        }
        FigValue::Float64(_) => {
            v.as_f64().map(FigValue::Float64)
        }
        FigValue::Float128(_) => {
            v.as_f64().map(FigValue::Float128)
        }
        FigValue::Bool(_) => {
            v.as_bool().map(FigValue::Bool)
        }
        FigValue::Duration(_) => {
            yaml_scalar_to_string(v)
                .and_then(|s| crate::sources::env::parse_env_value(&s, hint))
        }
        FigValue::ListString(_) => {
            let seq = v.as_sequence()?;
            let items: Option<Vec<String>> = seq.iter()
                .map(|i| yaml_scalar_to_string(i))
                .collect();
            items.map(FigValue::ListString)
        }
        FigValue::ListInt(_) => {
            let seq = v.as_sequence()?;
            let items: Option<Vec<i32>> = seq.iter()
                .map(|i| i.as_i64().and_then(|n| i32::try_from(n).ok()))
                .collect();
            items.map(FigValue::ListInt)
        }
        FigValue::ListInt64(_) => {
            let seq = v.as_sequence()?;
            let items: Option<Vec<i64>> = seq.iter()
                .map(|i| i.as_i64())
                .collect();
            items.map(FigValue::ListInt64)
        }
        FigValue::ListInt128(_) => {
            let seq = v.as_sequence()?;
            let items: Option<Vec<i128>> = seq.iter()
                .map(|i| i.as_i64().map(|n| n as i128))
                .collect();
            items.map(FigValue::ListInt128)
        }
        FigValue::ListFloat64(_) => {
            let seq = v.as_sequence()?;
            let items: Option<Vec<f64>> = seq.iter()
                .map(|i| i.as_f64())
                .collect();
            items.map(FigValue::ListFloat64)
        }
        FigValue::ListFloat128(_) => {
            let seq = v.as_sequence()?;
            let items: Option<Vec<f64>> = seq.iter()
                .map(|i| i.as_f64())
                .collect();
            items.map(FigValue::ListFloat128)
        }
        FigValue::ListBool(_) => {
            let seq = v.as_sequence()?;
            let items: Option<Vec<bool>> = seq.iter()
                .map(|i| i.as_bool())
                .collect();
            items.map(FigValue::ListBool)
        }
        FigValue::MapString(_) => {
            let map = v.as_mapping()?;
            let mut result = HashMap::new();
            for (k, val) in map {
                let key = yaml_scalar_to_string(k)?;
                let val = yaml_scalar_to_string(val)?;
                result.insert(key, val);
            }
            Some(FigValue::MapString(result))
        }
        FigValue::MapInt(_) => {
            let map = v.as_mapping()?;
            let mut result = HashMap::new();
            for (k, val) in map {
                let key = yaml_scalar_to_string(k)?;
                let val = val.as_i64().and_then(|n| i32::try_from(n).ok())?;
                result.insert(key, val);
            }
            Some(FigValue::MapInt(result))
        }
        FigValue::MapInt64(_) => {
            let map = v.as_mapping()?;
            let mut result = HashMap::new();
            for (k, val) in map {
                let key = yaml_scalar_to_string(k)?;
                let val = val.as_i64()?;
                result.insert(key, val);
            }
            Some(FigValue::MapInt64(result))
        }
        FigValue::MapInt128(_) => {
            let map = v.as_mapping()?;
            let mut result = HashMap::new();
            for (k, val) in map {
                let key = yaml_scalar_to_string(k)?;
                let val = val.as_i64().map(|n| n as i128)?;
                result.insert(key, val);
            }
            Some(FigValue::MapInt128(result))
        }
        FigValue::MapFloat64(_) => {
            let map = v.as_mapping()?;
            let mut result = HashMap::new();
            for (k, val) in map {
                let key = yaml_scalar_to_string(k)?;
                let val = val.as_f64()?;
                result.insert(key, val);
            }
            Some(FigValue::MapFloat64(result))
        }
        FigValue::MapFloat128(_) => {
            let map = v.as_mapping()?;
            let mut result = HashMap::new();
            for (k, val) in map {
                let key = yaml_scalar_to_string(k)?;
                let val = val.as_f64()?;
                result.insert(key, val);
            }
            Some(FigValue::MapFloat128(result))
        }
        FigValue::MapBool(_) => {
            let map = v.as_mapping()?;
            let mut result = HashMap::new();
            for (k, val) in map {
                let key = yaml_scalar_to_string(k)?;
                let val = val.as_bool()?;
                result.insert(key, val);
            }
            Some(FigValue::MapBool(result))
        }
    }
}

/// Converts a scalar YAML value to a String. Returns None for
/// sequences and mappings.
fn yaml_scalar_to_string(v: &serde_yaml::Value) -> Option<String> {
    match v {
        serde_yaml::Value::String(s)  => Some(s.clone()),
        serde_yaml::Value::Number(n)  => Some(n.to_string()),
        serde_yaml::Value::Bool(b)    => Some(b.to_string()),
        serde_yaml::Value::Null       => Some(String::new()),
        _                             => None,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn yaml_source(contents: &str) -> YamlSource {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(contents.as_bytes()).unwrap();
        YamlSource::load(f.path()).unwrap()
    }

    #[test]
    fn test_load_valid_yaml() {
        let src = yaml_source("workers: 10\nendpoint: http://localhost\n");
        assert!(src.get("workers").is_some());
    }

    #[test]
    fn test_get_returns_string_for_scalar() {
        let src = yaml_source("workers: 10\n");
        assert_eq!(src.get("workers"), Some(FigValue::String("10".into())));
    }

    #[test]
    fn test_get_typed_int() {
        let src = yaml_source("workers: 10\n");
        assert_eq!(
            src.get_typed("workers", &FigValue::Int(0)),
            Some(FigValue::Int(10))
        );
    }

    #[test]
    fn test_get_typed_bool() {
        let src = yaml_source("debug: true\n");
        assert_eq!(
            src.get_typed("debug", &FigValue::Bool(false)),
            Some(FigValue::Bool(true))
        );
    }

    #[test]
    fn test_get_typed_list_string() {
        let src = yaml_source("servers:\n  - server1\n  - server2\n");
        assert_eq!(
            src.get_typed("servers", &FigValue::ListString(vec![])),
            Some(FigValue::ListString(vec!["server1".into(), "server2".into()]))
        );
    }

    #[test]
    fn test_get_typed_map_string() {
        let src = yaml_source("metadata:\n  env: prod\n  version: \"1.0\"\n");
        if let Some(FigValue::MapString(m)) = src.get_typed("metadata", &FigValue::MapString(HashMap::new())) {
            assert_eq!(m.get("env"),     Some(&"prod".to_string()));
            assert_eq!(m.get("version"), Some(&"1.0".to_string()));
        } else {
            panic!("expected MapString");
        }
    }

    #[test]
    fn test_get_returns_none_for_missing_key() {
        let src = yaml_source("workers: 10\n");
        assert!(src.get("missing").is_none());
    }

    #[test]
    fn test_load_nonexistent_file_returns_error() {
        let result = YamlSource::load("/nonexistent/path/config.yaml");
        assert!(matches!(result, Err(FigtreeError::FileNotFound(_))));
    }

    #[test]
    fn test_source_name_contains_path() {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(b"a: 1\n").unwrap();
        let src = YamlSource::load(f.path()).unwrap();
        assert!(src.source_name().contains("file("));
    }

    #[test]
    fn test_as_fig_source_is_file_variant() {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(b"a: 1\n").unwrap();
        let src = YamlSource::load(f.path()).unwrap();
        assert!(matches!(src.as_fig_source(), FigSource::File(_)));
    }
}
