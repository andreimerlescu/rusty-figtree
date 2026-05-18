#[cfg(feature = "dotenv")]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use crate::error::{FigtreeError, FigtreeResult};
use crate::fig::{FigSource, FigValue};
use crate::priority::Source;
use crate::sources::env::parse_env_value;

/// A configuration source backed by a .env file.
///
/// Dotenv files are ubiquitous in twelve-factor applications and local
/// development environments. The format is a flat list of KEY=VALUE
/// pairs, one per line, with support for comments (#) and quoted values.
///
/// Uses the dotenvy crate, which is the actively maintained successor
/// to the unmaintained dotenv crate. dotenvy correctly handles quoted
/// values, escaped characters, and multiline values.
///
/// DotenvSource reads the file into an internal map at construction
/// time. It does NOT load values into the process environment — values
/// are kept in the internal map and resolved only when the Tree
/// requests them. This is intentional: loading into the environment
/// would affect other parts of the application and violate the
/// principle of source isolation.
///
/// DotenvSource is a string-origin source. parse_env_value() drives
/// typed conversion using the hint, the same as EnvSource.
pub struct DotenvSource {
    path:   PathBuf,
    values: HashMap<String, String>,
}

impl DotenvSource {
    /// Loads and parses a .env file without modifying the process
    /// environment.
    pub fn load(path: impl AsRef<Path>) -> FigtreeResult<Self> {
        let path = path.as_ref().to_path_buf();
        let iter = dotenvy::from_path_iter(&path).map_err(|_| {
            FigtreeError::FileNotFound(path.display().to_string())
        })?;

        let mut values = HashMap::new();
        for item in iter {
            let (key, val) = item.map_err(|e| FigtreeError::FileParseFailed {
                path:   path.display().to_string(),
                reason: e.to_string(),
            })?;
            values.insert(key, val);
        }

        Ok(DotenvSource { path, values })
    }

    /// Returns a typed FigValue for the given key.
    pub fn get_typed(&self, key: &str, hint: &FigValue) -> Option<FigValue> {
        let raw = self.values.get(key)?;
        parse_env_value(raw, hint)
    }
}

impl Source for DotenvSource {
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

    fn dotenv_source(contents: &str) -> DotenvSource {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(contents.as_bytes()).unwrap();
        DotenvSource::load(f.path()).unwrap()
    }

    #[test]
    fn test_load_valid_dotenv() {
        let src = dotenv_source("WORKERS=10\nENDPOINT=http://localhost\n");
        assert!(src.get("WORKERS").is_some());
    }

    #[test]
    fn test_get_returns_string() {
        let src = dotenv_source("WORKERS=10\n");
        assert_eq!(src.get("WORKERS"), Some(FigValue::String("10".into())));
    }

    #[test]
    fn test_get_typed_int() {
        let src = dotenv_source("WORKERS=10\n");
        assert_eq!(
            src.get_typed("WORKERS", &FigValue::Int(0)),
            Some(FigValue::Int(10))
        );
    }

    #[test]
    fn test_get_typed_bool() {
        let src = dotenv_source("DEBUG=true\n");
        assert_eq!(
            src.get_typed("DEBUG", &FigValue::Bool(false)),
            Some(FigValue::Bool(true))
        );
    }

    #[test]
    fn test_comment_lines_are_ignored() {
        let src = dotenv_source("# this is a comment\nWORKERS=5\n");
        assert_eq!(src.get("WORKERS"), Some(FigValue::String("5".into())));
        assert!(src.get("# this is a comment").is_none());
    }

    #[test]
    fn test_does_not_leak_into_process_environment() {
        let key = "FIGTREE_DOTENV_ISOLATION_TEST_XYZ";
        let src = dotenv_source(&format!("{}=leaked\n", key));
        assert_eq!(src.get(key), Some(FigValue::String("leaked".into())));
        // the process environment must not have been modified
        assert!(std::env::var(key).is_err());
    }

    #[test]
    fn test_get_returns_none_for_missing() {
        let src = dotenv_source("WORKERS=10\n");
        assert!(src.get("MISSING").is_none());
    }

    #[test]
    fn test_load_nonexistent_returns_error() {
        assert!(matches!(
            DotenvSource::load("/no/such/.env"),
            Err(FigtreeError::FileNotFound(_))
        ));
    }

    #[test]
    fn test_as_fig_source_is_file_variant() {
        let src = dotenv_source("A=1\n");
        assert!(matches!(src.as_fig_source(), FigSource::File(_)));
    }
}
