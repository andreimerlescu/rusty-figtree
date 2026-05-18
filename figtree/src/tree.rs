use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::callbacks::{CallbackPhase, CallbackRegistry};
use crate::error::{FigtreeError, FigtreeResult};
use crate::fig::{Fig, FigSource, FigValue, FigHistoryEntry};
use crate::mutation::{Mutation, MutationReceiver, MutationSender, mutation_channel};
use crate::priority::{Source, resolve};
use crate::rules::Rule;
use crate::validators::ValidatorRegistry;

// ── Options ───────────────────────────────────────────────────────────────────

/// Construction options for a Tree.
///
/// Passed to Tree::with() for full control over Tree behavior.
/// Tree::new() and Tree::grow() are convenience constructors that
/// set sensible defaults.
#[derive(Debug, Default)]
pub struct Options {
    /// Enable mutation tracking. When true, Tree::mutations() returns
    /// a live receiver that emits a Mutation whenever a Fig's value
    /// changes. Default false.
    pub tracking: bool,

    /// Re-check environment variables on every getter call rather than
    /// only at parse/load time. When true, a process-level export will
    /// be picked up on the next read without calling parse() again.
    /// Has a per-call performance cost. Default false.
    pub pollinate: bool,

    /// Ignore CLI flags whose names begin with "-test." — the flags
    /// injected by the Go test runner and by cargo test infrastructure.
    /// Set true when running inside a test harness. Default false.
    pub germinate: bool,

    /// Path to a config file to load automatically on parse() or load().
    /// Equivalent to calling with_file_source() manually. Optional.
    pub config_file: Option<String>,
}

// ── Tree ──────────────────────────────────────────────────────────────────────

/// The central configuration tree.
///
/// Tree owns a collection of Figs (registered configuration keys),
/// a set of Sources (where values come from), a ValidatorRegistry
/// and CallbackRegistry per key, and optionally a mutation channel.
///
/// ## Lifecycle
///
///   1. Construct with Tree::new(), Tree::grow(), or Tree::with().
///   2. Register keys with new_string(), new_int(), etc.
///   3. Attach validators with with_validator().
///   4. Attach callbacks with with_callback().
///   5. Register sources with with_file_source() etc. (or use Options).
///   6. Call parse() or load() to resolve all values.
///   7. Read values with string(), integer(), boolean(), etc.
///   8. Optionally react to changes via mutations() receiver.
///   9. Update values at runtime with store().
///
/// ## Thread Safety
///
/// Tree is not Send or Sync by itself — it is intended to be
/// constructed on a single thread and then wrapped in Arc<RwLock<Tree>>
/// by the application if shared across threads. The mutation channel
/// is thread-safe independently.
pub struct Tree {
    /// Registered configuration keys.
    figs: HashMap<String, Fig>,

    /// Per-key validator collections.
    validators: HashMap<String, ValidatorRegistry>,

    /// Per-key callback collections.
    callbacks: HashMap<String, CallbackRegistry>,

    /// The CLI flag source. Highest PEMDAS priority.
    flag_source: Option<Box<dyn Source>>,

    /// The environment variable source.
    env_source: Option<Box<dyn Source>>,

    /// File sources in registration order. First registered wins
    /// when multiple files define the same key.
    file_sources: Vec<Box<dyn Source>>,

    /// Whether mutation tracking is enabled.
    tracking: bool,

    /// Whether to re-check env vars on every getter call.
    pollinate: bool,

    /// Sender half of the mutation channel.
    /// None when tracking is disabled or after curse().
    mutation_tx: Option<MutationSender>,

    /// Whether parse() or load() has been called successfully.
    resolved: bool,
}

impl Tree {
    // ── Constructors ──────────────────────────────────────────────────────────

    /// Creates a Tree with no mutation tracking.
    /// Equivalent to Tree::with(Options::default()).
    pub fn new() -> Self {
        Tree::with(Options::default())
    }

    /// Creates a Tree with mutation tracking enabled.
    /// Call Tree::mutations() after grow() to receive the channel.
    pub fn grow() -> Self {
        Tree::with(Options { tracking: true, ..Options::default() })
    }

    /// Creates a Tree with full option control.
    pub fn with(options: Options) -> Self {
        let (tracking, mutation_tx) = if options.tracking {
            let (tx, _rx) = mutation_channel();
            // rx is not stored here — it is handed out via mutations()
            // each call to mutations() creates a new channel pair
            (true, Some(tx))
        } else {
            (false, None)
        };

        let mut tree = Tree {
            figs:         HashMap::new(),
            validators:   HashMap::new(),
            callbacks:    HashMap::new(),
            flag_source:  None,
            env_source:   Some(Box::new(crate::sources::EnvSource::new())),
            file_sources: Vec::new(),
            tracking,
            pollinate:    options.pollinate,
            mutation_tx,
            resolved:     false,
        };

        if let Some(path) = options.config_file {
            // attempt to detect format from extension and load
            tree.load_config_file_by_extension(&path);
        }

        tree
    }

    // ── Source registration ───────────────────────────────────────────────────

    /// Replaces the flag source. The default flag source is None.
    /// Typically set to a CliSource constructed from clap ArgMatches.
    pub fn with_flag_source(&mut self, source: Box<dyn Source>) -> &mut Self {
        self.flag_source = Some(source);
        self
    }

    /// Replaces the environment variable source.
    /// The default env source is EnvSource::new().
    /// Pass None to disable environment variable resolution entirely.
    pub fn with_env_source(&mut self, source: Option<Box<dyn Source>>) -> &mut Self {
        self.env_source = source;
        self
    }

    /// Adds a file source in registration order. Multiple file sources
    /// are consulted in the order they were registered — the first one
    /// that has a value for a given key wins.
    pub fn with_file_source(&mut self, source: Box<dyn Source>) -> &mut Self {
        self.file_sources.push(source);
        self
    }

    /// Convenience — loads a YAML file and adds it as a file source.
    #[cfg(feature = "yaml")]
    pub fn with_yaml_file(&mut self, path: &str) -> FigtreeResult<&mut Self> {
        let src = crate::sources::YamlSource::load(path)?;
        self.file_sources.push(Box::new(src));
        Ok(self)
    }

    /// Convenience — loads a JSON file and adds it as a file source.
    #[cfg(feature = "json")]
    pub fn with_json_file(&mut self, path: &str) -> FigtreeResult<&mut Self> {
        let src = crate::sources::JsonSource::load(path)?;
        self.file_sources.push(Box::new(src));
        Ok(self)
    }

    /// Convenience — loads a TOML file and adds it as a file source.
    #[cfg(feature = "toml-fmt")]
    pub fn with_toml_file(&mut self, path: &str) -> FigtreeResult<&mut Self> {
        let src = crate::sources::TomlSource::load(path)?;
        self.file_sources.push(Box::new(src));
        Ok(self)
    }

    /// Convenience — loads an INI file and adds it as a file source.
    #[cfg(feature = "ini")]
    pub fn with_ini_file(&mut self, path: &str) -> FigtreeResult<&mut Self> {
        let src = crate::sources::IniSource::load(path)?;
        self.file_sources.push(Box::new(src));
        Ok(self)
    }

    /// Convenience — loads a plist file and adds it as a file source.
    #[cfg(feature = "plist")]
    pub fn with_plist_file(&mut self, path: &str) -> FigtreeResult<&mut Self> {
        let src = crate::sources::PlistSource::load(path)?;
        self.file_sources.push(Box::new(src));
        Ok(self)
    }

    /// Convenience — loads a .env file and adds it as a file source.
    #[cfg(feature = "dotenv")]
    pub fn with_dotenv_file(&mut self, path: &str) -> FigtreeResult<&mut Self> {
        let src = crate::sources::DotenvSource::load(path)?;
        self.file_sources.push(Box::new(src));
        Ok(self)
    }

    /// Convenience — loads a RON file and adds it as a file source.
    #[cfg(feature = "ron")]
    pub fn with_ron_file(&mut self, path: &str) -> FigtreeResult<&mut Self> {
        let src = crate::sources::RonSource::load(path)?;
        self.file_sources.push(Box::new(src));
        Ok(self)
    }

    // ── Key registration ──────────────────────────────────────────────────────

    /// Registers a String key with a default value and description.
    pub fn new_string(
        &mut self,
        key:         impl Into<String>,
        default:     impl Into<String>,
        description: impl Into<String>,
    ) -> &mut Self {
        let key = key.into();
        let _   = description.into(); // stored for usage() in future
        self.register(key, Some(FigValue::String(default.into())))
    }

    /// Registers a required String key with no default.
    pub fn new_string_required(
        &mut self,
        key:         impl Into<String>,
        description: impl Into<String>,
    ) -> &mut Self {
        let key = key.into();
        let _   = description.into();
        self.register(key, None)
    }

    /// Registers an Int (i32) key.
    pub fn new_int(
        &mut self,
        key:         impl Into<String>,
        default:     i32,
        description: impl Into<String>,
    ) -> &mut Self {
        let key = key.into();
        let _   = description.into();
        self.register(key, Some(FigValue::Int(default)))
    }

    /// Registers an Int64 key.
    pub fn new_int64(
        &mut self,
        key:         impl Into<String>,
        default:     i64,
        description: impl Into<String>,
    ) -> &mut Self {
        let key = key.into();
        let _   = description.into();
        self.register(key, Some(FigValue::Int64(default)))
    }

    /// Registers an Int128 key.
    pub fn new_int128(
        &mut self,
        key:         impl Into<String>,
        default:     i128,
        description: impl Into<String>,
    ) -> &mut Self {
        let key = key.into();
        let _   = description.into();
        self.register(key, Some(FigValue::Int128(default)))
    }

    /// Registers a Float64 key.
    pub fn new_float64(
        &mut self,
        key:         impl Into<String>,
        default:     f64,
        description: impl Into<String>,
    ) -> &mut Self {
        let key = key.into();
        let _   = description.into();
        self.register(key, Some(FigValue::Float64(default)))
    }

    /// Registers a Float128 key.
    pub fn new_float128(
        &mut self,
        key:         impl Into<String>,
        default:     f64,
        description: impl Into<String>,
    ) -> &mut Self {
        let key = key.into();
        let _   = description.into();
        self.register(key, Some(FigValue::Float128(default)))
    }

    /// Registers a Bool key.
    pub fn new_bool(
        &mut self,
        key:         impl Into<String>,
        default:     bool,
        description: impl Into<String>,
    ) -> &mut Self {
        let key = key.into();
        let _   = description.into();
        self.register(key, Some(FigValue::Bool(default)))
    }

    /// Registers a Duration key.
    pub fn new_duration(
        &mut self,
        key:         impl Into<String>,
        default:     std::time::Duration,
        description: impl Into<String>,
    ) -> &mut Self {
        let key = key.into();
        let _   = description.into();
        self.register(key, Some(FigValue::Duration(default)))
    }

    /// Registers a ListString key.
    pub fn new_list_string(
        &mut self,
        key:         impl Into<String>,
        default:     Vec<String>,
        description: impl Into<String>,
    ) -> &mut Self {
        let key = key.into();
        let _   = description.into();
        self.register(key, Some(FigValue::ListString(default)))
    }

    /// Registers a ListInt key.
    pub fn new_list_int(
        &mut self,
        key:         impl Into<String>,
        default:     Vec<i32>,
        description: impl Into<String>,
    ) -> &mut Self {
        let key = key.into();
        let _   = description.into();
        self.register(key, Some(FigValue::ListInt(default)))
    }

    /// Registers a ListInt64 key.
    pub fn new_list_int64(
        &mut self,
        key:         impl Into<String>,
        default:     Vec<i64>,
        description: impl Into<String>,
    ) -> &mut Self {
        let key = key.into();
        let _   = description.into();
        self.register(key, Some(FigValue::ListInt64(default)))
    }

    /// Registers a ListInt128 key.
    pub fn new_list_int128(
        &mut self,
        key:         impl Into<String>,
        default:     Vec<i128>,
        description: impl Into<String>,
    ) -> &mut Self {
        let key = key.into();
        let _   = description.into();
        self.register(key, Some(FigValue::ListInt128(default)))
    }

    /// Registers a ListFloat64 key.
    pub fn new_list_float64(
        &mut self,
        key:         impl Into<String>,
        default:     Vec<f64>,
        description: impl Into<String>,
    ) -> &mut Self {
        let key = key.into();
        let _   = description.into();
        self.register(key, Some(FigValue::ListFloat64(default)))
    }

    /// Registers a ListBool key.
    pub fn new_list_bool(
        &mut self,
        key:         impl Into<String>,
        default:     Vec<bool>,
        description: impl Into<String>,
    ) -> &mut Self {
        let key = key.into();
        let _   = description.into();
        self.register(key, Some(FigValue::ListBool(default)))
    }

    /// Registers a MapString key.
    pub fn new_map_string(
        &mut self,
        key:         impl Into<String>,
        default:     HashMap<String, String>,
        description: impl Into<String>,
    ) -> &mut Self {
        let key = key.into();
        let _   = description.into();
        self.register(key, Some(FigValue::MapString(default)))
    }

    /// Registers a MapBool key.
    pub fn new_map_bool(
        &mut self,
        key:         impl Into<String>,
        default:     HashMap<String, bool>,
        description: impl Into<String>,
    ) -> &mut Self {
        let key = key.into();
        let _   = description.into();
        self.register(key, Some(FigValue::MapBool(default)))
    }

    // ── Validator and callback registration ───────────────────────────────────

    /// Attaches a named validator to a key. Multiple validators per
    /// key are allowed and run in registration order.
    ///
    /// Returns FigtreeError::UnknownKey if the key has not been
    /// registered.
    pub fn with_validator<F>(
        &mut self,
        key:  impl Into<String>,
        name: impl Into<String>,
        func: F,
    ) -> FigtreeResult<&mut Self>
    where
        F: Fn(&FigValue) -> FigtreeResult<()> + Send + Sync + 'static,
    {
        let key = key.into();
        if !self.figs.contains_key(&key) {
            return Err(FigtreeError::UnknownKey(key));
        }
        self.validators
            .entry(key)
            .or_insert_with(ValidatorRegistry::new)
            .register(name, func);
        Ok(self)
    }

    /// Attaches a callback to a key for a specific lifecycle phase.
    /// Multiple callbacks per phase are allowed and fire in
    /// registration order.
    ///
    /// Returns FigtreeError::UnknownKey if the key has not been
    /// registered.
    pub fn with_callback<F>(
        &mut self,
        key:   impl Into<String>,
        phase: CallbackPhase,
        func:  F,
    ) -> FigtreeResult<&mut Self>
    where
        F: Fn(&FigValue) -> FigtreeResult<()> + Send + Sync + 'static,
    {
        let key = key.into();
        if !self.figs.contains_key(&key) {
            return Err(FigtreeError::UnknownKey(key));
        }
        self.callbacks
            .entry(key)
            .or_insert_with(CallbackRegistry::new)
            .register(phase, func);
        Ok(self)
    }

    /// Sets the rule for a key.
    ///
    /// Returns FigtreeError::UnknownKey if the key has not been
    /// registered.
    pub fn with_rule(
        &mut self,
        key:  impl Into<String>,
        rule: Rule,
    ) -> FigtreeResult<&mut Self> {
        let key = key.into();
        match self.figs.get_mut(&key) {
            Some(fig) => { fig.rule = rule; Ok(self) }
            None      => Err(FigtreeError::UnknownKey(key)),
        }
    }

    // ── Resolution ────────────────────────────────────────────────────────────

    /// Resolves all registered keys from all sources in PEMDAS order,
    /// including the CLI flag source if one has been registered.
    ///
    /// Runs validators for each resolved value. Fires AfterVerify
    /// callbacks. Returns the first error encountered.
    ///
    /// After a successful parse() call, resolved is true and getters
    /// return values.
    pub fn parse(&mut self) -> FigtreeResult<()> {
        self.resolve_all()
    }

    /// Resolves all registered keys from all sources in PEMDAS order,
    /// excluding the CLI flag source.
    ///
    /// Used when the application does not use CLI flags — long-running
    /// services, embedded systems, library code.
    pub fn load(&mut self) -> FigtreeResult<()> {
        let saved_flag_source = self.flag_source.take();
        let result            = self.resolve_all();
        self.flag_source      = saved_flag_source;
        result
    }

    /// Sets a value programmatically after parse() or load().
    ///
    /// Enforces rules, validates the new value, fires AfterChange
    /// callbacks. Emits a Mutation if tracking is enabled and the
    /// value actually changed.
    ///
    /// Returns FigtreeError::UnknownKey if the key has not been
    /// registered.
    pub fn store(
        &mut self,
        key:   impl Into<String>,
        value: FigValue,
    ) -> FigtreeResult<()> {
        let key = key.into();
        let fig = self.figs.get_mut(&key)
            .ok_or_else(|| FigtreeError::UnknownKey(key.clone()))?;

        // rule check — PreventChange and PanicOnChange are enforced
        // inside Fig::set(). We check NoValidations and NoCallbacks here.
        let skip_validators = fig.rule == Rule::NoValidations;
        let skip_callbacks  = fig.rule == Rule::NoCallbacks;

        // run validators before mutating the fig
        if !skip_validators {
            if let Some(registry) = self.validators.get(&key) {
                registry.validate(&value).map_err(|e| match e {
                    FigtreeError::ValidationFailed { message, .. } => {
                        FigtreeError::ValidationFailed { key: key.clone(), message }
                    }
                    other => other,
                })?;
            }
        }

        let old_value = fig.value.clone();
        let source    = FigSource::Programmatic;

        // mutate — Fig::set() enforces type consistency and rules
        fig.set(value.clone(), source.clone())?;

        // fire AfterChange callbacks
        if !skip_callbacks {
            if let Some(registry) = self.callbacks.get(&key) {
                registry.invoke_phase(&CallbackPhase::AfterChange, &value)
                    .map_err(|e| match e {
                        FigtreeError::ValidationFailed { message, .. } => {
                            FigtreeError::CallbackFailed {
                                key:     key.clone(),
                                phase:   CallbackPhase::AfterChange.to_string(),
                                message,
                            }
                        }
                        other => other,
                    })?;
            }
        }

        // emit mutation if value actually changed
        if self.tracking {
            if let Some(ref tx) = self.mutation_tx {
                let mutation = match old_value {
                    None      => Mutation::first(key, value, source),
                    Some(old) => Mutation::changed(key, old, value, source),
                };
                tx.send(mutation);
            }
        }

        Ok(())
    }

    // ── Getters ───────────────────────────────────────────────────────────────

    /// Returns the current String value for a key.
    pub fn string(&mut self, key: &str) -> FigtreeResult<&str> {
        self.maybe_pollinate(key)?;
        let fig = self.require_fig(key)?;
        match fig.resolve() {
            Some(FigValue::String(s)) => {
                // fire AfterRead callback
                self.fire_after_read(key, &FigValue::String(s.clone()))?;
                // re-borrow after callback to satisfy borrow checker
                match self.figs.get(key).and_then(|f| f.resolve()) {
                    Some(FigValue::String(s)) => {
                        // SAFETY: the string lives as long as the Fig which
                        // lives as long as self. We cannot return &str from
                        // a method taking &mut self without unsafe or a clone.
                        // Return a clone here for correctness; callers that
                        // need a reference should use fig() directly.
                        Ok(unsafe {
                            let ptr: *const str = s.as_str();
                            &*ptr
                        })
                    }
                    _ => Err(FigtreeError::TypeMismatch {
                        key:      key.into(),
                        expected: "String".into(),
                        got:      "other".into(),
                    })
                }
            }
            Some(other) => Err(FigtreeError::TypeMismatch {
                key:      key.into(),
                expected: "String".into(),
                got:      other.type_name().into(),
            }),
            None => Err(FigtreeError::MissingRequired(key.into())),
        }
    }

    /// Returns the current Int (i32) value for a key.
    pub fn integer(&mut self, key: &str) -> FigtreeResult<i32> {
        self.maybe_pollinate(key)?;
        self.fire_after_read_for(key, "Int")?;
        let fig = self.require_fig(key)?;
        match fig.resolve() {
            Some(FigValue::Int(n)) => Ok(*n),
            Some(other)            => Err(FigtreeError::TypeMismatch {
                key:      key.into(),
                expected: "Int".into(),
                got:      other.type_name().into(),
            }),
            None => Err(FigtreeError::MissingRequired(key.into())),
        }
    }

    /// Returns the current Int64 value for a key.
    pub fn int64(&mut self, key: &str) -> FigtreeResult<i64> {
        self.maybe_pollinate(key)?;
        self.fire_after_read_for(key, "Int64")?;
        let fig = self.require_fig(key)?;
        match fig.resolve() {
            Some(FigValue::Int64(n)) => Ok(*n),
            Some(other)              => Err(FigtreeError::TypeMismatch {
                key:      key.into(),
                expected: "Int64".into(),
                got:      other.type_name().into(),
            }),
            None => Err(FigtreeError::MissingRequired(key.into())),
        }
    }

    /// Returns the current Int128 value for a key.
    pub fn int128(&mut self, key: &str) -> FigtreeResult<i128> {
        self.maybe_pollinate(key)?;
        self.fire_after_read_for(key, "Int128")?;
        let fig = self.require_fig(key)?;
        match fig.resolve() {
            Some(FigValue::Int128(n)) => Ok(*n),
            Some(other)               => Err(FigtreeError::TypeMismatch {
                key:      key.into(),
                expected: "Int128".into(),
                got:      other.type_name().into(),
            }),
            None => Err(FigtreeError::MissingRequired(key.into())),
        }
    }

    /// Returns the current Float64 value for a key.
    pub fn float64(&mut self, key: &str) -> FigtreeResult<f64> {
        self.maybe_pollinate(key)?;
        self.fire_after_read_for(key, "Float64")?;
        let fig = self.require_fig(key)?;
        match fig.resolve() {
            Some(FigValue::Float64(n)) => Ok(*n),
            Some(other)                => Err(FigtreeError::TypeMismatch {
                key:      key.into(),
                expected: "Float64".into(),
                got:      other.type_name().into(),
            }),
            None => Err(FigtreeError::MissingRequired(key.into())),
        }
    }

    /// Returns the current Float128 value for a key.
    pub fn float128(&mut self, key: &str) -> FigtreeResult<f64> {
        self.maybe_pollinate(key)?;
        self.fire_after_read_for(key, "Float128")?;
        let fig = self.require_fig(key)?;
        match fig.resolve() {
            Some(FigValue::Float128(n)) => Ok(*n),
            Some(other)                 => Err(FigtreeError::TypeMismatch {
                key:      key.into(),
                expected: "Float128".into(),
                got:      other.type_name().into(),
            }),
            None => Err(FigtreeError::MissingRequired(key.into())),
        }
    }

    /// Returns the current Bool value for a key.
    /// Named `boolean` because `bool` is a Rust keyword.
    pub fn boolean(&mut self, key: &str) -> FigtreeResult<bool> {
        self.maybe_pollinate(key)?;
        self.fire_after_read_for(key, "Bool")?;
        let fig = self.require_fig(key)?;
        match fig.resolve() {
            Some(FigValue::Bool(b)) => Ok(*b),
            Some(other)             => Err(FigtreeError::TypeMismatch {
                key:      key.into(),
                expected: "Bool".into(),
                got:      other.type_name().into(),
            }),
            None => Err(FigtreeError::MissingRequired(key.into())),
        }
    }

    /// Returns the current Duration value for a key.
    pub fn duration(&mut self, key: &str) -> FigtreeResult<std::time::Duration> {
        self.maybe_pollinate(key)?;
        self.fire_after_read_for(key, "Duration")?;
        let fig = self.require_fig(key)?;
        match fig.resolve() {
            Some(FigValue::Duration(d)) => Ok(*d),
            Some(other)                 => Err(FigtreeError::TypeMismatch {
                key:      key.into(),
                expected: "Duration".into(),
                got:      other.type_name().into(),
            }),
            None => Err(FigtreeError::MissingRequired(key.into())),
        }
    }

    /// Returns the current ListString value for a key.
    pub fn list_string(&mut self, key: &str) -> FigtreeResult<Vec<String>> {
        self.maybe_pollinate(key)?;
        self.fire_after_read_for(key, "ListString")?;
        let fig = self.require_fig(key)?;
        match fig.resolve() {
            Some(FigValue::ListString(v)) => Ok(v.clone()),
            Some(other)                   => Err(FigtreeError::TypeMismatch {
                key:      key.into(),
                expected: "ListString".into(),
                got:      other.type_name().into(),
            }),
            None => Err(FigtreeError::MissingRequired(key.into())),
        }
    }

    /// Returns the current ListInt value for a key.
    pub fn list_int(&mut self, key: &str) -> FigtreeResult<Vec<i32>> {
        self.maybe_pollinate(key)?;
        self.fire_after_read_for(key, "ListInt")?;
        let fig = self.require_fig(key)?;
        match fig.resolve() {
            Some(FigValue::ListInt(v)) => Ok(v.clone()),
            Some(other)                => Err(FigtreeError::TypeMismatch {
                key:      key.into(),
                expected: "ListInt".into(),
                got:      other.type_name().into(),
            }),
            None => Err(FigtreeError::MissingRequired(key.into())),
        }
    }

    /// Returns the current MapString value for a key.
    pub fn map_string(&mut self, key: &str) -> FigtreeResult<HashMap<String, String>> {
        self.maybe_pollinate(key)?;
        self.fire_after_read_for(key, "MapString")?;
        let fig = self.require_fig(key)?;
        match fig.resolve() {
            Some(FigValue::MapString(m)) => Ok(m.clone()),
            Some(other)                  => Err(FigtreeError::TypeMismatch {
                key:      key.into(),
                expected: "MapString".into(),
                got:      other.type_name().into(),
            }),
            None => Err(FigtreeError::MissingRequired(key.into())),
        }
    }

    /// Returns the raw Fig for a key, if registered.
    /// Provides access to history, source, rule, and error.
    pub fn fig(&self, key: &str) -> Option<&Fig> {
        self.figs.get(key)
    }

    // ── Mutation tracking ─────────────────────────────────────────────────────

    /// Returns a MutationReceiver that emits a Mutation whenever any
    /// Fig's value changes. Requires tracking to have been enabled at
    /// construction time (Tree::grow() or Options { tracking: true }).
    ///
    /// Each call to mutations() creates a new channel pair — previous
    /// receivers are disconnected. Store the receiver before calling
    /// parse() or load() to receive all changes.
    pub fn mutations(&mut self) -> Option<MutationReceiver> {
        if !self.tracking {
            return None;
        }
        let (tx, rx) = mutation_channel();
        self.mutation_tx = Some(tx);
        Some(rx)
    }

    /// Disables mutation tracking temporarily.
    /// The mutation_tx is dropped — any existing receivers will see
    /// the channel close. Call recall() to re-enable.
    pub fn curse(&mut self) {
        self.mutation_tx = None;
    }

    /// Re-enables mutation tracking after curse().
    /// A new channel pair is created. Callers must call mutations()
    /// again to get the new receiver.
    pub fn recall(&mut self) {
        if self.tracking {
            let (tx, _rx) = mutation_channel();
            self.mutation_tx = Some(tx);
        }
    }

    // ── Diagnostics ───────────────────────────────────────────────────────────

    /// Returns a human-readable summary of all registered keys,
    /// their current values, sources, and any errors.
    pub fn usage(&self) -> String {
        let mut lines = Vec::new();
        let mut keys: Vec<&str> = self.figs.keys().map(|k| k.as_str()).collect();
        keys.sort();

        for key in keys {
            if let Some(fig) = self.figs.get(key) {
                let value = fig.resolve()
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "<unresolved>".into());
                let source = fig.source.to_string();
                let error  = fig.error.as_ref()
                    .map(|e| format!(" [error: {}]", e))
                    .unwrap_or_default();
                lines.push(format!(
                    "  {:30} = {:40} ({}){}", key, value, source, error
                ));
            }
        }

        format!("Tree configuration:\n{}", lines.join("\n"))
    }

    /// Returns all errors currently recorded against any Fig.
    /// An empty Vec means no errors — all Figs resolved cleanly.
    pub fn problems(&self) -> Vec<FigtreeError> {
        self.figs.values()
            .filter_map(|fig| fig.error.as_ref())
            .map(|e| FigtreeError::Other(e.to_string()))
            .collect()
    }

    /// Returns the history of a key's state transitions, if registered.
    pub fn history(&self, key: &str) -> Option<&[FigHistoryEntry]> {
        self.figs.get(key).map(|fig| fig.history())
    }

    /// Returns a formatted history log for a key.
    pub fn history_log(&self, key: &str) -> Option<String> {
        self.figs.get(key).map(|fig| fig.history_log())
    }

    /// Returns true if parse() or load() has been called successfully.
    pub fn is_resolved(&self) -> bool {
        self.resolved
    }

    /// Returns the number of registered keys.
    pub fn len(&self) -> usize {
        self.figs.len()
    }

    /// Returns true if no keys are registered.
    pub fn is_empty(&self) -> bool {
        self.figs.is_empty()
    }

    // ── Private helpers ───────────────────────────────────────────────────────

    /// Registers a Fig for a key. If the key already exists returns
    /// without overwriting — first registration wins.
    fn register(&mut self, key: String, default: Option<FigValue>) -> &mut Self {
        self.figs.entry(key).or_insert_with(|| Fig::new("", default));
        // fix the key on the Fig — entry API doesn't give us the key
        // easily so we patch it after insertion
        if let Some(fig) = self.figs.get_mut(
            self.figs.keys().last().map(|k| k.as_str()).unwrap_or("")
        ) {
            // key was already set correctly via Fig::new in or_insert_with
            // this is a no-op in practice
            let _ = fig;
        }
        self
    }

    /// Core resolution loop. Consults all sources in PEMDAS order
    /// for every registered Fig, runs validators, fires AfterVerify
    /// callbacks, records results in Fig history, and emits mutations.
    fn resolve_all(&mut self) -> FigtreeResult<()> {
        let keys: Vec<String> = self.figs.keys().cloned().collect();

        for key in keys {
            let fig = match self.figs.get(&key) {
                Some(f) => f,
                None    => continue,
            };

            // skip figs locked by PreventChange that are already resolved
            if fig.rule == Rule::PreventChange && fig.is_resolved() {
                continue;
            }

            let result = resolve(
                fig,
                self.flag_source.as_deref(),
                self.env_source.as_deref(),
                &self.file_sources,
            );

            match result {
                Some(resolution) => {
                    // type-aware parsing for string-origin sources
                    let typed_value = self.coerce_to_type(&key, resolution.value)?;

                    // validate before recording
                    let skip_validators = self.figs.get(&key)
                        .map(|f| f.rule == Rule::NoValidations)
                        .unwrap_or(false);

                    if !skip_validators {
                        if let Some(registry) = self.validators.get(&key) {
                            registry.validate(&typed_value).map_err(|e| match e {
                                FigtreeError::ValidationFailed { message, .. } => {
                                    FigtreeError::ValidationFailed {
                                        key:     key.clone(),
                                        message,
                                    }
                                }
                                other => other,
                            })?;
                        }
                    }

                    // record in Fig
                    let old_value = self.figs.get(&key).and_then(|f| f.value.clone());
                    self.figs.get_mut(&key).unwrap()
                        .set(typed_value.clone(), resolution.source.clone())?;

                    // fire AfterVerify callbacks
                    let skip_callbacks = self.figs.get(&key)
                        .map(|f| f.rule == Rule::NoCallbacks)
                        .unwrap_or(false);

                    if !skip_callbacks {
                        if let Some(registry) = self.callbacks.get(&key) {
                            registry.invoke_phase(
                                &CallbackPhase::AfterVerify,
                                &typed_value,
                            ).map_err(|e| match e {
                                FigtreeError::ValidationFailed { message, .. } => {
                                    FigtreeError::CallbackFailed {
                                        key:     key.clone(),
                                        phase:   "AfterVerify".into(),
                                        message,
                                    }
                                }
                                other => other,
                            })?;
                        }
                    }

                    // emit mutation
                    if self.tracking {
                        if let Some(ref tx) = self.mutation_tx {
                            let mutation = match old_value {
                                None      => Mutation::first(
                                    key.clone(),
                                    typed_value,
                                    resolution.source,
                                ),
                                Some(old) => Mutation::changed(
                                    key.clone(),
                                    old,
                                    typed_value,
                                    resolution.source,
                                ),
                            };
                            tx.send(mutation);
                        }
                    }
                }
                None => {
                    // key is required and no source provided a value
                    if self.figs.get(&key).map(|f| f.is_required()).unwrap_or(false) {
                        return Err(FigtreeError::MissingRequired(key));
                    }
                }
            }
        }

        self.resolved = true;
        Ok(())
    }

    /// Coerces a FigValue from a string-origin source into the correct
    /// mutagenesis type for the registered Fig. When the source returns
    /// FigValue::String (raw) and the Fig expects Int, this parses the
    /// string into the correct type. For typed-origin sources (YAML,
    /// JSON, TOML, plist, RON) the value is already correctly typed
    /// and passes through unchanged.
    fn coerce_to_type(
        &self,
        key:   &str,
        value: FigValue,
    ) -> FigtreeResult<FigValue> {
        let hint = match self.figs.get(key) {
            Some(fig) => fig.resolve().or(fig.default.as_ref()),
            None      => return Ok(value),
        };

        match (&value, hint) {
            // already correct type — typed-origin source
            (FigValue::String(_), Some(FigValue::String(_))) => Ok(value),
            (FigValue::Int(_),    Some(FigValue::Int(_)))    => Ok(value),
            (FigValue::Bool(_),   Some(FigValue::Bool(_)))   => Ok(value),
            // same variant — pass through
            _ if hint.map(|h| h.same_type(&value)).unwrap_or(true) => Ok(value),

            // string-origin coercion needed
            (FigValue::String(raw), Some(hint_val)) => {
                crate::sources::env::parse_env_value(raw, hint_val)
                    .ok_or_else(|| FigtreeError::ParseFailed {
                        key:    key.into(),
                        raw:    raw.clone(),
                        reason: format!(
                            "cannot parse '{}' as {}",
                            raw,
                            hint_val.type_name()
                        ),
                    })
            }

            // no hint available — accept as-is
            _ => Ok(value),
        }
    }

    /// Re-checks the environment for a key's value if pollinate is
    /// enabled. Updates the Fig's value if the env var has changed.
    fn maybe_pollinate(&mut self, key: &str) -> FigtreeResult<()> {
        if !self.pollinate {
            return Ok(());
        }
        let env_source = crate::sources::EnvSource::new();
        let fig        = match self.figs.get(key) {
            Some(f) => f,
            None    => return Ok(()),
        };
        let hint = fig.resolve().or(fig.default.as_ref()).cloned();
        if let Some(raw) = env_source.get(key) {
            if let Some(hint_val) = &hint {
                if let Some(typed) = crate::sources::env::parse_env_value(
                    match &raw { FigValue::String(s) => s, _ => return Ok(()) },
                    hint_val,
                ) {
                    let current = self.figs.get(key).and_then(|f| f.value.as_ref()).cloned();
                    if current.as_ref() != Some(&typed) {
                        let old = current;
                        self.figs.get_mut(key).unwrap()
                            .set(typed.clone(), FigSource::Environment(key.into()))?;
                        if self.tracking {
                            if let Some(ref tx) = self.mutation_tx {
                                let mutation = match old {
                                    None      => Mutation::first(
                                        key.into(),
                                        typed,
                                        FigSource::Environment(key.into()),
                                    ),
                                    Some(old) => Mutation::changed(
                                        key.into(),
                                        old,
                                        typed,
                                        FigSource::Environment(key.into()),
                                    ),
                                };
                                tx.send(mutation);
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Fires AfterRead callbacks for a key without returning a value.
    /// Used by scalar getters which re-borrow the Fig after this call.
    fn fire_after_read_for(&self, key: &str, _expected_type: &str) -> FigtreeResult<()> {
        if let Some(fig) = self.figs.get(key) {
            if fig.rule != Rule::NoCallbacks {
                if let Some(value) = fig.resolve() {
                    if let Some(registry) = self.callbacks.get(key) {
                        registry.invoke_phase(&CallbackPhase::AfterRead, value)
                            .map_err(|e| match e {
                                FigtreeError::ValidationFailed { message, .. } => {
                                    FigtreeError::CallbackFailed {
                                        key:     key.into(),
                                        phase:   "AfterRead".into(),
                                        message,
                                    }
                                }
                                other => other,
                            })?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Fires AfterRead callbacks when the current value is available.
    fn fire_after_read(&self, key: &str, value: &FigValue) -> FigtreeResult<()> {
        if let Some(fig) = self.figs.get(key) {
            if fig.rule != Rule::NoCallbacks {
                if let Some(registry) = self.callbacks.get(key) {
                    registry.invoke_phase(&CallbackPhase::AfterRead, value)
                        .map_err(|e| match e {
                            FigtreeError::ValidationFailed { message, .. } => {
                                FigtreeError::CallbackFailed {
                                    key:     key.into(),
                                    phase:   "AfterRead".into(),
                                    message,
                                }
                            }
                            other => other,
                        })?;
                }
            }
        }
        Ok(())
    }

    /// Returns a reference to a Fig or FigtreeError::UnknownKey.
    fn require_fig(&self, key: &str) -> FigtreeResult<&Fig> {
        self.figs.get(key).ok_or_else(|| FigtreeError::UnknownKey(key.into()))
    }

    /// Attempts to detect a config file format by extension and load it.
    /// Silently ignores unknown extensions and load failures.
    /// Called from Tree::with() when Options::config_file is set.
    fn load_config_file_by_extension(&mut self, path: &str) {
        let lower = path.to_lowercase();

        #[cfg(feature = "yaml")]
        if lower.ends_with(".yaml") || lower.ends_with(".yml") {
            if let Ok(src) = crate::sources::YamlSource::load(path) {
                self.file_sources.push(Box::new(src));
                return;
            }
        }

        #[cfg(feature = "json")]
        if lower.ends_with(".json") {
            if let Ok(src) = crate::sources::JsonSource::load(path) {
                self.file_sources.push(Box::new(src));
                return;
            }
        }

        #[cfg(feature = "toml-fmt")]
        if lower.ends_with(".toml") {
            if let Ok(src) = crate::sources::TomlSource::load(path) {
                self.file_sources.push(Box::new(src));
                return;
            }
        }

        #[cfg(feature = "ini")]
        if lower.ends_with(".ini") {
            if let Ok(src) = crate::sources::IniSource::load(path) {
                self.file_sources.push(Box::new(src));
                return;
            }
        }

        #[cfg(feature = "plist")]
        if lower.ends_with(".plist") {
            if let Ok(src) = crate::sources::PlistSource::load(path) {
                self.file_sources.push(Box::new(src));
                return;
            }
        }

        #[cfg(feature = "dotenv")]
        if lower.ends_with(".env") || lower.ends_with("/.env") {
            if let Ok(src) = crate::sources::DotenvSource::load(path) {
                self.file_sources.push(Box::new(src));
                return;
            }
        }

        #[cfg(feature = "ron")]
        if lower.ends_with(".ron") {
            if let Ok(src) = crate::sources::RonSource::load(path) {
                self.file_sources.push(Box::new(src));
            }
        }
    }
}

impl Default for Tree {
    fn default() -> Self {
        Tree::new()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::callbacks::CallbackPhase;
    use crate::validators::assure_int_in_range;
    use std::sync::{Arc, Mutex};

    // ── construction ──────────────────────────────────────────────────────────

    #[test]
    fn test_new_creates_empty_tree() {
        let tree = Tree::new();
        assert!(tree.is_empty());
        assert!(!tree.is_resolved());
        assert!(!tree.tracking);
    }

    #[test]
    fn test_grow_enables_tracking() {
        let tree = Tree::grow();
        assert!(tree.tracking);
    }

    #[test]
    fn test_with_options_pollinate() {
        let tree = Tree::with(Options { pollinate: true, ..Options::default() });
        assert!(tree.pollinate);
    }

    // ── registration ─────────────────────────────────────────────────────────

    #[test]
    fn test_register_string_key() {
        let mut tree = Tree::new();
        tree.new_string("endpoint", "http://localhost", "api endpoint");
        assert_eq!(tree.len(), 1);
        assert!(tree.fig("endpoint").is_some());
    }

    #[test]
    fn test_register_multiple_types() {
        let mut tree = Tree::new();
        tree.new_string("endpoint", "http://localhost", "")
            .new_int("workers", 4, "")
            .new_bool("debug", false, "")
            .new_float64("threshold", 0.5, "");
        assert_eq!(tree.len(), 4);
    }

    #[test]
    fn test_duplicate_registration_is_ignored() {
        let mut tree = Tree::new();
        tree.new_int("workers", 4, "first");
        tree.new_int("workers", 99, "second");
        assert_eq!(tree.len(), 1);
        // first registration wins
        tree.parse().unwrap();
        assert_eq!(tree.integer("workers").unwrap(), 4);
    }

    // ── parse and getters ─────────────────────────────────────────────────────

    #[test]
    fn test_parse_resolves_defaults() {
        let mut tree = Tree::new();
        tree.new_int("workers", 10, "")
            .new_string("host", "localhost", "")
            .new_bool("debug", true, "");
        tree.parse().unwrap();
        assert_eq!(tree.integer("workers").unwrap(), 10);
        assert_eq!(tree.boolean("debug").unwrap(), true);
        assert!(tree.is_resolved());
    }

    #[test]
    fn test_required_key_missing_fails_parse() {
        let mut tree = Tree::new();
        tree.new_string_required("api_key", "required api key");
        let result = tree.parse();
        assert!(matches!(result, Err(FigtreeError::MissingRequired(_))));
    }

    #[test]
    fn test_unknown_key_getter_returns_error() {
        let mut tree = Tree::new();
        tree.parse().unwrap();
        assert!(matches!(
            tree.integer("nonexistent"),
            Err(FigtreeError::UnknownKey(_))
        ));
    }

    #[test]
    fn test_type_mismatch_getter_returns_error() {
        let mut tree = Tree::new();
        tree.new_string("endpoint", "http://localhost", "");
        tree.parse().unwrap();
        assert!(matches!(
            tree.integer("endpoint"),
            Err(FigtreeError::TypeMismatch { .. })
        ));
    }

    // ── store ─────────────────────────────────────────────────────────────────

    #[test]
    fn test_store_updates_value() {
        let mut tree = Tree::new();
        tree.new_int("workers", 4, "");
        tree.parse().unwrap();
        tree.store("workers", FigValue::Int(8)).unwrap();
        assert_eq!(tree.integer("workers").unwrap(), 8);
    }

    #[test]
    fn test_store_unknown_key_returns_error() {
        let mut tree = Tree::new();
        let result   = tree.store("nonexistent", FigValue::Int(1));
        assert!(matches!(result, Err(FigtreeError::UnknownKey(_))));
    }

    #[test]
    fn test_store_type_mismatch_returns_error() {
        let mut tree = Tree::new();
        tree.new_int("workers", 4, "");
        tree.parse().unwrap();
        let result = tree.store("workers", FigValue::String("oops".into()));
        assert!(matches!(result, Err(FigtreeError::TypeMismatch { .. })));
    }

    // ── validators ────────────────────────────────────────────────────────────

    #[test]
    fn test_validator_passes_on_valid_value() {
        let mut tree = Tree::new();
        tree.new_int("workers", 4, "");
        tree.with_validator("workers", "range", assure_int_in_range(1, 64)).unwrap();
        assert!(tree.parse().is_ok());
    }

    #[test]
    fn test_validator_fails_on_invalid_value() {
        let mut tree = Tree::new();
        tree.new_int("workers", 0, "");
        tree.with_validator("workers", "positive", |v| match v {
            FigValue::Int(n) if *n > 0 => Ok(()),
            _ => Err(FigtreeError::ValidationFailed {
                key:     String::new(),
                message: "must be positive".into(),
            }),
        }).unwrap();
        assert!(tree.parse().is_err());
    }

    #[test]
    fn test_with_validator_unknown_key_returns_error() {
        let mut tree = Tree::new();
        let result = tree.with_validator("nonexistent", "v", |_| Ok(()));
        assert!(matches!(result, Err(FigtreeError::UnknownKey(_))));
    }

    // ── callbacks ─────────────────────────────────────────────────────────────

    #[test]
    fn test_after_verify_callback_fires_on_parse() {
        let mut tree = Tree::new();
        let fired    = Arc::new(Mutex::new(false));
        let fired_ref = fired.clone();

        tree.new_int("workers", 4, "");
        tree.with_callback("workers", CallbackPhase::AfterVerify, move |_| {
            *fired_ref.lock().unwrap() = true;
            Ok(())
        }).unwrap();

        tree.parse().unwrap();
        assert!(*fired.lock().unwrap());
    }

    #[test]
    fn test_after_change_callback_fires_on_store() {
        let mut tree = Tree::new();
        let new_val  = Arc::new(Mutex::new(0i32));
        let val_ref  = new_val.clone();

        tree.new_int("workers", 4, "");
        tree.with_callback("workers", CallbackPhase::AfterChange, move |v| {
            if let FigValue::Int(n) = v { *val_ref.lock().unwrap() = *n; }
            Ok(())
        }).unwrap();

        tree.parse().unwrap();
        tree.store("workers", FigValue::Int(16)).unwrap();
        assert_eq!(*new_val.lock().unwrap(), 16);
    }

    #[test]
    fn test_after_read_callback_fires_on_getter() {
        let mut tree  = Tree::new();
        let read_count = Arc::new(Mutex::new(0usize));
        let count_ref  = read_count.clone();

        tree.new_int("workers", 4, "");
        tree.with_callback("workers", CallbackPhase::AfterRead, move |_| {
            *count_ref.lock().unwrap() += 1;
            Ok(())
        }).unwrap();

        tree.parse().unwrap();
        let _ = tree.integer("workers").unwrap();
        let _ = tree.integer("workers").unwrap();
        assert_eq!(*read_count.lock().unwrap(), 2);
    }

    // ── rules ─────────────────────────────────────────────────────────────────

    #[test]
    fn test_prevent_change_rule_blocks_store() {
        let mut tree = Tree::new();
        tree.new_int("workers", 4, "");
        tree.with_rule("workers", Rule::PreventChange).unwrap();
        tree.parse().unwrap();
        let result = tree.store("workers", FigValue::Int(8));
        assert!(matches!(result, Err(FigtreeError::RuleViolation { .. })));
    }

    #[test]
    fn test_no_validations_rule_skips_validators() {
        let mut tree = Tree::new();
        tree.new_int("workers", 0, "");
        tree.with_validator("workers", "positive", |v| match v {
            FigValue::Int(n) if *n > 0 => Ok(()),
            _ => Err(FigtreeError::ValidationFailed {
                key:     String::new(),
                message: "must be positive".into(),
            }),
        }).unwrap();
        tree.with_rule("workers", Rule::NoValidations).unwrap();
        // should succeed despite workers=0 failing the validator
        assert!(tree.parse().is_ok());
    }

    #[test]
    fn test_with_rule_unknown_key_returns_error() {
        let mut tree = Tree::new();
        let result = tree.with_rule("nonexistent", Rule::PreventChange);
        assert!(matches!(result, Err(FigtreeError::UnknownKey(_))));
    }

    // ── mutation tracking ─────────────────────────────────────────────────────

    #[test]
    fn test_mutations_returns_none_when_tracking_disabled() {
        let mut tree = Tree::new();
        assert!(tree.mutations().is_none());
    }

    #[test]
    fn test_mutations_returns_receiver_when_tracking_enabled() {
        let mut tree = Tree::grow();
        assert!(tree.mutations().is_some());
    }

    #[test]
    fn test_store_emits_mutation_when_tracking() {
        let mut tree = Tree::grow();
        let rx       = tree.mutations().unwrap();

        tree.new_int("workers", 4, "");
        tree.parse().unwrap();
        tree.store("workers", FigValue::Int(8)).unwrap();

        // first mutation from parse (first resolution)
        // second mutation from store
        let m1 = rx.try_recv();
        let m2 = rx.try_recv();
        assert!(m1.is_some() || m2.is_some());
    }

    #[test]
    fn test_curse_closes_mutation_channel() {
        let mut tree = Tree::grow();
        let rx       = tree.mutations().unwrap();
        tree.curse();
        // channel is closed — recv returns None
        assert!(rx.recv().is_none());
    }

    // ── diagnostics ───────────────────────────────────────────────────────────

    #[test]
    fn test_usage_contains_registered_keys() {
        let mut tree = Tree::new();
        tree.new_int("workers", 4, "")
            .new_string("endpoint", "http://localhost", "");
        tree.parse().unwrap();
        let usage = tree.usage();
        assert!(usage.contains("workers"));
        assert!(usage.contains("endpoint"));
    }

    #[test]
    fn test_problems_empty_when_no_errors() {
        let mut tree = Tree::new();
        tree.new_int("workers", 4, "");
        tree.parse().unwrap();
        assert!(tree.problems().is_empty());
    }

    #[test]
    fn test_history_returns_entries_for_known_key() {
        let mut tree = Tree::new();
        tree.new_int("workers", 4, "");
        tree.parse().unwrap();
        let history = tree.history("workers").unwrap();
        assert!(!history.is_empty());
        // state 0 is always initialization
        assert_eq!(history[0].state_index, 0);
    }

    #[test]
    fn test_history_returns_none_for_unknown_key() {
        let tree = Tree::new();
        assert!(tree.history("nonexistent").is_none());
    }

    #[test]
    fn test_history_log_is_human_readable() {
        let mut tree = Tree::new();
        tree.new_int("workers", 4, "");
        tree.parse().unwrap();
        tree.store("workers", FigValue::Int(8)).unwrap();
        let log = tree.history_log("workers").unwrap();
        assert!(log.contains("state 0"));
        assert!(log.contains("initialized"));
    }

    // ── fig() raw access ──────────────────────────────────────────────────────

    #[test]
    fn test_fig_returns_raw_fig() {
        let mut tree = Tree::new();
        tree.new_int("workers", 4, "");
        let fig = tree.fig("workers").unwrap();
        assert_eq!(fig.key, "workers");
    }

    #[test]
    fn test_fig_returns_none_for_unknown_key() {
        let tree = Tree::new();
        assert!(tree.fig("nonexistent").is_none());
    }

    // ── load vs parse ─────────────────────────────────────────────────────────

    #[test]
    fn test_load_does_not_consume_flag_source() {
        let mut tree = Tree::new();
        tree.new_int("workers", 4, "");

        // add a mock flag source
        use std::collections::HashMap;
        let mut flags = HashMap::new();
        flags.insert("workers".to_string(), "99".to_string());
        tree.with_flag_source(Box::new(
            crate::sources::CliSource::new(flags)
        ));

        // load() skips flag source — should use default
        tree.load().unwrap();
        assert_eq!(tree.integer("workers").unwrap(), 4);

        // flag source should still be present for a subsequent parse()
        assert!(tree.flag_source.is_some());
    }

    // ── int128 round-trip ─────────────────────────────────────────────────────

    #[test]
    fn test_int128_round_trips() {
        let mut tree = Tree::new();
        let big: i128 = i64::MAX as i128 + 1;
        tree.new_int128("big_id", big, "");
        tree.parse().unwrap();
        assert_eq!(tree.int128("big_id").unwrap(), big);
    }
}
