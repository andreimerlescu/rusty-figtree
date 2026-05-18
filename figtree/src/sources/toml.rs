#[cfg(feature = "toml-fmt")]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use crate::error::{FigtreeError, FigtreeResult};
use crate::fig::{FigSource, FigValue};
use crate::priority::Source;

/// A configuration source backed by a TOML file.
///
/// TOML is the de facto standard configuration format in the Rust
/// ecosystem — Cargo itself uses it. Not supporting TOML would be
/// a glaring omission for any Rust configuration library.
///
/// TOML is a typed-origin source. Integer, float, boolean, array,
/// and table values are preserved from the file without going through
/// string parsing. This makes TOML conversions more precise than
/// string-origin sources like EnvSource or IniSource.
///
/// Nested TOML tables are flattened into dotted keys:
///   [database]
///   host = "localhost"
/// becomes accessible as "database.host".
///
/// The file is read and parsed once at construction time.
pub struct TomlSource {
    path:   PathBuf,
    values: toml::Table,
}

impl TomlSource {
    /// Loads and parses a TOML file.
    pub fn load(path: impl AsRef<Path>) -> FigtreeResult<Self> {
        let path     = path.as_ref().to_path_buf();
        let contents = std::fs::read_to_string(&path).map_err(|_| {
            FigtreeError::FileNotFound(path.display().to_string())
        })?;
        let table: toml::Table = contents.parse().map_err(|e: toml::de::Error| {
            FigtreeError::FileParseFailed {
                path:   path.display().to_string(),
                reason: e.to_string(),
            }
        })?;
        Ok(TomlSource { path, values: table })
    }

    /// Returns a typed FigValue for the given key using the hint
    /// to determine which mutagenesis variant to convert into.
    ///
    /// Dotted keys ("database.host") are resolved by walking the
    /// nested table structure.
    pub fn get_typed(&self, key: &str, hint: &FigValue) -> Option<FigValue> {
        let val = self.get_toml_value(key)?;
        toml_to_figvalue(&val, hint)
    }

    /// Resolves a possibly-dotted key through nested TOML tables.
    fn get_toml_value(&self, key: &str) -> Option<toml::Value> {
        let parts: Vec<&str> = key.splitn(2, '.').collect();
        if parts.len() == 1 {
            return self.values.get(key).cloned();
        }
        // nested key — walk into the table
        let table = self.values.get(parts[0])?.as_table()?;
        table.get(parts[1]).cloned()
    }
}

impl Source for TomlSource {
    fn get(&self, key: &str) -> Option<FigValue> {
        let v = self.get_toml_value(key)?;
        toml_scalar_to_string(&v).map(FigValue::String)
    }

    fn source_name(&self) -> String {
        format!("file({})", self.path.display())
    }

    fn as_fig_source(&self) -> FigSource {
        FigSource::File(self.path.display().to_string())
    }
}

fn toml_to_figvalue(v: &toml::Value, hint: &FigValue) -> Option<FigValue> {
    match hint {
        FigValue::String(_)   => toml_scalar_to_string(v).map(FigValue::String),
        FigValue::Int(_)      => v.as_integer().and_then(|n| i32::try_from(n).ok()).map(FigValue::Int),
        FigValue::Int64(_)    => v.as_integer().map(FigValue::Int64),
        FigValue::Int128(_)   => v.as_integer().map(|n| FigValue::Int128(n as i128)),
        FigValue::Float64(_)  => v.as_float().map(FigValue::Float64),
        FigValue::Float128(_) => v.as_float().map(FigValue::Float128),
        FigValue::Bool(_)     => v.as_bool().map(FigValue::Bool),
        FigValue::Duration(_) => {
            toml_scalar_to_string(v)
                .and_then(|s| crate::sources::env::parse_env_value(&s, hint))
        }
        FigValue::ListString(_) => {
            let arr = v.as_array()?;
            let items: Option<Vec<String>> = arr.iter()
                .map(|i| toml_scalar_to_string(i))
                .collect();
            items.map(FigValue::ListString)
        }
        FigValue::ListInt(_) => {
            let arr = v.as_array()?;
            let items: Option<Vec<i32>> = arr.iter()
                .map(|i| i.as_integer().and_then(|n| i32::try_from(n).ok()))
                .collect();
            items.map(FigValue::ListInt)
        }
        FigValue::ListInt64(_) => {
            let arr = v.as_array()?;
            let items: Option<Vec<i64>> = arr.iter()
                .map(|i| i.as_integer())
                .collect();
            items.map(FigValue::ListInt64)
        }
        FigValue::ListInt128(_) => {
            let arr = v.as_array()?;
            let items: Option<Vec<i128>> = arr.iter()
                .map(|i| i.as_integer().map(|n| n as i128))
                .collect();
            items.map(FigValue::ListInt128)
        }
        FigValue::ListFloat64(_) => {
            let arr = v.as_array()?;
            let items: Option<Vec<f64>> = arr.iter()
                .map(|i| i.as_float())
                .collect();
            items.map(FigValue::ListFloat64)
        }
        FigValue::ListFloat128(_) => {
            let arr = v.as_array()?;
            let items: Option<Vec<f64>> = arr.iter()
                .map(|i| i.as_float())
                .collect();
            items.map(FigValue::ListFloat128)
        }
        FigValue::ListBool(_) => {
            let arr = v.as_array()?;
            let items: Option<Vec<bool>> = arr.iter()
                .map(|i| i.as_bool())
                .collect();
            items.map(FigValue::ListBool)
        }
        FigValue::MapString(_) => {
            let tbl = v.as_table()?;
            let mut m = HashMap::new();
            for (k, val) in tbl {
                m.insert(k.clone(), toml_scalar_to_string(val)?);
            }
            Some(FigValue::MapString(m))
        }
        FigValue::MapInt(_) => {
            let tbl = v.as_table()?;
            let mut m = HashMap::new();
            for (k, val) in tbl {
                m.insert(k.clone(), val.as_integer().and_then(|n| i32::try_from(n).ok())?);
            }
            Some(FigValue::MapInt(m))
        }
        FigValue::MapInt64(_) => {
            let tbl = v.as_table()?;
            let mut m = HashMap::new();
            for (k, val) in tbl { m.insert(k.clone(), val.as_integer()?); }
            Some(FigValue::MapInt64(m))
        }
        FigValue::MapInt128(_) => {
            let tbl = v.as_table()?;
            let mut m = HashMap::new();
            for (k, val) in tbl {
                m.insert(k.clone(), val.as_integer().map(|n| n as i128)?);
            }
            Some(FigValue::MapInt128(m))
        }
        FigValue::MapFloat64(_) => {
            let tbl = v.as_table()?;
            let mut m = HashMap::new();
            for (k, val) in tbl { m.insert(k.clone(), val.as_float()?); }
            Some(FigValue::MapFloat64(m))
        }
        FigValue::MapFloat128(_) => {
            let tbl = v.as_table()?;
            let mut m = HashMap::new();
            for (k, val) in tbl { m.insert(k.clone(), val.as_float()?); }
            Some(FigValue::MapFloat128(m))
        }
        FigValue::MapBool(_) => {
            let tbl = v.as_table()?;
            let mut m = HashMap::new();
            for (k, val) in tbl { m.insert(k.clone(), val.as_bool()?); }
            Some(FigValue::MapBool(m))
        }
    }
}

fn toml_scalar_to_string(v: &toml::Value) -> Option<String> {
    match v {
        toml::Value::String(s)   => Some(s.clone()),
        toml::Value::Integer(n)  => Some(n.to_string()),
        toml::Value::Float(f)    => Some(f.to_string()),
        toml::Value::Boolean(b)  => Some(b.to_string()),
        toml::Value::Datetime(d) => Some(d.to_string()),
        _                        => None,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn toml_source(contents: &str) -> TomlSource {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(contents.as_bytes()).unwrap();
        TomlSource::load(f.path()).unwrap()
    }

    #[test]
    fn test_load_valid_toml() {
        let src = toml_source("workers = 10\nendpoint = \"http://localhost\"\n");
        assert!(src.get("workers").is_some());
    }

    #[test]
    fn test_get_typed_int() {
        let src = toml_source("workers = 10\n");
        assert_eq!(src.get_typed("workers", &FigValue::Int(0)), Some(FigValue::Int(10)));
    }

    #[test]
    fn test_get_typed_bool() {
        let src = toml_source("debug = true\n");
        assert_eq!(src.get_typed("debug", &FigValue::Bool(false)), Some(FigValue::Bool(true)));
    }

    #[test]
    fn test_get_typed_float64() {
        let src = toml_source("threshold = 0.75\n");
        assert_eq!(src.get_typed("threshold", &FigValue::Float64(0.0)), Some(FigValue::Float64(0.75)));
    }

    #[test]
    fn test_get_typed_list_string() {
        let src = toml_source("servers = [\"a\", \"b\", \"c\"]\n");
        assert_eq!(
            src.get_typed("servers", &FigValue::ListString(vec![])),
            Some(FigValue::ListString(vec!["a".into(), "b".into(), "c".into()]))
        );
    }

    #[test]
    fn test_nested_key_via_dotted_access() {
        let src = toml_source("[database]\nhost = \"localhost\"\nport = 5432\n");
        assert_eq!(
            src.get_typed("database.host", &FigValue::String(String::new())),
            Some(FigValue::String("localhost".into()))
        );
        assert_eq!(
            src.get_typed("database.port", &FigValue::Int(0)),
            Some(FigValue::Int(5432))
        );
    }

    #[test]
    fn test_get_returns_none_for_missing() {
        let src = toml_source("workers = 10\n");
        assert!(src.get("missing").is_none());
    }

    #[test]
    fn test_load_nonexistent_returns_error() {
        assert!(matches!(
            TomlSource::load("/no/such/file.toml"),
            Err(FigtreeError::FileNotFound(_))
        ));
    }

    #[test]
    fn test_as_fig_source_is_file_variant() {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(b"a = 1\n").unwrap();
        let src = TomlSource::load(f.path()).unwrap();
        assert!(matches!(src.as_fig_source(), FigSource::File(_)));
    }
}
