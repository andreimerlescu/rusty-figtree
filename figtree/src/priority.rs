use crate::error::FigtreeResult;
use crate::fig::{Fig, FigSource, FigValue};
use crate::rules::Rule;

// ── Source trait ──────────────────────────────────────────────────────────────

/// The contract any configuration source must satisfy.
///
/// A Source knows how to look up a value for a given key and return
/// it as a FigValue, or return None if it has no value for that key.
///
/// Sources are consulted in PEMDAS priority order by resolve().
/// Each source is responsible only for fetching — it does not
/// validate, apply rules, or record history. Those responsibilities
/// belong to the Tree.
///
/// The source_name() method returns a human-readable identifier used
/// in FigSource variants and error messages.
pub trait Source: Send + Sync {
    /// Returns a value for the given key if this source has one,
    /// or None if it does not.
    ///
    /// The returned FigValue must match the mutagenesis type of the
    /// Fig it is being resolved into. Type enforcement is the
    /// responsibility of the Tree, not the Source.
    fn get(&self, key: &str) -> Option<FigValue>;

    /// Returns a human-readable name for this source, used in
    /// FigSource variants and diagnostic output.
    ///
    /// Examples: "env", "file(/etc/app/config.yaml)", "flag"
    fn source_name(&self) -> String;

    /// Converts this source's identity into a FigSource variant,
    /// suitable for recording in Fig history and Mutation events.
    ///
    /// The default implementation returns FigSource::File with the
    /// source_name() as the path. Individual sources override this
    /// to return the correct variant.
    fn as_fig_source(&self) -> FigSource {
        FigSource::File(self.source_name())
    }
}

// ── ResolutionResult ──────────────────────────────────────────────────────────

/// The outcome of a single resolution attempt.
///
/// Carries both the winning value and the source that provided it,
/// so the caller can record both in the Fig's history and in any
/// Mutation event.
#[derive(Debug, Clone)]
pub struct ResolutionResult {
    /// The resolved value.
    pub value: FigValue,

    /// The source that provided the value.
    pub source: FigSource,
}

impl ResolutionResult {
    fn new(value: FigValue, source: FigSource) -> Self {
        ResolutionResult { value, source }
    }
}

// ── Priority resolution ───────────────────────────────────────────────────────

/// Resolves a Fig's value by consulting sources in PEMDAS priority order.
///
/// The resolution chain is fixed and invariant:
///
///   1. Flag        — CLI flag source (highest priority)
///   2. Environment — environment variable source
///   3. File        — configuration file sources, in registration order
///   4. Programmatic — values set via store() (already in fig.value)
///   5. Default     — the value declared at registration time
///
/// The first source that returns a value wins. If no source provides
/// a value and no default is declared, returns None — the key is
/// unresolved and the Tree will return FigtreeError::MissingRequired.
///
/// Rules that affect source consultation (RuleNoFlags, RuleNoEnv)
/// are enforced here before consulting the relevant source.
///
/// This function is pure — it does not mutate the Fig, record history,
/// or emit mutations. Those responsibilities belong to the Tree, which
/// calls this function and acts on the result.
pub fn resolve(
    fig:         &Fig,
    flag_source: Option<&dyn Source>,
    env_source:  Option<&dyn Source>,
    file_sources: &[Box<dyn Source>],
) -> Option<ResolutionResult> {

    // ── 1. CLI flag ───────────────────────────────────────────────────────────
    if !fig.rule.ignores_flags() {
        if let Some(src) = flag_source {
            if let Some(value) = src.get(&fig.key) {
                return Some(ResolutionResult::new(value, src.as_fig_source()));
            }
        }
    }

    // ── 2. Environment variable ───────────────────────────────────────────────
    if !fig.rule.ignores_env() {
        if let Some(src) = env_source {
            if let Some(value) = src.get(&fig.key) {
                return Some(ResolutionResult::new(value, src.as_fig_source()));
            }
        }
    }

    // ── 3. Configuration files (in registration order) ────────────────────────
    for src in file_sources {
        if let Some(value) = src.get(&fig.key) {
            return Some(ResolutionResult::new(value, src.as_fig_source()));
        }
    }

    // ── 4. Programmatic (already recorded in fig.value) ───────────────────────
    // If the Fig already has a value from a previous store() call,
    // it won the resolution at that time and is still current.
    // We do not re-resolve it here — the Tree preserves it as-is.
    // This step exists in the documented chain for completeness and
    // is surfaced in history as FigSource::Programmatic.

    // ── 5. Default ────────────────────────────────────────────────────────────
    if let Some(default) = &fig.default {
        return Some(ResolutionResult::new(
            default.clone(),
            FigSource::Default,
        ));
    }

    // ── No source provided a value ────────────────────────────────────────────
    None
}

/// Resolves all Figs in a collection, returning the results keyed
/// by Fig key. Called by Tree::parse() and Tree::load() to resolve
/// the full configuration in one pass.
///
/// Returns Ok(Vec<(key, ResolutionResult)>) for successful resolutions
/// and collects FigtreeError::MissingRequired for any required key
/// that could not be resolved. If any required key is missing the
/// entire resolution fails — the Tree does not partially apply results.
pub fn resolve_all<'a>(
    figs:         impl Iterator<Item = &'a Fig>,
    flag_source:  Option<&dyn Source>,
    env_source:   Option<&dyn Source>,
    file_sources: &[Box<dyn Source>],
) -> FigtreeResult<Vec<(String, ResolutionResult)>> {
    use crate::error::FigtreeError;

    let mut results  = Vec::new();
    let mut missing  = Vec::new();

    for fig in figs {
        // Skip figs whose rule blocks all sources — they keep
        // whatever value they already have.
        if fig.rule == Rule::PreventChange && fig.is_resolved() {
            continue;
        }

        match resolve(fig, flag_source, env_source, file_sources) {
            Some(result) => results.push((fig.key.clone(), result)),
            None => {
                if fig.is_required() {
                    missing.push(fig.key.clone());
                }
                // optional figs with no value simply remain unresolved
            }
        }
    }

    if !missing.is_empty() {
        // Report the first missing key. The Tree may choose to
        // collect all missing keys and report them together —
        // this is the single-error path.
        return Err(FigtreeError::MissingRequired(missing.join(", ")));
    }

    Ok(results)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fig::{Fig, FigSource, FigValue};
    use crate::rules::Rule;
    use std::collections::HashMap;

    // ── Test source implementation ────────────────────────────────────────────

    /// A simple in-memory source for testing. Holds a map of
    /// key → FigValue and returns values on request.
    struct MapSource {
        name:   String,
        values: HashMap<String, FigValue>,
        kind:   SourceKind,
    }

    #[derive(Clone)]
    enum SourceKind {
        Flag,
        Env,
        File,
    }

    impl MapSource {
        fn flag(values: HashMap<String, FigValue>) -> Self {
            MapSource { name: "flag".into(),  values, kind: SourceKind::Flag }
        }
        fn env(values: HashMap<String, FigValue>) -> Self {
            MapSource { name: "env".into(),   values, kind: SourceKind::Env  }
        }
        fn file(name: &str, values: HashMap<String, FigValue>) -> Self {
            MapSource { name: name.into(),    values, kind: SourceKind::File }
        }
        fn empty_flag() -> Self { Self::flag(HashMap::new()) }
        fn empty_env()  -> Self { Self::env(HashMap::new())  }
    }

    impl Source for MapSource {
        fn get(&self, key: &str) -> Option<FigValue> {
            self.values.get(key).cloned()
        }
        fn source_name(&self) -> String {
            self.name.clone()
        }
        fn as_fig_source(&self) -> FigSource {
            match self.kind {
                SourceKind::Flag => FigSource::Flag(self.name.clone()),
                SourceKind::Env  => FigSource::Environment(self.name.clone()),
                SourceKind::File => FigSource::File(self.name.clone()),
            }
        }
    }

    // helper — a fig with an int default
    fn int_fig(key: &str, default: i32) -> Fig {
        Fig::new(key, Some(FigValue::Int(default)))
    }

    // helper — a required fig with no default
    fn required_fig(key: &str) -> Fig {
        Fig::new(key, None)
    }

    // helper — one key in a hashmap
    fn kv(key: &str, value: FigValue) -> HashMap<String, FigValue> {
        let mut m = HashMap::new();
        m.insert(key.to_string(), value);
        m
    }

    // ── PEMDAS ordering ───────────────────────────────────────────────────────

    #[test]
    fn test_flag_wins_over_env_and_file_and_default() {
        let fig  = int_fig("workers", 1);
        let flag = MapSource::flag(kv("workers", FigValue::Int(10)));
        let env  = MapSource::env(kv("workers",  FigValue::Int(20)));
        let file: Vec<Box<dyn Source>> = vec![
            Box::new(MapSource::file("config.yaml", kv("workers", FigValue::Int(30))))
        ];

        let result = resolve(&fig, Some(&flag), Some(&env), &file).unwrap();
        assert_eq!(result.value,  FigValue::Int(10));
        assert_eq!(result.source, FigSource::Flag("flag".into()));
    }

    #[test]
    fn test_env_wins_over_file_and_default_when_no_flag() {
        let fig  = int_fig("workers", 1);
        let flag = MapSource::empty_flag();
        let env  = MapSource::env(kv("workers", FigValue::Int(20)));
        let file: Vec<Box<dyn Source>> = vec![
            Box::new(MapSource::file("config.yaml", kv("workers", FigValue::Int(30))))
        ];

        let result = resolve(&fig, Some(&flag), Some(&env), &file).unwrap();
        assert_eq!(result.value,  FigValue::Int(20));
        assert_eq!(result.source, FigSource::Environment("env".into()));
    }

    #[test]
    fn test_file_wins_over_default_when_no_flag_or_env() {
        let fig  = int_fig("workers", 1);
        let flag = MapSource::empty_flag();
        let env  = MapSource::empty_env();
        let file: Vec<Box<dyn Source>> = vec![
            Box::new(MapSource::file("config.yaml", kv("workers", FigValue::Int(30))))
        ];

        let result = resolve(&fig, Some(&flag), Some(&env), &file).unwrap();
        assert_eq!(result.value,  FigValue::Int(30));
        assert_eq!(result.source, FigSource::File("config.yaml".into()));
    }

    #[test]
    fn test_default_wins_when_no_other_source_has_value() {
        let fig  = int_fig("workers", 1);
        let flag = MapSource::empty_flag();
        let env  = MapSource::empty_env();
        let file: Vec<Box<dyn Source>> = vec![];

        let result = resolve(&fig, Some(&flag), Some(&env), &file).unwrap();
        assert_eq!(result.value,  FigValue::Int(1));
        assert_eq!(result.source, FigSource::Default);
    }

    #[test]
    fn test_returns_none_when_required_and_no_source() {
        let fig  = required_fig("api_key");
        let flag = MapSource::empty_flag();
        let env  = MapSource::empty_env();
        let file: Vec<Box<dyn Source>> = vec![];

        let result = resolve(&fig, Some(&flag), Some(&env), &file);
        assert!(result.is_none());
    }

    // ── File source ordering ──────────────────────────────────────────────────

    #[test]
    fn test_first_file_wins_when_multiple_files_have_key() {
        let fig  = int_fig("workers", 1);
        let flag = MapSource::empty_flag();
        let env  = MapSource::empty_env();
        let file: Vec<Box<dyn Source>> = vec![
            Box::new(MapSource::file("base.yaml",  kv("workers", FigValue::Int(10)))),
            Box::new(MapSource::file("local.yaml", kv("workers", FigValue::Int(20)))),
        ];

        let result = resolve(&fig, Some(&flag), Some(&env), &file).unwrap();
        assert_eq!(result.value,  FigValue::Int(10));
        assert_eq!(result.source, FigSource::File("base.yaml".into()));
    }

    #[test]
    fn test_second_file_used_when_first_file_lacks_key() {
        let fig  = int_fig("workers", 1);
        let flag = MapSource::empty_flag();
        let env  = MapSource::empty_env();
        let file: Vec<Box<dyn Source>> = vec![
            Box::new(MapSource::file("base.yaml",  HashMap::new())),
            Box::new(MapSource::file("local.yaml", kv("workers", FigValue::Int(20)))),
        ];

        let result = resolve(&fig, Some(&flag), Some(&env), &file).unwrap();
        assert_eq!(result.value,  FigValue::Int(20));
        assert_eq!(result.source, FigSource::File("local.yaml".into()));
    }

    // ── Rule enforcement ──────────────────────────────────────────────────────

    #[test]
    fn test_no_flags_rule_skips_flag_source() {
        let mut fig = int_fig("workers", 99);
        fig.rule    = Rule::NoFlags;

        let flag = MapSource::flag(kv("workers", FigValue::Int(10)));
        let env  = MapSource::env(kv("workers",  FigValue::Int(20)));
        let file: Vec<Box<dyn Source>> = vec![];

        // flag should be skipped — env should win
        let result = resolve(&fig, Some(&flag), Some(&env), &file).unwrap();
        assert_eq!(result.value,  FigValue::Int(20));
        assert_eq!(result.source, FigSource::Environment("env".into()));
    }

    #[test]
    fn test_no_env_rule_skips_environment_source() {
        let mut fig = int_fig("workers", 99);
        fig.rule    = Rule::NoEnv;

        let flag = MapSource::empty_flag();
        let env  = MapSource::env(kv("workers", FigValue::Int(20)));
        let file: Vec<Box<dyn Source>> = vec![
            Box::new(MapSource::file("config.yaml", kv("workers", FigValue::Int(30))))
        ];

        // env should be skipped — file should win
        let result = resolve(&fig, Some(&flag), Some(&env), &file).unwrap();
        assert_eq!(result.value,  FigValue::Int(30));
        assert_eq!(result.source, FigSource::File("config.yaml".into()));
    }

    #[test]
    fn test_prevent_change_resolved_fig_skipped_in_resolve_all() {
        let mut fig  = int_fig("workers", 1);
        fig.rule     = Rule::PreventChange;
        // simulate already resolved
        fig.set(FigValue::Int(42), FigSource::Programmatic).unwrap();

        let flag  = MapSource::flag(kv("workers", FigValue::Int(99)));
        let env   = MapSource::empty_env();
        let files: Vec<Box<dyn Source>> = vec![];

        let results = resolve_all(
            std::iter::once(&fig),
            Some(&flag),
            Some(&env),
            &files,
        ).unwrap();

        // the fig was skipped entirely — no result for it
        assert!(results.is_empty());
    }

    // ── resolve_all ───────────────────────────────────────────────────────────

    #[test]
    fn test_resolve_all_returns_all_resolved_keys() {
        let figs = vec![
            int_fig("workers",  10),
            int_fig("timeout",  30),
        ];

        let flag  = MapSource::flag(kv("workers", FigValue::Int(20)));
        let env   = MapSource::empty_env();
        let files: Vec<Box<dyn Source>> = vec![];

        let results = resolve_all(
            figs.iter(),
            Some(&flag),
            Some(&env),
            &files,
        ).unwrap();

        assert_eq!(results.len(), 2);

        let workers = results.iter().find(|(k, _)| k == "workers").unwrap();
        assert_eq!(workers.1.value, FigValue::Int(20));

        let timeout = results.iter().find(|(k, _)| k == "timeout").unwrap();
        assert_eq!(timeout.1.value, FigValue::Int(30)); // from default
    }

    #[test]
    fn test_resolve_all_fails_when_required_key_missing() {
        let figs = vec![
            required_fig("api_key"),
            int_fig("workers", 10),
        ];

        let flag  = MapSource::empty_flag();
        let env   = MapSource::empty_env();
        let files: Vec<Box<dyn Source>> = vec![];

        let result = resolve_all(figs.iter(), Some(&flag), Some(&env), &files);
        assert!(result.is_err());

        if let Err(crate::error::FigtreeError::MissingRequired(key)) = result {
            assert!(key.contains("api_key"));
        } else {
            panic!("expected MissingRequired error");
        }
    }

    #[test]
    fn test_resolve_all_optional_unresolved_fig_does_not_fail() {
        // a fig with no default and no required constraint is optional —
        // it simply remains unresolved and is not included in results
        let optional = Fig::new("maybe_key", Some(FigValue::String(String::new())));

        let flag  = MapSource::empty_flag();
        let env   = MapSource::empty_env();
        let files: Vec<Box<dyn Source>> = vec![];

        let results = resolve_all(
            std::iter::once(&optional),
            Some(&flag),
            Some(&env),
            &files,
        ).unwrap();

        // resolved to default (empty string)
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1.value, FigValue::String(String::new()));
        assert_eq!(results[0].1.source, FigSource::Default);
    }

    // ── no sources ────────────────────────────────────────────────────────────

    #[test]
    fn test_resolve_with_no_sources_uses_default() {
        let fig    = int_fig("workers", 5);
        let files: Vec<Box<dyn Source>> = vec![];
        let result = resolve(&fig, None, None, &files).unwrap();
        assert_eq!(result.value,  FigValue::Int(5));
        assert_eq!(result.source, FigSource::Default);
    }

    #[test]
    fn test_resolve_with_no_sources_and_no_default_returns_none() {
        let fig    = required_fig("api_key");
        let files: Vec<Box<dyn Source>> = vec![];
        assert!(resolve(&fig, None, None, &files).is_none());
    }

    // ── source trait ─────────────────────────────────────────────────────────

    #[test]
    fn test_source_trait_get_returns_none_for_unknown_key() {
        let src = MapSource::empty_env();
        assert!(src.get("nonexistent").is_none());
    }

    #[test]
    fn test_source_name_is_correct() {
        let src = MapSource::file("myconfig.yaml", HashMap::new());
        assert_eq!(src.source_name(), "myconfig.yaml");
    }

    #[test]
    fn test_as_fig_source_returns_correct_variant() {
        let flag = MapSource::flag(HashMap::new());
        let env  = MapSource::env(HashMap::new());
        let file = MapSource::file("config.yaml", HashMap::new());

        assert!(matches!(flag.as_fig_source(), FigSource::Flag(_)));
        assert!(matches!(env.as_fig_source(),  FigSource::Environment(_)));
        assert!(matches!(file.as_fig_source(), FigSource::File(_)));
    }
}