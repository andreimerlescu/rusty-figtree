#[cfg(feature = "plist")]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use crate::error::{FigtreeError, FigtreeResult};
use crate::fig::{FigSource, FigValue};
use crate::priority::Source;

/// A configuration source backed by an Apple property list file.
///
/// Supports all three plist variants:
///   - XML (.plist)
///   - Binary (.binaryPlist)
///   - NeXTSTEP ASCII (legacy)
///
/// The plist crate detects the format automatically from the file
/// content rather than the extension. All three variants are
/// transparently supported without configuration.
///
/// Plist is a typed-origin source. Integer, float, boolean, array,
/// and dictionary values are preserved from the file. String
/// conversion is used only as a fallback for types that have no
/// direct FigValue equivalent (dates, data blobs).
///
/// Useful for:
///   - macOS and iOS application configuration
///   - Swift interoperability
///   - Engineers working in the Apple ecosystem
///   - Preferences files from macOS applications
pub struct PlistSource {
    path:   PathBuf,
    values: plist::Dictionary,
}

impl PlistSource {
    /// Loads and parses a plist file in any supported format.
    pub fn load(path: impl AsRef<Path>) -> FigtreeResult<Self> {
        let path = path.as_ref().to_path_buf();
        let value = plist::Value::from_file(&path).map_err(|e| {
            FigtreeError::FileParseFailed {
                path:   path.display().to_string(),
                reason: e.to_string(),
            }
        })?;
        let dict = value.into_dictionary().ok_or_else(|| {
            FigtreeError::FileParseFailed {
                path:   path.display().to_string(),
                reason: "plist root must be a dictionary".into(),
            }
        })?;
        Ok(PlistSource { path, values: dict })
    }

    /// Returns a typed FigValue for the given key.
    pub fn get_typed(&self, key: &str, hint: &FigValue) -> Option<FigValue> {
        let val = self.values.get(key)?;
        plist_to_figvalue(val, hint)
    }
}

impl Source for PlistSource {
    fn get(&self, key: &str) -> Option<FigValue> {
        let v = self.values.get(key)?;
        plist_scalar_to_string(v).map(FigValue::String)
    }

    fn source_name(&self) -> String {
        format!("file({})", self.path.display())
    }

    fn as_fig_source(&self) -> FigSource {
        FigSource::File(self.path.display().to_string())
    }
}

fn plist_to_figvalue(v: &plist::Value, hint: &FigValue) -> Option<FigValue> {
    match hint {
        FigValue::String(_)   => plist_scalar_to_string(v).map(FigValue::String),
        FigValue::Int(_)      => v.as_signed_integer().and_then(|n| i32::try_from(n).ok()).map(FigValue::Int),
        FigValue::Int64(_)    => v.as_signed_integer().map(FigValue::Int64),
        FigValue::Int128(_)   => v.as_signed_integer().map(|n| FigValue::Int128(n as i128)),
        FigValue::Float64(_)  => v.as_real().map(FigValue::Float64),
        FigValue::Float128(_) => v.as_real().map(FigValue::Float128),
        FigValue::Bool(_)     => v.as_boolean().map(FigValue::Bool),
        FigValue::Duration(_) => {
            plist_scalar_to_string(v)
                .and_then(|s| crate::sources::env::parse_env_value(&s, hint))
        }
        FigValue::ListString(_) => {
            let arr = v.as_array()?;
            let items: Option<Vec<String>> = arr.iter()
                .map(|i| plist_scalar_to_string(i))
                .collect();
            items.map(FigValue::ListString)
        }
        FigValue::ListInt(_) => {
            let arr = v.as_array()?;
            let items: Option<Vec<i32>> = arr.iter()
                .map(|i| i.as_signed_integer().and_then(|n| i32::try_from(n).ok()))
                .collect();
            items.map(FigValue::ListInt)
        }
        FigValue::ListInt64(_) => {
            let arr = v.as_array()?;
            let items: Option<Vec<i64>> = arr.iter()
                .map(|i| i.as_signed_integer())
                .collect();
            items.map(FigValue::ListInt64)
        }
        FigValue::ListInt128(_) => {
            let arr = v.as_array()?;
            let items: Option<Vec<i128>> = arr.iter()
                .map(|i| i.as_signed_integer().map(|n| n as i128))
                .collect();
            items.map(FigValue::ListInt128)
        }
        FigValue::ListFloat64(_) => {
            let arr = v.as_array()?;
            let items: Option<Vec<f64>> = arr.iter()
                .map(|i| i.as_real())
                .collect();
            items.map(FigValue::ListFloat64)
        }
        FigValue::ListFloat128(_) => {
            let arr = v.as_array()?;
            let items: Option<Vec<f64>> = arr.iter()
                .map(|i| i.as_real())
                .collect();
            items.map(FigValue::ListFloat128)
        }
        FigValue::ListBool(_) => {
            let arr = v.as_array()?;
            let items: Option<Vec<bool>> = arr.iter()
                .map(|i| i.as_boolean())
                .collect();
            items.map(FigValue::ListBool)
        }
        FigValue::MapString(_) => {
            let dict = v.as_dictionary()?;
            let mut m = HashMap::new();
            for (k, val) in dict {
                m.insert(k.clone(), plist_scalar_to_string(val)?);
            }
            Some(FigValue::MapString(m))
        }
        FigValue::MapInt(_) => {
            let dict = v.as_dictionary()?;
            let mut m = HashMap::new();
            for (k, val) in dict {
                m.insert(k.clone(), val.as_signed_integer().and_then(|n| i32::try_from(n).ok())?);
            }
            Some(FigValue::MapInt(m))
        }
        FigValue::MapInt64(_) => {
            let dict = v.as_dictionary()?;
            let mut m = HashMap::new();
            for (k, val) in dict { m.insert(k.clone(), val.as_signed_integer()?); }
            Some(FigValue::MapInt64(m))
        }
        FigValue::MapInt128(_) => {
            let dict = v.as_dictionary()?;
            let mut m = HashMap::new();
            for (k, val) in dict {
                m.insert(k.clone(), val.as_signed_integer().map(|n| n as i128)?);
            }
            Some(FigValue::MapInt128(m))
        }
        FigValue::MapFloat64(_) => {
            let dict = v.as_dictionary()?;
            let mut m = HashMap::new();
            for (k, val) in dict { m.insert(k.clone(), val.as_real()?); }
            Some(FigValue::MapFloat64(m))
        }
        FigValue::MapFloat128(_) => {
            let dict = v.as_dictionary()?;
            let mut m = HashMap::new();
            for (k, val) in dict { m.insert(k.clone(), val.as_real()?); }
            Some(FigValue::MapFloat128(m))
        }
        FigValue::MapBool(_) => {
            let dict = v.as_dictionary()?;
            let mut m = HashMap::new();
            for (k, val) in dict { m.insert(k.clone(), val.as_boolean()?); }
            Some(FigValue::MapBool(m))
        }
    }
}

fn plist_scalar_to_string(v: &plist::Value) -> Option<String> {
    match v {
        plist::Value::String(s)  => Some(s.clone()),
        plist::Value::Integer(n) => Some(n.as_signed().unwrap_or(0).to_string()),
        plist::Value::Real(f)    => Some(f.to_string()),
        plist::Value::Boolean(b) => Some(b.to_string()),
        _                        => None,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn plist_source(contents: &str) -> PlistSource {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(contents.as_bytes()).unwrap();
        PlistSource::load(f.path()).unwrap()
    }

    const SIMPLE_PLIST: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>workers</key>
    <integer>10</integer>
    <key>debug</key>
    <true/>
    <key>endpoint</key>
    <string>http://localhost</string>
    <key>threshold</key>
    <real>0.75</real>
</dict>
</plist>"#;

    #[test]
    fn test_load_valid_plist() {
        let src = plist_source(SIMPLE_PLIST);
        assert!(src.get("workers").is_some());
    }

    #[test]
    fn test_get_typed_int() {
        let src = plist_source(SIMPLE_PLIST);
        assert_eq!(src.get_typed("workers", &FigValue::Int(0)), Some(FigValue::Int(10)));
    }

    #[test]
    fn test_get_typed_bool() {
        let src = plist_source(SIMPLE_PLIST);
        assert_eq!(src.get_typed("debug", &FigValue::Bool(false)), Some(FigValue::Bool(true)));
    }

    #[test]
    fn test_get_typed_float64() {
        let src = plist_source(SIMPLE_PLIST);
        assert_eq!(src.get_typed("threshold", &FigValue::Float64(0.0)), Some(FigValue::Float64(0.75)));
    }

    #[test]
    fn test_get_typed_string() {
        let src = plist_source(SIMPLE_PLIST);
        assert_eq!(
            src.get_typed("endpoint", &FigValue::String(String::new())),
            Some(FigValue::String("http://localhost".into()))
        );
    }

    #[test]
    fn test_get_returns_none_for_missing() {
        let src = plist_source(SIMPLE_PLIST);
        assert!(src.get("missing").is_none());
    }

    #[test]
    fn test_as_fig_source_is_file_variant() {
        let src = plist_source(SIMPLE_PLIST);
        assert!(matches!(src.as_fig_source(), FigSource::File(_)));
    }
}
