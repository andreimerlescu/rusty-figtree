#[cfg(feature = "ini")]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use crate::error::{FigtreeError, FigtreeResult};
use crate::fig::{FigSource, FigValue};
use crate::priority::Source;
use crate::sources::env::parse_env_value;

/// A configuration source backed by an INI file.
///
/// INI files are flat key=value pairs, optionally grouped into
/// sections. figtree treats the default (sectionless) keys and
/// all section keys as a flat namespace. Section-qualified keys
/// are accessible as "section.key".
///
/// All values are strings in INI — parse_env_value drives typed
/// conversion using the hint, the same as EnvSource.
pub struct IniSource {
    path:   PathBuf,
    values: HashMap<String, String>,
}

impl IniSource {
    /// Loads and parses an INI file. Section-qualified keys are
    /// stored as "section.key". Top-level keys are stored as-is.
    pub fn load(path: impl AsRef<Path>) -> FigtreeResult<Self> {
        let path = path.as_ref().to_path_buf();
        let ini  = ini::Ini::load_from_file(&path).map_err(|e| {
            FigtreeError::FileParseFailed {
                path:   path.display().to_string(),
                reason: e.to_string(),
            }
        })?;

        let mut values = HashMap::new();
        for (section, props) in &ini {
            for (key, value) in props {
                let full_key = match section {
                    Some(s) => format!("{}.{}", s, key),
                    None    => key.to_string(),
                };
                values.insert(full_key, value.to_string());
            }
        }

        Ok(IniSource { path, values })
    }

    /// Returns a typed FigValue for the given key.
    pub fn get_typed(&self, key: &str, hint: &FigValue) -> Option<FigValue> {
        let raw = self.values.get(key)?;
        parse_env_value(raw, hint)
    }
}

impl Source for IniSource {
    fn get(&self, key: &str) -> Option<FigValue> {
        self.values.get(key).map(|v| FigValue::String(v.clone()))
    }

    fn source_name(&self) -> String {
        format!("file({})", self.path.display())
    }

    fn as_fig_source(&self) -> FigSource {
        FigSource::File(self.path.display().to_string())
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn ini_source(contents: &str) -> IniSource {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(contents.as_bytes()).unwrap();
        IniSource::load(f.path()).unwrap()
    }

    #[test]
    fn test_load_valid_ini() {
        let src = ini_source("workers=10\nendpoint=http://localhost\n");
        assert!(src.get("workers").is_some());
    }

    #[test]
    fn test_get_returns_string() {
        let src = ini_source("workers=10\n");
        assert_eq!(src.get("workers"), Some(FigValue::String("10".into())));
    }

    #[test]
    fn test_get_typed_int() {
        let src = ini_source("workers=10\n");
        assert_eq!(src.get_typed("workers", &FigValue::Int(0)), Some(FigValue::Int(10)));
    }

    #[test]
    fn test_section_key_accessible_as_dotted() {
        let src = ini_source("[database]\nhost=localhost\nport=5432\n");
        assert_eq!(
            src.get("database.host"),
            Some(FigValue::String("localhost".into()))
        );
    }

    #[test]
    fn test_get_returns_none_for_missing() {
        let src = ini_source("workers=10\n");
        assert!(src.get("missing").is_none());
    }

    #[test]
    fn test_as_fig_source_is_file_variant() {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(b"a=1\n").unwrap();
        let src = IniSource::load(f.path()).unwrap();
        assert!(matches!(src.as_fig_source(), FigSource::File(_)));
    }
}
