use std::collections::HashMap;
use std::time::{Duration, SystemTime};

use crate::error::{FigtreeError, FigtreeResult};
use crate::rules::Rule;

// ── FigValue ──────────────────────────────────────────────────────────────────

/// The typed value container for a Fig.
///
/// Each variant corresponds to a mutagenesis type. Rust-native types
/// i128 and f128 are included beyond Go's i64/f64 ceiling.
/// List and Map variants carry typed element types.
#[derive(Debug, Clone, PartialEq)]
pub enum FigValue {
    // ── Scalar types ──────────────────────────────────────────────────────────
    String(String),
    Int(i32),
    Int64(i64),
    Int128(i128),
    Float64(f64),
    Float128(f64), // Rust does not have a native f128 yet; stored as f64
                   // with a semantic distinction. When Rust stabilizes f128
                   // this variant will be updated. The name is preserved for
                   // API consistency and forward compatibility.
    Bool(bool),
    Duration(Duration),

    // ── Typed list variants ───────────────────────────────────────────────────
    ListString(Vec<String>),
    ListInt(Vec<i32>),
    ListInt64(Vec<i64>),
    ListInt128(Vec<i128>),
    ListFloat64(Vec<f64>),
    ListFloat128(Vec<f64>), // same note as Float128 above
    ListBool(Vec<bool>),

    // ── Typed map variants ────────────────────────────────────────────────────
    // Key is always String. Value type varies by variant.
    MapString(HashMap<String, String>),
    MapInt(HashMap<String, i32>),
    MapInt64(HashMap<String, i64>),
    MapInt128(HashMap<String, i128>),
    MapFloat64(HashMap<String, f64>),
    MapFloat128(HashMap<String, f64>), // same note as Float128 above
    MapBool(HashMap<String, bool>),
}

impl FigValue {
    /// Returns the name of the mutagenesis type this value represents.
    /// Used in error messages and history entries.
    pub fn type_name(&self) -> &'static str {
        match self {
            FigValue::String(_)      => "String",
            FigValue::Int(_)         => "Int",
            FigValue::Int64(_)       => "Int64",
            FigValue::Int128(_)      => "Int128",
            FigValue::Float64(_)     => "Float64",
            FigValue::Float128(_)    => "Float128",
            FigValue::Bool(_)        => "Bool",
            FigValue::Duration(_)    => "Duration",
            FigValue::ListString(_)  => "ListString",
            FigValue::ListInt(_)     => "ListInt",
            FigValue::ListInt64(_)   => "ListInt64",
            FigValue::ListInt128(_)  => "ListInt128",
            FigValue::ListFloat64(_) => "ListFloat64",
            FigValue::ListFloat128(_)=> "ListFloat128",
            FigValue::ListBool(_)    => "ListBool",
            FigValue::MapString(_)   => "MapString",
            FigValue::MapInt(_)      => "MapInt",
            FigValue::MapInt64(_)    => "MapInt64",
            FigValue::MapInt128(_)   => "MapInt128",
            FigValue::MapFloat64(_)  => "MapFloat64",
            FigValue::MapFloat128(_) => "MapFloat128",
            FigValue::MapBool(_)     => "MapBool",
        }
    }

    /// Returns true if this value and the other are the same mutagenesis
    /// variant, regardless of the contained value.
    pub fn same_type(&self, other: &FigValue) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }
}

impl std::fmt::Display for FigValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FigValue::String(v)       => write!(f, "{}", v),
            FigValue::Int(v)          => write!(f, "{}", v),
            FigValue::Int64(v)        => write!(f, "{}", v),
            FigValue::Int128(v)       => write!(f, "{}", v),
            FigValue::Float64(v)      => write!(f, "{}", v),
            FigValue::Float128(v)     => write!(f, "{}", v),
            FigValue::Bool(v)         => write!(f, "{}", v),
            FigValue::Duration(v)     => write!(f, "{:?}", v),
            FigValue::ListString(v)   => write!(f, "[{}]", v.join(", ")),
            FigValue::ListInt(v)      => write!(f, "[{}]", v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", ")),
            FigValue::ListInt64(v)    => write!(f, "[{}]", v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", ")),
            FigValue::ListInt128(v)   => write!(f, "[{}]", v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", ")),
            FigValue::ListFloat64(v)  => write!(f, "[{}]", v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", ")),
            FigValue::ListFloat128(v) => write!(f, "[{}]", v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", ")),
            FigValue::ListBool(v)     => write!(f, "[{}]", v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", ")),
            FigValue::MapString(v)    => write!(f, "{{{}}}", v.iter().map(|(k, val)| format!("{}: {}", k, val)).collect::<Vec<_>>().join(", ")),
            FigValue::MapInt(v)       => write!(f, "{{{}}}", v.iter().map(|(k, val)| format!("{}: {}", k, val)).collect::<Vec<_>>().join(", ")),
            FigValue::MapInt64(v)     => write!(f, "{{{}}}", v.iter().map(|(k, val)| format!("{}: {}", k, val)).collect::<Vec<_>>().join(", ")),
            FigValue::MapInt128(v)    => write!(f, "{{{}}}", v.iter().map(|(k, val)| format!("{}: {}", k, val)).collect::<Vec<_>>().join(", ")),
            FigValue::MapFloat64(v)   => write!(f, "{{{}}}", v.iter().map(|(k, val)| format!("{}: {}", k, val)).collect::<Vec<_>>().join(", ")),
            FigValue::MapFloat128(v)  => write!(f, "{{{}}}", v.iter().map(|(k, val)| format!("{}: {}", k, val)).collect::<Vec<_>>().join(", ")),
            FigValue::MapBool(v)      => write!(f, "{{{}}}", v.iter().map(|(k, val)| format!("{}: {}", k, val)).collect::<Vec<_>>().join(", ")),
        }
    }
}

// ── FigSource ─────────────────────────────────────────────────────────────────

/// Identifies which configuration source provided a Fig's value.
///
/// Ordered by priority — higher discriminant means higher priority.
/// This ordering is the PEMDAS chain:
///   Flag > Environment > File > Programmatic > Default > Empty
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum FigSource {
    /// No source has provided a value yet. Every Fig starts here.
    Empty,

    /// Value came from the declared default at registration time.
    Default,

    /// Value was set programmatically via store() after parse or load.
    Programmatic,

    /// Value came from a configuration file. Carries the file path.
    File(String),

    /// Value came from an environment variable. Carries the variable name.
    Environment(String),

    /// Value came from a CLI flag. Carries the flag name.
    Flag(String),
}

impl std::fmt::Display for FigSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FigSource::Empty            => write!(f, "empty"),
            FigSource::Default          => write!(f, "default"),
            FigSource::Programmatic     => write!(f, "programmatic"),
            FigSource::File(path)       => write!(f, "file({})", path),
            FigSource::Environment(key) => write!(f, "env({})", key),
            FigSource::Flag(name)       => write!(f, "flag({})", name),
        }
    }
}

// ── FigState ──────────────────────────────────────────────────────────────────

/// The semantic state of a Fig at a point in its history.
///
/// States are numbered from zero and describe what happened at each
/// transition in plain language, suitable for logging and diagnostics.
#[derive(Debug, Clone, PartialEq)]
pub enum FigState {
    /// State 0 — the Fig was created with no value.
    /// This is always the first state in any Fig's history.
    Initialized,

    /// State N>0 — the Fig received its first value from a source.
    /// Carries the source that won the PEMDAS resolution.
    FirstResolution { source: FigSource },

    /// State N>1 — the Fig's value changed after first resolution.
    /// Carries the previous value, the new value, and the source
    /// of the change.
    Changed {
        from:   FigValue,
        to:     FigValue,
        source: FigSource,
    },

    /// The Fig's value was unchanged by an attempted set —
    /// the incoming value was identical to the current value.
    /// Recorded so the history is complete even for no-ops.
    Unchanged { source: FigSource },

    /// A validator rejected a value. The Fig's value was not changed.
    /// Carries the rejection message.
    ValidationRejected { message: String },

    /// A rule blocked a change. The Fig's value was not changed.
    /// Carries the rule that blocked it.
    RuleBlocked { rule: String },
}

impl std::fmt::Display for FigState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FigState::Initialized => {
                write!(f, "initialized with empty value")
            }
            FigState::FirstResolution { source } => {
                write!(f, "set for the first time via {}", source)
            }
            FigState::Changed { from, to, source } => {
                write!(f, "changed from '{}' to '{}' via {}", from, to, source)
            }
            FigState::Unchanged { source } => {
                write!(f, "unchanged (same value) via {}", source)
            }
            FigState::ValidationRejected { message } => {
                write!(f, "validation rejected: {}", message)
            }
            FigState::RuleBlocked { rule } => {
                write!(f, "blocked by rule '{}'", rule)
            }
        }
    }
}

// ── FigHistoryEntry ───────────────────────────────────────────────────────────

/// A single entry in a Fig's history log.
///
/// History is append-only. Every state transition appends one entry.
/// The sequence of entries tells the complete story of the Fig's life,
/// from initialization through every resolution, change, and rejection.
#[derive(Debug, Clone)]
pub struct FigHistoryEntry {
    /// Sequential state index, starting at 0 for initialization.
    pub state_index: usize,

    /// The semantic state at this point in history.
    pub state: FigState,

    /// The value at this point, if one existed.
    /// None for the initialization entry and for rejected transitions.
    pub value: Option<FigValue>,

    /// When this entry was recorded.
    pub recorded: SystemTime,
}

impl FigHistoryEntry {
    /// Creates the state-0 initialization entry.
    pub fn init() -> Self {
        FigHistoryEntry {
            state_index: 0,
            state:       FigState::Initialized,
            value:       None,
            recorded:    SystemTime::now(),
        }
    }

    /// Creates a state entry for a value transition.
    pub fn record(index: usize, state: FigState, value: Option<FigValue>) -> Self {
        FigHistoryEntry {
            state_index: index,
            state,
            value,
            recorded: SystemTime::now(),
        }
    }
}

impl std::fmt::Display for FigHistoryEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let recorded = self.recorded
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default();
        write!(
            f,
            "state {}: {} (at {}s)",
            self.state_index,
            self.state,
            recorded.as_secs()
        )
    }
}

// ── Fig ───────────────────────────────────────────────────────────────────────

/// A Fig is the container for a single configuration value.
///
/// It holds the current value, the default, the winning source,
/// the rule governing its behavior, and a complete append-only
/// history of every state it has passed through since initialization.
#[derive(Debug, Clone)]
pub struct Fig {
    /// The unique key identifying this Fig within its Tree.
    pub key: String,

    /// The current resolved value. None until resolution has run.
    pub value: Option<FigValue>,

    /// The default value declared at registration time.
    /// None if no default was declared — the key is then required.
    pub default: Option<FigValue>,

    /// Which source provided the current value.
    pub source: FigSource,

    /// The rule governing this Fig's behavior.
    pub rule: Rule,

    /// Complete append-only history of this Fig's state transitions.
    /// Entry at index 0 is always the initialization entry.
    pub history: Vec<FigHistoryEntry>,

    /// The last error recorded against this Fig, if any.
    /// Cleared on a successful set.
    pub error: Option<FigtreeError>,
}

impl Fig {
    /// Creates a new Fig at initialization state.
    /// History starts with a single state-0 initialization entry.
    pub fn new(key: impl Into<String>, default: Option<FigValue>) -> Self {
        Fig {
            key:     key.into(),
            value:   None,
            default,
            source:  FigSource::Empty,
            rule:    Rule::default(),
            history: vec![FigHistoryEntry::init()],
            error:   None,
        }
    }

    /// Returns the current value if resolved, otherwise the default.
    pub fn resolve(&self) -> Option<&FigValue> {
        self.value.as_ref().or(self.default.as_ref())
    }

    /// Returns true if this Fig has been resolved from any source.
    pub fn is_resolved(&self) -> bool {
        self.source != FigSource::Empty
    }

    /// Returns true if this Fig has no default and therefore
    /// must be provided by some source.
    pub fn is_required(&self) -> bool {
        self.default.is_none()
    }

    /// Returns the number of state transitions since initialization.
    /// The initialization entry at index 0 is not counted.
    pub fn change_count(&self) -> usize {
        self.history.len().saturating_sub(1)
    }

    /// Returns the full history as a slice.
    pub fn history(&self) -> &[FigHistoryEntry] {
        &self.history
    }

    /// Renders the complete history as a human-readable log string.
    /// Each line is one state entry.
    pub fn history_log(&self) -> String {
        self.history
            .iter()
            .map(|e| format!("  {}", e))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Attempts to set the current value from a given source.
    ///
    /// Enforces:
    ///   - Rule::PanicOnChange  — panics if already resolved
    ///   - Rule::PreventChange  — returns RuleViolation if already resolved
    ///   - Type consistency     — new value must match existing mutagenesis
    ///
    /// Records the appropriate FigState in history regardless of outcome.
    /// Returns Ok(()) on success, Err on rule violation or type mismatch.
    pub fn set(&mut self, value: FigValue, source: FigSource) -> FigtreeResult<()> {
        let next_index = self.history.len();

        // ── rule checks ───────────────────────────────────────────────────────
        if self.is_resolved() {
            if self.rule == Rule::PanicOnChange {
                self.history.push(FigHistoryEntry::record(
                    next_index,
                    FigState::RuleBlocked { rule: self.rule.to_string() },
                    None,
                ));
                panic!(
                    "figtree: PanicOnChange rule violated for key '{}'",
                    self.key
                );
            }
            if self.rule == Rule::PreventChange {
                self.history.push(FigHistoryEntry::record(
                    next_index,
                    FigState::RuleBlocked { rule: self.rule.to_string() },
                    None,
                ));
                return Err(FigtreeError::RuleViolation {
                    key:  self.key.clone(),
                    rule: self.rule.to_string(),
                });
            }
        }

        // ── type consistency check ─────────────────────────────────────────
        let existing_type = self.value.as_ref().or(self.default.as_ref());
        if let Some(existing) = existing_type {
            if !existing.same_type(&value) {
                let err = FigtreeError::TypeMismatch {
                    key:      self.key.clone(),
                    expected: existing.type_name().to_string(),
                    got:      value.type_name().to_string(),
                };
                self.error = Some(FigtreeError::TypeMismatch {
                    key:      self.key.clone(),
                    expected: existing.type_name().to_string(),
                    got:      value.type_name().to_string(),
                });
                return Err(err);
            }
        }

        // ── unchanged check ───────────────────────────────────────────────────
        if self.value.as_ref() == Some(&value) {
            self.history.push(FigHistoryEntry::record(
                next_index,
                FigState::Unchanged { source: source.clone() },
                Some(value),
            ));
            return Ok(());
        }

        // ── record the state transition ───────────────────────────────────────
        let state = if !self.is_resolved() {
            FigState::FirstResolution { source: source.clone() }
        } else {
            FigState::Changed {
                from:   self.value.clone().unwrap(),
                to:     value.clone(),
                source: source.clone(),
            }
        };

        self.history.push(FigHistoryEntry::record(
            next_index,
            state,
            Some(value.clone()),
        ));

        self.value  = Some(value);
        self.source = source;
        self.error  = None;

        Ok(())
    }

    /// Records a validation rejection in history without changing the value.
    pub fn record_validation_rejection(&mut self, message: impl Into<String>) {
        let message = message.into();
        let index   = self.history.len();
        self.history.push(FigHistoryEntry::record(
            index,
            FigState::ValidationRejected { message: message.clone() },
            None,
        ));
        self.error = Some(FigtreeError::ValidationFailed {
            key:     self.key.clone(),
            message,
        });
    }

    /// Records a rule block in history without changing the value.
    pub fn record_rule_block(&mut self, rule: impl Into<String>) {
        let rule  = rule.into();
        let index = self.history.len();
        self.history.push(FigHistoryEntry::record(
            index,
            FigState::RuleBlocked { rule: rule.clone() },
            None,
        ));
        self.error = Some(FigtreeError::RuleViolation {
            key:  self.key.clone(),
            rule,
        });
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn int_fig(default: i32) -> Fig {
        Fig::new("workers", Some(FigValue::Int(default)))
    }

    fn string_fig(default: &str) -> Fig {
        Fig::new("endpoint", Some(FigValue::String(default.into())))
    }

    // ── initialization ────────────────────────────────────────────────────────

    #[test]
    fn test_fig_state_0_is_initialized() {
        let fig = int_fig(10);
        assert_eq!(fig.history.len(), 1);
        assert_eq!(fig.history[0].state_index, 0);
        assert!(matches!(fig.history[0].state, FigState::Initialized));
        assert!(fig.history[0].value.is_none());
    }

    #[test]
    fn test_fig_starts_unresolved() {
        let fig = int_fig(10);
        assert!(!fig.is_resolved());
        assert_eq!(fig.source, FigSource::Empty);
    }

    #[test]
    fn test_fig_resolve_falls_back_to_default() {
        let fig = int_fig(10);
        assert_eq!(fig.resolve(), Some(&FigValue::Int(10)));
    }

    #[test]
    fn test_fig_required_when_no_default() {
        let fig = Fig::new("api_key", None);
        assert!(fig.is_required());
    }

    #[test]
    fn test_fig_not_required_when_default_present() {
        assert!(!int_fig(10).is_required());
    }

    // ── first resolution ──────────────────────────────────────────────────────

    #[test]
    fn test_fig_first_set_records_first_resolution_state() {
        let mut fig = int_fig(10);
        fig.set(FigValue::Int(20), FigSource::Environment("WORKERS".into())).unwrap();
        assert_eq!(fig.history.len(), 2);
        assert_eq!(fig.history[1].state_index, 1);
        assert!(matches!(
            fig.history[1].state,
            FigState::FirstResolution { source: FigSource::Environment(_) }
        ));
        assert_eq!(fig.value, Some(FigValue::Int(20)));
    }

    // ── subsequent changes ────────────────────────────────────────────────────

    #[test]
    fn test_fig_second_set_records_changed_state() {
        let mut fig = int_fig(10);
        fig.set(FigValue::Int(20), FigSource::File("config.yaml".into())).unwrap();
        fig.set(FigValue::Int(30), FigSource::Flag("workers".into())).unwrap();
        assert_eq!(fig.history.len(), 3);
        assert!(matches!(
            fig.history[2].state,
            FigState::Changed { .. }
        ));
        if let FigState::Changed { from, to, .. } = &fig.history[2].state {
            assert_eq!(from, &FigValue::Int(20));
            assert_eq!(to,   &FigValue::Int(30));
        }
    }

    #[test]
    fn test_fig_unchanged_set_records_unchanged_state() {
        let mut fig = int_fig(10);
        fig.set(FigValue::Int(20), FigSource::Environment("WORKERS".into())).unwrap();
        fig.set(FigValue::Int(20), FigSource::Programmatic).unwrap();
        assert_eq!(fig.history.len(), 3);
        assert!(matches!(fig.history[2].state, FigState::Unchanged { .. }));
        // value should still be 20
        assert_eq!(fig.value, Some(FigValue::Int(20)));
    }

    // ── history log ───────────────────────────────────────────────────────────

    #[test]
    fn test_history_log_is_readable() {
        let mut fig = string_fig("http://localhost");
        fig.set(FigValue::String("http://staging".into()), FigSource::File("config.yaml".into())).unwrap();
        fig.set(FigValue::String("http://prod".into()),    FigSource::Flag("endpoint".into())).unwrap();
        let log = fig.history_log();
        assert!(log.contains("state 0"));
        assert!(log.contains("state 1"));
        assert!(log.contains("state 2"));
        assert!(log.contains("http://staging"));
        assert!(log.contains("http://prod"));
    }

    #[test]
    fn test_change_count_excludes_initialization() {
        let mut fig = int_fig(10);
        assert_eq!(fig.change_count(), 0);
        fig.set(FigValue::Int(20), FigSource::Programmatic).unwrap();
        assert_eq!(fig.change_count(), 1);
        fig.set(FigValue::Int(30), FigSource::Programmatic).unwrap();
        assert_eq!(fig.change_count(), 2);
    }

    // ── type enforcement ──────────────────────────────────────────────────────

    #[test]
    fn test_type_mismatch_returns_error_and_preserves_value() {
        let mut fig = int_fig(10);
        let result = fig.set(FigValue::String("oops".into()), FigSource::Programmatic);
        assert!(matches!(result, Err(FigtreeError::TypeMismatch { .. })));
        assert!(fig.value.is_none());
    }

    // ── rules ─────────────────────────────────────────────────────────────────

    #[test]
    fn test_prevent_change_blocks_second_set() {
        let mut fig = int_fig(10);
        fig.rule = Rule::PreventChange;
        fig.set(FigValue::Int(20), FigSource::Environment("WORKERS".into())).unwrap();
        let result = fig.set(FigValue::Int(30), FigSource::Programmatic);
        assert!(matches!(result, Err(FigtreeError::RuleViolation { .. })));
        assert_eq!(fig.value, Some(FigValue::Int(20)));
        // rule block should be in history
        assert!(matches!(fig.history.last().unwrap().state, FigState::RuleBlocked { .. }));
    }

    #[test]
    fn test_panic_on_change_panics_on_second_set() {
        let mut fig = int_fig(10);
        fig.rule = Rule::PanicOnChange;
        fig.set(FigValue::Int(20), FigSource::Environment("WORKERS".into())).unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut fig2 = fig.clone();
            fig2.set(FigValue::Int(30), FigSource::Programmatic).unwrap();
        }));
        assert!(result.is_err());
    }

    // ── validation and rule recording ─────────────────────────────────────────

    #[test]
    fn test_record_validation_rejection_appends_to_history() {
        let mut fig = int_fig(10);
        fig.record_validation_rejection("value must be positive");
        assert!(matches!(
            fig.history.last().unwrap().state,
            FigState::ValidationRejected { .. }
        ));
        assert!(fig.error.is_some());
        assert!(fig.value.is_none()); // unchanged
    }

    #[test]
    fn test_record_rule_block_appends_to_history() {
        let mut fig = int_fig(10);
        fig.record_rule_block("PreventChange");
        assert!(matches!(
            fig.history.last().unwrap().state,
            FigState::RuleBlocked { .. }
        ));
    }

    // ── source ordering ───────────────────────────────────────────────────────

    #[test]
    fn test_source_priority_ordering() {
        assert!(FigSource::Flag("x".into())        > FigSource::Environment("x".into()));
        assert!(FigSource::Environment("x".into()) > FigSource::File("x".into()));
        assert!(FigSource::File("x".into())        > FigSource::Programmatic);
        assert!(FigSource::Programmatic            > FigSource::Default);
        assert!(FigSource::Default                 > FigSource::Empty);
    }

    // ── typed variants ────────────────────────────────────────────────────────

    #[test]
    fn test_int128_stores_and_retrieves() {
        let mut fig = Fig::new("big_number", Some(FigValue::Int128(0)));
        let big: i128 = i64::MAX as i128 + 1;
        fig.set(FigValue::Int128(big), FigSource::Programmatic).unwrap();
        assert_eq!(fig.value, Some(FigValue::Int128(big)));
    }

    #[test]
    fn test_list_int128_stores_correctly() {
        let mut fig = Fig::new("ids", Some(FigValue::ListInt128(vec![])));
        let ids: Vec<i128> = vec![i64::MAX as i128, i64::MAX as i128 + 999];
        fig.set(FigValue::ListInt128(ids.clone()), FigSource::Programmatic).unwrap();
        assert_eq!(fig.value, Some(FigValue::ListInt128(ids)));
    }

    #[test]
    fn test_same_type_distinguishes_int_from_int128() {
        assert!(!FigValue::Int(1).same_type(&FigValue::Int128(1)));
        assert!(FigValue::Int128(1).same_type(&FigValue::Int128(2)));
    }

    #[test]
    fn test_figvalue_type_names_are_correct() {
        assert_eq!(FigValue::Int128(0).type_name(),           "Int128");
        assert_eq!(FigValue::Float128(0.0).type_name(),       "Float128");
        assert_eq!(FigValue::ListInt128(vec![]).type_name(),  "ListInt128");
        assert_eq!(FigValue::MapInt128(HashMap::new()).type_name(), "MapInt128");
    }
}
