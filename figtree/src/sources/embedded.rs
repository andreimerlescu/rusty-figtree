#[cfg(feature = "embedded")]

use crate::fig::{FigSource, FigValue};
use crate::priority::Source;

/// A configuration source for bare metal embedded targets.
///
/// This source is a stub designed to be extended by the consuming
/// application. It provides the Source trait interface over any
/// key-value store accessible without the standard library —
/// EEPROM, flash memory, a serial interface, or any other
/// hardware-backed storage.
///
/// Usage: implement the read() method for your specific hardware
/// by constructing an EmbeddedSource with a custom reader closure.
/// The reader receives a key and returns a raw byte slice that
/// EmbeddedSource converts to a FigValue using the hint type.
///
/// This module compiles only when the "embedded" feature is enabled.
/// It does not import or depend on std::fs, std::env, or any other
/// std facility that may not be available on a no_std target.
pub struct EmbeddedSource {
    /// Human-readable name for diagnostics.
    name: &'static str,

    /// The reader function. Receives a key and returns a raw UTF-8
    /// byte slice if the key exists, or None if it does not.
    ///
    /// The caller is responsible for the lifetime of returned bytes.
    /// For EEPROM or flash this is typically a fixed-address read.
    reader: Box<dyn Fn(&str) -> Option<&'static str> + Send + Sync>,
}

impl EmbeddedSource {
    /// Creates an EmbeddedSource with a custom reader.
    ///
    /// Example for a statically compiled config table:
    ///
    ///     const CONFIG: &[(&str, &str)] = &[
    ///         ("pid_kp", "0.8"),
    ///         ("pid_ki", "0.2"),
    ///     ];
    ///
    ///     let src = EmbeddedSource::new("flash", |key| {
    ///         CONFIG.iter()
    ///             .find(|(k, _)| *k == key)
    ///             .map(|(_, v)| *v)
    ///     });
    pub fn new(
        name:   &'static str,
        reader: impl Fn(&str) -> Option<&'static str> + Send + Sync + 'static,
    ) -> Self {
        EmbeddedSource {
            name,
            reader: Box::new(reader),
        }
    }

    /// Returns a typed FigValue for the given key using the hint
    /// to determine which mutagenesis variant to parse into.
    ///
    /// Uses the same string parsing logic as EnvSource since
    /// embedded config values are typically stored as strings.
    #[cfg(not(feature = "embedded"))]
    pub fn get_typed(&self, _key: &str, _hint: &FigValue) -> Option<FigValue> {
        None
    }

    #[cfg(feature = "embedded")]
    pub fn get_typed(&self, key: &str, hint: &FigValue) -> Option<FigValue> {
        let raw = (self.reader)(key)?;
        // Reuse env parsing — both are string-origin sources
        crate::sources::env::parse_env_value(raw, hint)
    }
}

impl Source for EmbeddedSource {
    fn get(&self, key: &str) -> Option<FigValue> {
        (self.reader)(key).map(|v| FigValue::String(v.to_string()))
    }

    fn source_name(&self) -> String {
        self.name.to_string()
    }

    fn as_fig_source(&self) -> FigSource {
        FigSource::File(self.name.to_string())
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const STATIC_CONFIG: &[(&str, &str)] = &[
        ("pid_kp",      "0.8"),
        ("pid_ki",      "0.2"),
        ("max_velocity","1.5"),
        ("enabled",     "true"),
        ("channels",    "1,2,3"),
    ];

    fn test_source() -> EmbeddedSource {
        EmbeddedSource::new("flash", |key| {
            STATIC_CONFIG.iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| *v)
        })
    }

    #[test]
    fn test_get_returns_string_for_known_key() {
        let src = test_source();
        assert_eq!(src.get("pid_kp"), Some(FigValue::String("0.8".into())));
    }

    #[test]
    fn test_get_returns_none_for_unknown_key() {
        let src = test_source();
        assert!(src.get("nonexistent").is_none());
    }

    #[test]
    fn test_get_typed_float64() {
        let src = test_source();
        assert_eq!(
            src.get_typed("pid_kp", &FigValue::Float64(0.0)),
            Some(FigValue::Float64(0.8))
        );
    }

    #[test]
    fn test_get_typed_bool() {
        let src = test_source();
        assert_eq!(
            src.get_typed("enabled", &FigValue::Bool(false)),
            Some(FigValue::Bool(true))
        );
    }

    #[test]
    fn test_get_typed_list_int() {
        let src = test_source();
        assert_eq!(
            src.get_typed("channels", &FigValue::ListInt(vec![])),
            Some(FigValue::ListInt(vec![1, 2, 3]))
        );
    }

    #[test]
    fn test_source_name() {
        assert_eq!(test_source().source_name(), "flash");
    }

    #[test]
    fn test_as_fig_source() {
        assert!(matches!(test_source().as_fig_source(), FigSource::File(_)));
    }
}
