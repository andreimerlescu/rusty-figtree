use std::fmt;

/// The canonical error type for figtree.
#[derive(Debug, Clone)]
pub enum FigtreeError {
    MissingRequired(String),
    ParseFailed {
        key:    String,
        raw:    String,
        reason: String,
    },
    ValidationFailed {
        key:     String,
        message: String,
    },
    CallbackFailed {
        key:     String,
        phase:   String,
        message: String,
    },
    FileNotFound(String),
    FileParseFailed {
        path:   String,
        reason: String,
    },
    RuleViolation {
        key:  String,
        rule: String,
    },
    UnknownKey(String),
    DuplicateKey(String),
    TypeMismatch {
        key:      String,
        expected: String,
        got:      String,
    },
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

pub type FigtreeResult<T> = Result<T, FigtreeError>;
