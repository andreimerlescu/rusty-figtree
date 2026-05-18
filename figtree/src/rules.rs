/// A Rule governs the behavior of a single Fig after it has been
/// resolved. Rules are applied per-key via the #[figtree(rule = ...)]
/// attribute. Only one rule may be applied per key.
///
/// Rules are evaluated before validators and callbacks. A rule
/// violation returns FigtreeError::RuleViolation immediately
/// without proceeding further.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rule {
    /// Default. No behavioral restriction. The key behaves normally
    /// in all circumstances.
    Undefined,

    /// Any call to store() on this key returns
    /// FigtreeError::RuleViolation. The value set at parse or load
    /// time is permanent for the lifetime of the tree.
    PreventChange,

    /// Any call to store() on this key panics immediately.
    /// Use when a changing value would represent a programming
    /// error rather than a user error.
    PanicOnChange,

    /// All validators registered on this key are skipped.
    /// The value is accepted regardless of what validators would
    /// have said. Does not affect callbacks or other rules.
    NoValidations,

    /// All callbacks registered on this key are skipped.
    /// Validators still run. Does not affect other rules.
    NoCallbacks,

    /// The CLI flag source is ignored for this key.
    /// Resolution falls through to the next source in priority order.
    NoFlags,

    /// The environment variable source is ignored for this key.
    /// Resolution falls through to the next source in priority order.
    NoEnv,

    /// Rejects with_rule() if the key holds a map type, and rejects
    /// store() calls that supply any map FigValue variant.
    /// Use to guarantee a key is never a map type.
    NoMaps,

    /// Rejects with_rule() if the key holds a list type, and rejects
    /// store() calls that supply any list FigValue variant.
    /// Use to guarantee a key is never a list type.
    NoLists,

    /// If a condemned key is accessed via resurrect(), the tree
    /// panics immediately. Used to permanently retire a key that
    /// must never reappear.
    CondemnedFromResurrection,
}

impl Rule {
    /// Returns true if this rule blocks store() operations.
    pub fn blocks_change(&self) -> bool {
        matches!(self, Rule::PreventChange | Rule::PanicOnChange)
    }

    /// Returns true if this rule skips validators.
    pub fn skips_validations(&self) -> bool {
        matches!(self, Rule::NoValidations)
    }

    /// Returns true if this rule skips callbacks.
    pub fn skips_callbacks(&self) -> bool {
        matches!(self, Rule::NoCallbacks)
    }

    /// Returns true if this rule ignores the CLI flag source.
    pub fn ignores_flags(&self) -> bool {
        matches!(self, Rule::NoFlags)
    }

    /// Returns true if this rule ignores the environment source.
    pub fn ignores_env(&self) -> bool {
        matches!(self, Rule::NoEnv)
    }

    /// Returns true if this rule blocks map types.
    pub fn blocks_maps(&self) -> bool {
        matches!(self, Rule::NoMaps)
    }

    /// Returns true if this rule blocks list types.
    pub fn blocks_lists(&self) -> bool {
        matches!(self, Rule::NoLists)
    }
}

impl Default for Rule {
    fn default() -> Self {
        Rule::Undefined
    }
}

impl std::fmt::Display for Rule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Rule::Undefined                  => "Undefined",
            Rule::PreventChange              => "PreventChange",
            Rule::PanicOnChange              => "PanicOnChange",
            Rule::NoValidations              => "NoValidations",
            Rule::NoCallbacks                => "NoCallbacks",
            Rule::NoFlags                    => "NoFlags",
            Rule::NoEnv                      => "NoEnv",
            Rule::NoMaps                     => "NoMaps",
            Rule::NoLists                    => "NoLists",
            Rule::CondemnedFromResurrection  => "CondemnedFromResurrection",
        };
        write!(f, "{}", name)
    }
}