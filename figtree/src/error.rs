use std::fmt;

/// The canonical error type for figtree.
///
/// Every fallible operation in the crate returns FigtreeError.
/// Consumers match on variants to handle specific failure modes.
#[derive(Debug)]
pub enum FigtreeError {

    /// A required configuration key had no value from any source
    /// and no default was declared.
    MissingRequired(String),

    /// A value was present but could not be parsed into the
    /// declared type. Carries the key name and the raw string
    /// that failed to parse.
    ParseFailed {
        key: String,
        raw: String,
        reason: String,
    },

    /// A validator rejected the resolved value for a key.
    /// Carries the key name and the validator's rejection message.
    ValidationFailed {
        key: String,
        message: String,
    },

    /// A callback returned an error.
    /// Carries the key name, which callback phase triggered it,
    /// and the underlying error message.
    CallbackFailed {
        key:     String,
        phase:   String,
        message: String,
    },

    /// A configuration file could not be read or did not exist.
    FileNotFound(String),

    /// A configuration file existed but could not be parsed.
    /// Carries the file path and the parse error detail.
    FileParseFailed {
        path:   String,
        reason: String,
    },

    /// A rule blocked an attempted operation on a key.
    /// Carries the key name and which rule blocked it.
    RuleViolation {
        key:  String,
        rule: String,
    },

    /// An operation was attempted on a key that has not been
    /// registered on the tree.
    UnknownKey(String),

    /// A key was registered more than once on the same tree.
    DuplicateKey(String),

    /// A store operation was attempted on a key whose type
    /// did not match the declared mutagenesis.
    TypeMismatch {
        key:      String,
        expected: String,
        got:      String,
    },

    /// Wraps any error that does not fit a more specific variant.
    /// Used sparingly — prefer a specific variant where possible.
    Other(String),
}

impl fmt::Display for FigtreeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FigtreeError::MissingRequired(key) => {
                write!(f, "required key '{}' has no value and no default", key)
            }
            FigtreeError::ParseFailed { key, raw, reason } => {
                write!(f, "key '{}': could not parse '{}': {}", key, raw, reason)
            }
            FigtreeError::ValidationFailed { key, message } => {
                write!(f, "key '{}' failed validation: {}", key, message)
            }
            FigtreeError::CallbackFailed { key, phase, message } => {
                write!(f, "key '{}' callback '{}' failed: {}", key, phase, message)
            }
            FigtreeError::FileNotFound(path) => {
                write!(f, "config file not found: '{}'", path)
            }
            FigtreeError::FileParseFailed { path, reason } => {
                write!(f, "config file '{}' could not be parsed: {}", path, reason)
            }
            FigtreeError::RuleViolation { key, rule } => {
                write!(f, "key '{}' blocked by rule '{}'", key, rule)
            }
            FigtreeError::UnknownKey(key) => {
                write!(f, "unknown key '{}'", key)
            }
            FigtreeError::DuplicateKey(key) => {
                write!(f, "key '{}' was registered more than once", key)
            }
            FigtreeError::TypeMismatch { key, expected, got } => {
                write!(
                    f,
                    "key '{}': type mismatch — expected '{}', got '{}'",
                    key, expected, got
                )
            }
            FigtreeError::Other(msg) => {
                write!(f, "figtree error: {}", msg)
            }
        }
    }
}

impl std::error::Error for FigtreeError {}

/// Convenience type alias used throughout the crate.
pub type FigtreeResult<T> = Result<T, FigtreeError>;
