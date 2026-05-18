use std::collections::HashMap;

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

    /// When true, Tree::pollinate() re-checks all environment variables
    /// and updates any Figs whose env var has changed since last resolution.
    /// Pollination is always explicit — call tree.pollinate() in a ticker
    /// or signal handler. Default false means pollinate() is a no-op.
    pub pollinate: bool,

    /// Ignore CLI flags whose names begin with "-test." — flags injected
    /// by the test runner infrastructure. Set true when running inside a
    /// test harness. Default false.
    pub germinate: bool,

    /// Path to a config file to load automatically on parse() or load().
    /// Format is detected from the file extension. Optional.
    pub config_file: Option<String>,
}

// ── Tree ──────────────────────────────────────────────────────────────────────

/// The central configuration tree.
///
/// ## Lifecycle
///
///   1. Construct:   Tree::new(), Tree::grow(), or Tree::with(Options)
///   2. Register:    new_string(), new_int(), new_bool(), etc.
///   3. Constrain:   with_validator(), with_callback(), with_rule()
///   4. Source:      with_file_source(), with_yaml_file(), etc.
///   5. Resolve:     parse() or load()
///   6. Read:        string(), integer(), boolean(), etc.  — all take &self
///   7. React:       mutations() receiver for live change notifications
///   8. Update:      store() for programmatic changes after resolution
///   9. Pollinate:   pollinate() or pollinate_key() to re-check env vars
///
/// ## Mutation vs Pollination
///
/// Mutations are outbound notifications — the Tree tells you something
/// changed. Pollination is inbound re-checking — you tell the Tree to
/// look again at the environment. They are orthogonal and both optional.
///
/// ## Thread Safety
///
/// Tree is not Send or Sync by itself. Wrap in Arc<RwLock<Tree>> for
/// shared multi-threaded access. The mutation channel is independently
/// thread-safe.
pub struct Tree {
    figs:         HashMap<String, Fig>,
    descriptions: HashMap<String, String>,
    validators:   HashMap<String, ValidatorRegistry>,
    callbacks:    HashMap<String, CallbackRegistry>,
    pub(crate) flag_source:  Option<Box<dyn Source>>,
    env_source:   Option<Box<dyn Source>>,
    file_sources: Vec<Box<dyn Source>>,
    tracking:     bool,
    pollinate:    bool,
    mutation_tx:  Option<MutationSender>,
    resolved:     bool,
}

impl Tree {
    // ── Constructors ──────────────────────────────────────────────────────────

    /// Creates a Tree with no mutation tracking.
    pub fn new() -> Self {
        Tree::with(Options::default())
    }

    /// Creates a Tree with mutation tracking enabled.
    /// Call mutations() after grow() to receive the channel before
    /// calling parse() or load() — otherwise early mutations are missed.
    pub fn grow() -> Self {
        Tree::with(Options { tracking: true, ..Options::default() })
    }

    /// Creates a Tree with full option control.
    pub fn with(options: Options) -> Self {
        let (tracking, mutation_tx) = if options.tracking {
            let (tx, _rx) = mutation_channel();
            (true, Some(tx))
        } else {
            (false, None)
        };

        let mut tree = Tree {
            figs:         HashMap::new(),
            descriptions: HashMap::new(),
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
            tree.load_config_file_by_extension(&path);
        }

        tree
    }

    // ── Source registration ───────────────────────────────────────────────────

    /// Replaces the flag source. The default is None.
    /// Typically set to a CliSource constructed from clap ArgMatches.
    pub fn with_flag_source(&mut self, source: Box<dyn Source>) -> &mut Self {
        self.flag_source = Some(source);
        self
    }

    /// Replaces the environment variable source.
    /// Pass None to disable environment variable resolution entirely.
    pub fn with_env_source(&mut self, source: Option<Box<dyn Source>>) -> &mut Self {
        self.env_source = source;
        self
    }

    /// Adds a file source in registration order. Multiple file sources
    /// are consulted in registration order — first registered wins.
    pub fn with_file_source(&mut self, source: Box<dyn Source>) -> &mut Self {
        self.file_sources.push(source);
        self
    }

    #[cfg(feature = "yaml")]
    pub fn with_yaml_file(&mut self, path: &str) -> FigtreeResult<&mut Self> {
        let src = crate::sources::YamlSource::load(path)?;
        self.file_sources.push(Box::new(src));
        Ok(self)
    }

    #[cfg(feature = "json")]
    pub fn with_json_file(&mut self, path: &str) -> FigtreeResult<&mut Self> {
        let src = crate::sources::JsonSource::load(path)?;
        self.file_sources.push(Box::new(src));
        Ok(self)
    }

    #[cfg(feature = "toml-fmt")]
    pub fn with_toml_file(&mut self, path: &str) -> FigtreeResult<&mut Self> {
        let src = crate::sources::TomlSource::load(path)?;
        self.file_sources.push(Box::new(src));
        Ok(self)
    }

    #[cfg(feature = "ini")]
    pub fn with_ini_file(&mut self, path: &str) -> FigtreeResult<&mut Self> {
        let src = crate::sources::IniSource::load(path)?;
        self.file_sources.push(Box::new(src));
        Ok(self)
    }

    #[cfg(feature = "plist")]
    pub fn with_plist_file(&mut self, path: &str) -> FigtreeResult<&mut Self> {
        let src = crate::sources::PlistSource::load(path)?;
        self.file_sources.push(Box::new(src));
        Ok(self)
    }

    #[cfg(feature = "dotenv")]
    pub fn with_dotenv_file(&mut self, path: &str) -> FigtreeResult<&mut Self> {
        let src = crate::sources::DotenvSource::load(path)?;
        self.file_sources.push(Box::new(src));
        Ok(self)
    }

    #[cfg(feature = "ron")]
    pub fn with_ron_file(&mut self, path: &str) -> FigtreeResult<&mut Self> {
        let src = crate::sources::RonSource::load(path)?;
        self.file_sources.push(Box::new(src));
        Ok(self)
    }

    // ── Key registration ──────────────────────────────────────────────────────

    pub fn new_string(
        &mut self,
        key:         impl Into<String>,
        default:     impl Into<String>,
        description: impl Into<String>,
    ) -> &mut Self {
        let key = key.into();
        self.descriptions.insert(key.clone(), description.into());
        self.register(key, Some(FigValue::String(default.into())))
    }

    pub fn new_string_required(
        &mut self,
        key:         impl Into<String>,
        description: impl Into<String>,
    ) -> &mut Self {
        let key = key.into();
        self.descriptions.insert(key.clone(), description.into());
        self.register(key, None)
    }

    pub fn new_int(
        &mut self,
        key:         impl Into<String>,
        default:     i32,
        description: impl Into<String>,
    ) -> &mut Self {
        let key = key.into();
        self.descriptions.insert(key.clone(), description.into());
        self.register(key, Some(FigValue::Int(default)))
    }

    pub fn new_int64(
        &mut self,
        key:         impl Into<String>,
        default:     i64,
        description: impl Into<String>,
    ) -> &mut Self {
        let key = key.into();
        self.descriptions.insert(key.clone(), description.into());
        self.register(key, Some(FigValue::Int64(default)))
    }

    pub fn new_int128(
        &mut self,
        key:         impl Into<String>,
        default:     i128,
        description: impl Into<String>,
    ) -> &mut Self {
        let key = key.into();
        self.descriptions.insert(key.clone(), description.into());
        self.register(key, Some(FigValue::Int128(default)))
    }

    pub fn new_float64(
        &mut self,
        key:         impl Into<String>,
        default:     f64,
        description: impl Into<String>,
    ) -> &mut Self {
        let key = key.into();
        self.descriptions.insert(key.clone(), description.into());
        self.register(key, Some(FigValue::Float64(default)))
    }

    pub fn new_float128(
        &mut self,
        key:         impl Into<String>,
        default:     f64,
        description: impl Into<String>,
    ) -> &mut Self {
        let key = key.into();
        self.descriptions.insert(key.clone(), description.into());
        self.register(key, Some(FigValue::Float128(default)))
    }

    pub fn new_bool(
        &mut self,
        key:         impl Into<String>,
        default:     bool,
        description: impl Into<String>,
    ) -> &mut Self {
        let key = key.into();
        self.descriptions.insert(key.clone(), description.into());
        self.register(key, Some(FigValue::Bool(default)))
    }

    pub fn new_duration(
        &mut self,
        key:         impl Into<String>,
        default:     std::time::Duration,
        description: impl Into<String>,
    ) -> &mut Self {
        let key = key.into();
        self.descriptions.insert(key.clone(), description.into());
        self.register(key, Some(FigValue::Duration(default)))
    }

    pub fn new_list_string(
        &mut self,
        key:         impl Into<String>,
        default:     Vec<String>,
        description: impl Into<String>,
    ) -> &mut Self {
        let key = key.into();
        self.descriptions.insert(key.clone(), description.into());
        self.register(key, Some(FigValue::ListString(default)))
    }

    pub fn new_list_int(
        &mut self,
        key:         impl Into<String>,
        default:     Vec<i32>,
        description: impl Into<String>,
    ) -> &mut Self {
        let key = key.into();
        self.descriptions.insert(key.clone(), description.into());
        self.register(key, Some(FigValue::ListInt(default)))
    }

    pub fn new_list_int64(
        &mut self,
        key:         impl Into<String>,
        default:     Vec<i64>,
        description: impl Into<String>,
    ) -> &mut Self {
        let key = key.into();
        self.descriptions.insert(key.clone(), description.into());
        self.register(key, Some(FigValue::ListInt64(default)))
    }

    pub fn new_list_int128(
        &mut self,
        key:         impl Into<String>,
        default:     Vec<i128>,
        description: impl Into<String>,
    ) -> &mut Self {
        let key = key.into();
        self.descriptions.insert(key.clone(), description.into());
        self.register(key, Some(FigValue::ListInt128(default)))
    }

    pub fn new_list_float64(
        &mut self,
        key:         impl Into<String>,
        default:     Vec<f64>,
        description: impl Into<String>,
    ) -> &mut Self {
        let key = key.into();
        self.descriptions.insert(key.clone(), description.into());
        self.register(key, Some(FigValue::ListFloat64(default)))
    }

    pub fn new_list_float128(
        &mut self,
        key:         impl Into<String>,
        default:     Vec<f64>,
        description: impl Into<String>,
    ) -> &mut Self {
        let key = key.into();
        self.descriptions.insert(key.clone(), description.into());
        self.register(key, Some(FigValue::ListFloat128(default)))
    }

    pub fn new_list_bool(
        &mut self,
        key:         impl Into<String>,
        default:     Vec<bool>,
        description: impl Into<String>,
    ) -> &mut Self {
        let key = key.into();
        self.descriptions.insert(key.clone(), description.into());
        self.register(key, Some(FigValue::ListBool(default)))
    }

    pub fn new_map_string(
        &mut self,
        key:         impl Into<String>,
        default:     HashMap<String, String>,
        description: impl Into<String>,
    ) -> &mut Self {
        let key = key.into();
        self.descriptions.insert(key.clone(), description.into());
        self.register(key, Some(FigValue::MapString(default)))
    }

    pub fn new_map_bool(
        &mut self,
        key:         impl Into<String>,
        default:     HashMap<String, bool>,
        description: impl Into<String>,
    ) -> &mut Self {
        let key = key.into();
        self.descriptions.insert(key.clone(), description.into());
        self.register(key, Some(FigValue::MapBool(default)))
    }

    pub fn new_map_int(
        &mut self,
        key:         impl Into<String>,
        default:     HashMap<String, i32>,
        description: impl Into<String>,
    ) -> &mut Self {
        let key = key.into();
        self.descriptions.insert(key.clone(), description.into());
        self.register(key, Some(FigValue::MapInt(default)))
    }

    pub fn new_map_int64(
        &mut self,
        key:         impl Into<String>,
        default:     HashMap<String, i64>,
        description: impl Into<String>,
    ) -> &mut Self {
        let key = key.into();
        self.descriptions.insert(key.clone(), description.into());
        self.register(key, Some(FigValue::MapInt64(default)))
    }

    pub fn new_map_int128(
        &mut self,
        key:         impl Into<String>,
        default:     HashMap<String, i128>,
        description: impl Into<String>,
    ) -> &mut Self {
        let key = key.into();
        self.descriptions.insert(key.clone(), description.into());
        self.register(key, Some(FigValue::MapInt128(default)))
    }

    pub fn new_map_float64(
        &mut self,
        key:         impl Into<String>,
        default:     HashMap<String, f64>,
        description: impl Into<String>,
    ) -> &mut Self {
        let key = key.into();
        self.descriptions.insert(key.clone(), description.into());
        self.register(key, Some(FigValue::MapFloat64(default)))
    }

    pub fn new_map_float128(
        &mut self,
        key:         impl Into<String>,
        default:     HashMap<String, f64>,
        description: impl Into<String>,
    ) -> &mut Self {
        let key = key.into();
        self.descriptions.insert(key.clone(), description.into());
        self.register(key, Some(FigValue::MapFloat128(default)))
    }

    // ── Constraints ───────────────────────────────────────────────────────────

    /// Attaches a named validator to a key.
    /// Multiple validators per key run in registration order.
    /// First failure halts the chain.
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
    /// Multiple callbacks per phase run in registration order.
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
    /// Rule::NoMaps rejects if the key's registered type is any map variant.
    /// Rule::NoLists rejects if the key's registered type is any list variant.
    pub fn with_rule(
        &mut self,
        key:  impl Into<String>,
        rule: Rule,
    ) -> FigtreeResult<&mut Self> {
        let key = key.into();
        let fig = match self.figs.get(&key) {
            Some(f) => f,
            None    => return Err(FigtreeError::UnknownKey(key)),
        };

        // NoMaps: reject if this key's default is a map type
        if rule.blocks_maps() {
            if let Some(v) = &fig.default {
                if v.is_map() {
                    return Err(FigtreeError::RuleViolation {
                        key,
                        rule: rule.to_string(),
                    });
                }
            }
        }

        // NoLists: reject if this key's default is a list type
        if rule.blocks_lists() {
            if let Some(v) = &fig.default {
                if v.is_list() {
                    return Err(FigtreeError::RuleViolation {
                        key,
                        rule: rule.to_string(),
                    });
                }
            }
        }

        self.figs.get_mut(&key).unwrap().rule = rule;
        Ok(self)
    }

    // ── Resolution ────────────────────────────────────────────────────────────

    /// Resolves all registered keys from all sources in PEMDAS order,
    /// including the CLI flag source if registered.
    /// Runs validators. Fires AfterVerify callbacks.
    /// Returns the first error encountered.
    pub fn parse(&mut self) -> FigtreeResult<()> {
        self.resolve_all(true)
    }

    /// Resolves all registered keys from all sources in PEMDAS order,
    /// excluding the CLI flag source.
    /// For long-running services, embedded systems, library code.
    pub fn load(&mut self) -> FigtreeResult<()> {
        self.resolve_all(false)
    }

    /// Sets a value programmatically after parse() or load().
    ///
    /// Enforces rules and validates the new value.
    /// Fires AfterChange callbacks.
    /// Emits a Mutation if tracking is enabled and the value changed.
    pub fn store(
        &mut self,
        key:   impl Into<String>,
        value: FigValue,
    ) -> FigtreeResult<()> {
        let key = key.into();

        // collect what we need before mutating
        let (skip_validators, skip_callbacks, old_value, rule) = {
            let fig = self.figs.get(&key)
                .ok_or_else(|| FigtreeError::UnknownKey(key.clone()))?;
            (
                fig.rule == Rule::NoValidations,
                fig.rule == Rule::NoCallbacks,
                fig.value.clone(),
                fig.rule.clone(),
            )
        };

        // NoMaps: block store() if the incoming value is a map type
        if rule.blocks_maps() && value.is_map() {
            return Err(FigtreeError::RuleViolation {
                key,
                rule: rule.to_string(),
            });
        }

        // NoLists: block store() if the incoming value is a list type
        if rule.blocks_lists() && value.is_list() {
            return Err(FigtreeError::RuleViolation {
                key,
                rule: rule.to_string(),
            });
        }

        // validate before mutating
        if !skip_validators {
            if let Some(registry) = self.validators.get(&key) {
                registry.validate(&value).map_err(|e| match e {
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

        // mutate
        self.figs.get_mut(&key).unwrap()
            .set(value.clone(), FigSource::Programmatic)?;

        // fire AfterChange callbacks
        if !skip_callbacks {
            if let Some(registry) = self.callbacks.get(&key) {
                registry.invoke_phase(&CallbackPhase::AfterChange, &value)
                    .map_err(|e| FigtreeError::CallbackFailed {
                        key:     key.clone(),
                        phase:   CallbackPhase::AfterChange.to_string(),
                        message: e.to_string(),
                    })?;
            }
        }

        // emit mutation
        self.emit_mutation(key, old_value, value, FigSource::Programmatic);

        Ok(())
    }

    // ── Pollination ───────────────────────────────────────────────────────────

    /// Re-checks all environment variables and updates any Figs whose
    /// env var value has changed since last resolution.
    pub fn pollinate(&mut self) -> FigtreeResult<()> {
        if !self.pollinate {
            return Ok(());
        }
        let keys: Vec<String> = self.figs.keys().cloned().collect();
        for key in keys {
            self.pollinate_key(&key)?;
        }
        Ok(())
    }

    /// Re-checks the environment variable for a single key.
    pub fn pollinate_key(&mut self, key: &str) -> FigtreeResult<()> {
        if !self.pollinate {
            return Ok(());
        }

        let env_source = crate::sources::EnvSource::new();

        let (hint, current) = {
            let fig = match self.figs.get(key) {
                Some(f) => f,
                None    => return Ok(()),
            };
            let hint    = fig.resolve().or(fig.default.as_ref()).cloned();
            let current = fig.value.clone();
            (hint, current)
        };

        let raw = match env_source.get(key) {
            Some(FigValue::String(s)) => s,
            _                         => return Ok(()),
        };

        let hint_val = match &hint {
            Some(h) => h,
            None    => return Ok(()),
        };

        let typed = match crate::sources::env::parse_env_value(&raw, hint_val) {
            Some(v) => v,
            None    => return Ok(()),
        };

        if current.as_ref() == Some(&typed) {
            return Ok(());
        }

        let source = FigSource::Environment(key.into());
        let old    = current;

        self.figs.get_mut(key).unwrap()
            .set(typed.clone(), source.clone())?;

        self.emit_mutation(key.into(), old, typed, source);

        Ok(())
    }

    // ── Getters — all take &self ──────────────────────────────────────────────

    /// Returns the current String value for a key.
    pub fn string(&self, key: &str) -> FigtreeResult<&str> {
        self.invoke_after_read(key)?;
        match self.require_fig(key)?.resolve() {
            Some(FigValue::String(s)) => Ok(s.as_str()),
            Some(other)               => Err(FigtreeError::TypeMismatch {
                key:      key.into(),
                expected: "String".into(),
                got:      other.type_name().into(),
            }),
            None => Err(FigtreeError::MissingRequired(key.into())),
        }
    }

    /// Returns the current Int (i32) value for a key.
    pub fn integer(&self, key: &str) -> FigtreeResult<i32> {
        self.invoke_after_read(key)?;
        match self.require_fig(key)?.resolve() {
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
    pub fn int64(&self, key: &str) -> FigtreeResult<i64> {
        self.invoke_after_read(key)?;
        match self.require_fig(key)?.resolve() {
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
    pub fn int128(&self, key: &str) -> FigtreeResult<i128> {
        self.invoke_after_read(key)?;
        match self.require_fig(key)?.resolve() {
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
    pub fn float64(&self, key: &str) -> FigtreeResult<f64> {
        self.invoke_after_read(key)?;
        match self.require_fig(key)?.resolve() {
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
    pub fn float128(&self, key: &str) -> FigtreeResult<f64> {
        self.invoke_after_read(key)?;
        match self.require_fig(key)?.resolve() {
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
    pub fn boolean(&self, key: &str) -> FigtreeResult<bool> {
        self.invoke_after_read(key)?;
        match self.require_fig(key)?.resolve() {
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
    pub fn duration(&self, key: &str) -> FigtreeResult<std::time::Duration> {
        self.invoke_after_read(key)?;
        match self.require_fig(key)?.resolve() {
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
    pub fn list_string(&self, key: &str) -> FigtreeResult<Vec<String>> {
        self.invoke_after_read(key)?;
        match self.require_fig(key)?.resolve() {
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
    pub fn list_int(&self, key: &str) -> FigtreeResult<Vec<i32>> {
        self.invoke_after_read(key)?;
        match self.require_fig(key)?.resolve() {
            Some(FigValue::ListInt(v)) => Ok(v.clone()),
            Some(other)                => Err(FigtreeError::TypeMismatch {
                key:      key.into(),
                expected: "ListInt".into(),
                got:      other.type_name().into(),
            }),
            None => Err(FigtreeError::MissingRequired(key.into())),
        }
    }

    /// Returns the current ListInt64 value for a key.
    pub fn list_int64(&self, key: &str) -> FigtreeResult<Vec<i64>> {
        self.invoke_after_read(key)?;
        match self.require_fig(key)?.resolve() {
            Some(FigValue::ListInt64(v)) => Ok(v.clone()),
            Some(other)                  => Err(FigtreeError::TypeMismatch {
                key:      key.into(),
                expected: "ListInt64".into(),
                got:      other.type_name().into(),
            }),
            None => Err(FigtreeError::MissingRequired(key.into())),
        }
    }

    /// Returns the current ListInt128 value for a key.
    pub fn list_int128(&self, key: &str) -> FigtreeResult<Vec<i128>> {
        self.invoke_after_read(key)?;
        match self.require_fig(key)?.resolve() {
            Some(FigValue::ListInt128(v)) => Ok(v.clone()),
            Some(other)                   => Err(FigtreeError::TypeMismatch {
                key:      key.into(),
                expected: "ListInt128".into(),
                got:      other.type_name().into(),
            }),
            None => Err(FigtreeError::MissingRequired(key.into())),
        }
    }

    /// Returns the current ListFloat64 value for a key.
    pub fn list_float64(&self, key: &str) -> FigtreeResult<Vec<f64>> {
        self.invoke_after_read(key)?;
        match self.require_fig(key)?.resolve() {
            Some(FigValue::ListFloat64(v)) => Ok(v.clone()),
            Some(other)                    => Err(FigtreeError::TypeMismatch {
                key:      key.into(),
                expected: "ListFloat64".into(),
                got:      other.type_name().into(),
            }),
            None => Err(FigtreeError::MissingRequired(key.into())),
        }
    }

    /// Returns the current ListFloat128 value for a key.
    pub fn list_float128(&self, key: &str) -> FigtreeResult<Vec<f64>> {
        self.invoke_after_read(key)?;
        match self.require_fig(key)?.resolve() {
            Some(FigValue::ListFloat128(v)) => Ok(v.clone()),
            Some(other)                     => Err(FigtreeError::TypeMismatch {
                key:      key.into(),
                expected: "ListFloat128".into(),
                got:      other.type_name().into(),
            }),
            None => Err(FigtreeError::MissingRequired(key.into())),
        }
    }

    /// Returns the current ListBool value for a key.
    pub fn list_bool(&self, key: &str) -> FigtreeResult<Vec<bool>> {
        self.invoke_after_read(key)?;
        match self.require_fig(key)?.resolve() {
            Some(FigValue::ListBool(v)) => Ok(v.clone()),
            Some(other)                 => Err(FigtreeError::TypeMismatch {
                key:      key.into(),
                expected: "ListBool".into(),
                got:      other.type_name().into(),
            }),
            None => Err(FigtreeError::MissingRequired(key.into())),
        }
    }

    /// Returns the current MapString value for a key.
    pub fn map_string(&self, key: &str) -> FigtreeResult<HashMap<String, String>> {
        self.invoke_after_read(key)?;
        match self.require_fig(key)?.resolve() {
            Some(FigValue::MapString(m)) => Ok(m.clone()),
            Some(other)                  => Err(FigtreeError::TypeMismatch {
                key:      key.into(),
                expected: "MapString".into(),
                got:      other.type_name().into(),
            }),
            None => Err(FigtreeError::MissingRequired(key.into())),
        }
    }

    /// Returns the current MapBool value for a key.
    pub fn map_bool(&self, key: &str) -> FigtreeResult<HashMap<String, bool>> {
        self.invoke_after_read(key)?;
        match self.require_fig(key)?.resolve() {
            Some(FigValue::MapBool(m)) => Ok(m.clone()),
            Some(other)                => Err(FigtreeError::TypeMismatch {
                key:      key.into(),
                expected: "MapBool".into(),
                got:      other.type_name().into(),
            }),
            None => Err(FigtreeError::MissingRequired(key.into())),
        }
    }

    /// Returns the current MapInt value for a key.
    pub fn map_int(&self, key: &str) -> FigtreeResult<HashMap<String, i32>> {
        self.invoke_after_read(key)?;
        match self.require_fig(key)?.resolve() {
            Some(FigValue::MapInt(m)) => Ok(m.clone()),
            Some(other)               => Err(FigtreeError::TypeMismatch {
                key:      key.into(),
                expected: "MapInt".into(),
                got:      other.type_name().into(),
            }),
            None => Err(FigtreeError::MissingRequired(key.into())),
        }
    }

    /// Returns the current MapInt64 value for a key.
    pub fn map_int64(&self, key: &str) -> FigtreeResult<HashMap<String, i64>> {
        self.invoke_after_read(key)?;
        match self.require_fig(key)?.resolve() {
            Some(FigValue::MapInt64(m)) => Ok(m.clone()),
            Some(other)                 => Err(FigtreeError::TypeMismatch {
                key:      key.into(),
                expected: "MapInt64".into(),
                got:      other.type_name().into(),
            }),
            None => Err(FigtreeError::MissingRequired(key.into())),
        }
    }

    /// Returns the current MapInt128 value for a key.
    pub fn map_int128(&self, key: &str) -> FigtreeResult<HashMap<String, i128>> {
        self.invoke_after_read(key)?;
        match self.require_fig(key)?.resolve() {
            Some(FigValue::MapInt128(m)) => Ok(m.clone()),
            Some(other)                  => Err(FigtreeError::TypeMismatch {
                key:      key.into(),
                expected: "MapInt128".into(),
                got:      other.type_name().into(),
            }),
            None => Err(FigtreeError::MissingRequired(key.into())),
        }
    }

    /// Returns the current MapFloat64 value for a key.
    pub fn map_float64(&self, key: &str) -> FigtreeResult<HashMap<String, f64>> {
        self.invoke_after_read(key)?;
        match self.require_fig(key)?.resolve() {
            Some(FigValue::MapFloat64(m)) => Ok(m.clone()),
            Some(other)                   => Err(FigtreeError::TypeMismatch {
                key:      key.into(),
                expected: "MapFloat64".into(),
                got:      other.type_name().into(),
            }),
            None => Err(FigtreeError::MissingRequired(key.into())),
        }
    }

    /// Returns the current MapFloat128 value for a key.
    pub fn map_float128(&self, key: &str) -> FigtreeResult<HashMap<String, f64>> {
        self.invoke_after_read(key)?;
        match self.require_fig(key)?.resolve() {
            Some(FigValue::MapFloat128(m)) => Ok(m.clone()),
            Some(other)                    => Err(FigtreeError::TypeMismatch {
                key:      key.into(),
                expected: "MapFloat128".into(),
                got:      other.type_name().into(),
            }),
            None => Err(FigtreeError::MissingRequired(key.into())),
        }
    }

    /// Returns the raw Fig for a key, providing access to history,
    /// source, rule, and error fields.
    pub fn fig(&self, key: &str) -> Option<&Fig> {
        self.figs.get(key)
    }

    // ── Mutation tracking ─────────────────────────────────────────────────────

    pub fn mutations(&mut self) -> Option<MutationReceiver> {
        if !self.tracking {
            return None;
        }
        let (tx, rx) = mutation_channel();
        self.mutation_tx = Some(tx);
        Some(rx)
    }

    pub fn curse(&mut self) {
        self.mutation_tx = None;
    }

    pub fn recall(&mut self) {
        if self.tracking {
            let (tx, _rx) = mutation_channel();
            self.mutation_tx = Some(tx);
        }
    }

    // ── Diagnostics — all take &self ──────────────────────────────────────────

    pub fn usage(&self) -> String {
        let mut keys: Vec<&str> = self.figs.keys().map(|k| k.as_str()).collect();
        keys.sort();

        let mut lines = vec!["Tree configuration:".to_string()];
        for key in keys {
            if let Some(fig) = self.figs.get(key) {
                let value = fig.resolve()
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "<unresolved>".into());
                let source = fig.source.to_string();
                let desc   = self.descriptions.get(key)
                    .map(|d| format!("  # {}", d))
                    .unwrap_or_default();
                let error  = fig.error.as_ref()
                    .map(|e| format!(" [ERROR: {}]", e))
                    .unwrap_or_default();
                lines.push(format!(
                    "  {:30} = {:40} ({}){}{}",
                    key, value, source, error, desc
                ));
            }
        }
        lines.join("\n")
    }

    pub fn problems(&self) -> Vec<FigtreeError> {
        self.figs.values()
            .filter_map(|fig| fig.error.as_ref())
            .map(|e| FigtreeError::Other(e.to_string()))
            .collect()
    }

    pub fn history(&self, key: &str) -> Option<&[FigHistoryEntry]> {
        self.figs.get(key).map(|fig| fig.history())
    }

    pub fn history_log(&self, key: &str) -> Option<String> {
        self.figs.get(key).map(|fig| fig.history_log())
    }

    pub fn is_resolved(&self) -> bool {
        self.resolved
    }

    pub fn len(&self) -> usize {
        self.figs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.figs.is_empty()
    }

    // ── Private helpers ───────────────────────────────────────────────────────

    fn register(&mut self, key: String, default: Option<FigValue>) -> &mut Self {
        self.figs
            .entry(key.clone())
            .or_insert_with(|| Fig::new(key, default));
        self
    }

    fn resolve_all(&mut self, include_flags: bool) -> FigtreeResult<()> {
        let keys: Vec<String> = self.figs.keys().cloned().collect();

        for key in keys {
            let (rule, is_resolved, is_required) = {
                let fig = match self.figs.get(&key) {
                    Some(f) => f,
                    None    => continue,
                };
                (fig.rule.clone(), fig.is_resolved(), fig.is_required())
            };

            if rule == Rule::PreventChange && is_resolved {
                continue;
            }

            let flag_src = if include_flags {
                self.flag_source.as_deref()
            } else {
                None
            };

            let result = {
                let fig = self.figs.get(&key).unwrap();
                resolve(fig, flag_src, self.env_source.as_deref(), &self.file_sources)
            };

            match result {
                Some(resolution) => {
                    let typed = self.coerce_to_type(&key, resolution.value)?;

                    let skip_validators = rule == Rule::NoValidations;
                    if !skip_validators {
                        if let Some(registry) = self.validators.get(&key) {
                            registry.validate(&typed).map_err(|e| match e {
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

                    let old_value = self.figs.get(&key)
                        .and_then(|f| f.value.clone());

                    self.figs.get_mut(&key).unwrap()
                        .set(typed.clone(), resolution.source.clone())?;

                    let skip_callbacks = rule == Rule::NoCallbacks;
                    if !skip_callbacks {
                        if let Some(registry) = self.callbacks.get(&key) {
                            registry.invoke_phase(
                                &CallbackPhase::AfterVerify,
                                &typed,
                            ).map_err(|e| FigtreeError::CallbackFailed {
                                key:     key.clone(),
                                phase:   "AfterVerify".into(),
                                message: e.to_string(),
                            })?;
                        }
                    }

                    self.emit_mutation(key, old_value, typed, resolution.source);
                }
                None if is_required => {
                    return Err(FigtreeError::MissingRequired(key));
                }
                None => {}
            }
        }

        self.resolved = true;
        Ok(())
    }

    fn coerce_to_type(
        &self,
        key:   &str,
        value: FigValue,
    ) -> FigtreeResult<FigValue> {
        let hint = self.figs
            .get(key)
            .and_then(|fig| fig.resolve().or(fig.default.as_ref()))
            .cloned();

        match (value, hint.as_ref()) {
            (v, Some(h)) if h.same_type(&v) => Ok(v),
            (v, None)                        => Ok(v),
            (FigValue::String(raw), Some(hint_val)) => {
                crate::sources::env::parse_env_value(&raw, hint_val)
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
            (v, _) => Ok(v),
        }
    }

    fn invoke_after_read(&self, key: &str) -> FigtreeResult<()> {
        let fig = match self.figs.get(key) {
            Some(f) => f,
            None    => return Ok(()),
        };

        if fig.rule == Rule::NoCallbacks {
            return Ok(());
        }

        let value = match fig.resolve() {
            Some(v) => v,
            None    => return Ok(()),
        };

        if let Some(registry) = self.callbacks.get(key) {
            registry.invoke_phase(&CallbackPhase::AfterRead, value)
                .map_err(|e| FigtreeError::CallbackFailed {
                    key:     key.into(),
                    phase:   "AfterRead".into(),
                    message: e.to_string(),
                })?;
        }

        Ok(())
    }

    fn emit_mutation(
        &self,
        key:       String,
        old_value: Option<FigValue>,
        new_value: FigValue,
        source:    FigSource,
    ) {
        if !self.tracking {
            return;
        }
        if let Some(ref tx) = self.mutation_tx {
            let mutation = match old_value {
                None      => Mutation::first(key, new_value, source),
                Some(old) => Mutation::changed(key, old, new_value, source),
            };
            tx.send(mutation);
        }
    }

    fn require_fig(&self, key: &str) -> FigtreeResult<&Fig> {
        self.figs
            .get(key)
            .ok_or_else(|| FigtreeError::UnknownKey(key.into()))
    }

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
        if lower.ends_with(".env") {
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
    fn test_new_creates_empty_non_tracking_tree() {
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
    fn test_with_pollinate_option() {
        let tree = Tree::with(Options { pollinate: true, ..Options::default() });
        assert!(tree.pollinate);
    }

    // ── registration ─────────────────────────────────────────────────────────

    #[test]
    fn test_register_multiple_types() {
        let mut tree = Tree::new();
        tree.new_string("endpoint", "http://localhost", "api endpoint")
            .new_int("workers", 4, "worker count")
            .new_bool("debug", false, "debug mode")
            .new_float64("threshold", 0.5, "match threshold")
            .new_int128("big_id", 0, "large identifier");
        assert_eq!(tree.len(), 5);
    }

    #[test]
    fn test_duplicate_registration_first_wins() {
        let mut tree = Tree::new();
        tree.new_int("workers", 4, "first");
        tree.new_int("workers", 99, "second");
        tree.parse().unwrap();
        assert_eq!(tree.integer("workers").unwrap(), 4);
    }

    #[test]
    fn test_description_stored() {
        let mut tree = Tree::new();
        tree.new_int("workers", 4, "number of worker threads");
        assert_eq!(
            tree.descriptions.get("workers").map(|s| s.as_str()),
            Some("number of worker threads")
        );
    }

    // ── parse and load ────────────────────────────────────────────────────────

    #[test]
    fn test_parse_resolves_defaults() {
        let mut tree = Tree::new();
        tree.new_int("workers", 10, "")
            .new_string("host", "localhost", "")
            .new_bool("debug", true, "");
        tree.parse().unwrap();
        assert!(tree.is_resolved());
        assert_eq!(tree.integer("workers").unwrap(), 10);
        assert_eq!(tree.string("host").unwrap(), "localhost");
        assert_eq!(tree.boolean("debug").unwrap(), true);
    }

    #[test]
    fn test_required_key_missing_fails_parse() {
        let mut tree = Tree::new();
        tree.new_string_required("api_key", "required api key");
        assert!(matches!(
            tree.parse(),
            Err(FigtreeError::MissingRequired(_))
        ));
    }

    #[cfg(feature = "cli")]
    #[test]
    fn test_load_skips_flag_source_and_preserves_it() {
        let mut tree = Tree::new();
        tree.new_int("workers", 4, "");

        use std::collections::HashMap as HM;
        let mut flags = HM::new();
        flags.insert("workers".to_string(), "99".to_string());
        tree.with_flag_source(Box::new(crate::sources::CliSource::new(flags)));

        tree.load().unwrap();
        assert_eq!(tree.integer("workers").unwrap(), 4);
        assert!(tree.flag_source.is_some());
    }

    // ── getters take &self ────────────────────────────────────────────────────

    #[test]
    fn test_string_getter_takes_shared_ref() {
        let mut tree = Tree::new();
        tree.new_string("host", "localhost", "");
        tree.parse().unwrap();
        let a = tree.string("host").unwrap();
        let b = tree.string("host").unwrap();
        assert_eq!(a, b);
        assert_eq!(a, "localhost");
    }

    #[test]
    fn test_getters_return_correct_types() {
        let mut tree = Tree::new();
        let big: i128 = i64::MAX as i128 + 1;
        tree.new_int("workers",     4,     "")
            .new_int64("big_int",   i64::MAX, "")
            .new_int128("huge",     big,   "")
            .new_float64("ratio",   0.75,  "")
            .new_bool("debug",      false, "")
            .new_string("host",     "x",   "");
        tree.parse().unwrap();

        assert_eq!(tree.integer("workers").unwrap(),  4);
        assert_eq!(tree.int64("big_int").unwrap(),    i64::MAX);
        assert_eq!(tree.int128("huge").unwrap(),      big);
        assert_eq!(tree.float64("ratio").unwrap(),    0.75);
        assert_eq!(tree.boolean("debug").unwrap(),    false);
        assert_eq!(tree.string("host").unwrap(),      "x");
    }

    #[test]
    fn test_all_list_getters_round_trip() {
        let mut tree = Tree::new();
        tree.new_list_int64("ids64",    vec![i64::MAX], "")
            .new_list_int128("ids128",  vec![i64::MAX as i128 + 1], "")
            .new_list_float64("floats", vec![1.5, 2.5], "")
            .new_list_bool("flags",     vec![true, false], "");
        tree.parse().unwrap();

        assert_eq!(tree.list_int64("ids64").unwrap(),    vec![i64::MAX]);
        assert_eq!(tree.list_int128("ids128").unwrap(),  vec![i64::MAX as i128 + 1]);
        assert_eq!(tree.list_float64("floats").unwrap(), vec![1.5, 2.5]);
        assert_eq!(tree.list_bool("flags").unwrap(),     vec![true, false]);
    }

    #[test]
    fn test_all_map_getters_round_trip() {
        let mut tree = Tree::new();

        let mut mi    = HashMap::new(); mi.insert("a".to_string(), 1i32);
        let mut mi64  = HashMap::new(); mi64.insert("b".to_string(), 2i64);
        let mut mi128 = HashMap::new(); mi128.insert("c".to_string(), 3i128);
        let mut mf64  = HashMap::new(); mf64.insert("d".to_string(), 4.0f64);
        let mut mb    = HashMap::new(); mb.insert("e".to_string(), true);

        tree.new_map_int("map_i",       mi.clone(),    "")
            .new_map_int64("map_i64",   mi64.clone(),  "")
            .new_map_int128("map_i128", mi128.clone(), "")
            .new_map_float64("map_f64", mf64.clone(),  "")
            .new_map_bool("map_b",      mb.clone(),    "");
        tree.parse().unwrap();

        assert_eq!(tree.map_int("map_i").unwrap(),       mi);
        assert_eq!(tree.map_int64("map_i64").unwrap(),   mi64);
        assert_eq!(tree.map_int128("map_i128").unwrap(), mi128);
        assert_eq!(tree.map_float64("map_f64").unwrap(), mf64);
        assert_eq!(tree.map_bool("map_b").unwrap(),      mb);
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
        assert!(matches!(
            tree.store("nonexistent", FigValue::Int(1)),
            Err(FigtreeError::UnknownKey(_))
        ));
    }

    #[test]
    fn test_store_type_mismatch_returns_error() {
        let mut tree = Tree::new();
        tree.new_int("workers", 4, "");
        tree.parse().unwrap();
        assert!(matches!(
            tree.store("workers", FigValue::String("oops".into())),
            Err(FigtreeError::TypeMismatch { .. })
        ));
    }

    // ── pollination ───────────────────────────────────────────────────────────

    #[test]
    fn test_pollinate_is_noop_when_disabled() {
        let mut tree = Tree::new();
        tree.new_int("workers", 4, "");
        tree.parse().unwrap();
        tree.pollinate().unwrap();
        assert_eq!(tree.integer("workers").unwrap(), 4);
    }

    #[test]
    fn test_pollinate_key_is_noop_when_disabled() {
        let mut tree = Tree::new();
        tree.new_int("workers", 4, "");
        tree.parse().unwrap();
        tree.pollinate_key("workers").unwrap();
        assert_eq!(tree.integer("workers").unwrap(), 4);
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
    fn test_validator_fails_on_invalid_default() {
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
        assert!(matches!(
            tree.with_validator("nonexistent", "v", |_| Ok(())),
            Err(FigtreeError::UnknownKey(_))
        ));
    }

    // ── callbacks ─────────────────────────────────────────────────────────────

    #[test]
    fn test_after_verify_fires_on_parse() {
        let fired     = Arc::new(Mutex::new(false));
        let fired_ref = fired.clone();
        let mut tree  = Tree::new();

        tree.new_int("workers", 4, "");
        tree.with_callback("workers", CallbackPhase::AfterVerify, move |_| {
            *fired_ref.lock().unwrap() = true;
            Ok(())
        }).unwrap();

        tree.parse().unwrap();
        assert!(*fired.lock().unwrap());
    }

    #[test]
    fn test_after_change_fires_on_store() {
        let received = Arc::new(Mutex::new(0i32));
        let recv_ref = received.clone();
        let mut tree = Tree::new();

        tree.new_int("workers", 4, "");
        tree.with_callback("workers", CallbackPhase::AfterChange, move |v| {
            if let FigValue::Int(n) = v { *recv_ref.lock().unwrap() = *n; }
            Ok(())
        }).unwrap();

        tree.parse().unwrap();
        tree.store("workers", FigValue::Int(16)).unwrap();
        assert_eq!(*received.lock().unwrap(), 16);
    }

    #[test]
    fn test_after_read_fires_on_getter() {
        let count     = Arc::new(Mutex::new(0usize));
        let count_ref = count.clone();
        let mut tree  = Tree::new();

        tree.new_int("workers", 4, "");
        tree.with_callback("workers", CallbackPhase::AfterRead, move |_| {
            *count_ref.lock().unwrap() += 1;
            Ok(())
        }).unwrap();

        tree.parse().unwrap();
        let _ = tree.integer("workers").unwrap();
        let _ = tree.integer("workers").unwrap();
        assert_eq!(*count.lock().unwrap(), 2);
    }

    // ── rules ─────────────────────────────────────────────────────────────────

    #[test]
    fn test_prevent_change_blocks_store() {
        let mut tree = Tree::new();
        tree.new_int("workers", 4, "");
        tree.with_rule("workers", Rule::PreventChange).unwrap();
        tree.parse().unwrap();
        assert!(matches!(
            tree.store("workers", FigValue::Int(8)),
            Err(FigtreeError::RuleViolation { .. })
        ));
    }

    #[test]
    fn test_no_validations_skips_validators() {
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
        assert!(tree.parse().is_ok());
        assert_eq!(tree.integer("workers").unwrap(), 0);
    }

    #[test]
    fn test_with_rule_unknown_key_returns_error() {
        let mut tree = Tree::new();
        assert!(matches!(
            tree.with_rule("nonexistent", Rule::PreventChange),
            Err(FigtreeError::UnknownKey(_))
        ));
    }

    #[test]
    fn test_no_maps_rejects_map_key_at_rule_time() {
        let mut tree = Tree::new();
        tree.new_map_string("metadata", HashMap::new(), "");
        assert!(matches!(
            tree.with_rule("metadata", Rule::NoMaps),
            Err(FigtreeError::RuleViolation { .. })
        ));
    }

    #[test]
    fn test_no_maps_allows_scalar_key() {
        let mut tree = Tree::new();
        tree.new_int("workers", 4, "");
        assert!(tree.with_rule("workers", Rule::NoMaps).is_ok());
    }

    #[test]
    fn test_no_maps_blocks_store_of_map_value() {
        let mut tree = Tree::new();
        tree.new_map_string("metadata", HashMap::new(), "");
        // can't apply NoMaps to a map key — apply it to a scalar key instead
        // then try to store a map value into it (after changing the type — use
        // a key that starts as a map and test store directly via the rule)
        // Actually test: scalar key with NoMaps, then store a map FigValue
        tree.new_string("tag", "hello", "");
        tree.with_rule("tag", Rule::NoMaps).unwrap();
        tree.parse().unwrap();
        let mut m = HashMap::new();
        m.insert("a".to_string(), "b".to_string());
        assert!(matches!(
            tree.store("tag", FigValue::MapString(m)),
            Err(FigtreeError::RuleViolation { .. })
        ));
    }

    #[test]
    fn test_no_lists_rejects_list_key_at_rule_time() {
        let mut tree = Tree::new();
        tree.new_list_string("tags", vec![], "");
        assert!(matches!(
            tree.with_rule("tags", Rule::NoLists),
            Err(FigtreeError::RuleViolation { .. })
        ));
    }

    #[test]
    fn test_no_lists_allows_scalar_key() {
        let mut tree = Tree::new();
        tree.new_int("workers", 4, "");
        assert!(tree.with_rule("workers", Rule::NoLists).is_ok());
    }

    #[test]
    fn test_no_lists_blocks_store_of_list_value() {
        let mut tree = Tree::new();
        tree.new_string("tag", "hello", "");
        tree.with_rule("tag", Rule::NoLists).unwrap();
        tree.parse().unwrap();
        assert!(matches!(
            tree.store("tag", FigValue::ListString(vec!["a".into()])),
            Err(FigtreeError::RuleViolation { .. })
        ));
    }

    // ── mutation tracking ─────────────────────────────────────────────────────

    #[test]
    fn test_mutations_returns_none_without_tracking() {
        let mut tree = Tree::new();
        assert!(tree.mutations().is_none());
    }

    #[test]
    fn test_mutations_returns_receiver_with_tracking() {
        let mut tree = Tree::grow();
        assert!(tree.mutations().is_some());
    }

    #[test]
    fn test_store_emits_mutation() {
        let mut tree = Tree::grow();
        let rx       = tree.mutations().unwrap();

        tree.new_int("workers", 4, "");
        tree.parse().unwrap();
        tree.store("workers", FigValue::Int(8)).unwrap();

        let mutations: Vec<_> = std::iter::from_fn(|| rx.try_recv()).collect();
        assert!(!mutations.is_empty());
        let last = mutations.last().unwrap();
        assert_eq!(last.key, "workers");
        assert_eq!(last.new, FigValue::Int(8));
    }

    #[test]
    fn test_curse_closes_channel() {
        let mut tree = Tree::grow();
        let rx       = tree.mutations().unwrap();
        tree.curse();
        assert!(rx.recv().is_none());
    }

    #[test]
    fn test_recall_reopens_channel() {
        let mut tree = Tree::grow();
        tree.curse();
        tree.recall();
        assert!(tree.mutation_tx.is_some());
    }

    // ── diagnostics ───────────────────────────────────────────────────────────

    #[test]
    fn test_usage_contains_keys_and_descriptions() {
        let mut tree = Tree::new();
        tree.new_int("workers", 4, "number of workers")
            .new_string("host", "localhost", "hostname");
        tree.parse().unwrap();
        let usage = tree.usage();
        assert!(usage.contains("workers"));
        assert!(usage.contains("host"));
        assert!(usage.contains("number of workers"));
        assert!(usage.contains("hostname"));
    }

    #[test]
    fn test_problems_empty_on_clean_tree() {
        let mut tree = Tree::new();
        tree.new_int("workers", 4, "");
        tree.parse().unwrap();
        assert!(tree.problems().is_empty());
    }

    #[test]
    fn test_history_contains_initialization_entry() {
        let mut tree = Tree::new();
        tree.new_int("workers", 4, "");
        tree.parse().unwrap();
        let history = tree.history("workers").unwrap();
        assert!(!history.is_empty());
        assert_eq!(history[0].state_index, 0);
    }

    #[test]
    fn test_history_log_is_readable() {
        let mut tree = Tree::new();
        tree.new_int("workers", 4, "");
        tree.parse().unwrap();
        tree.store("workers", FigValue::Int(8)).unwrap();
        let log = tree.history_log("workers").unwrap();
        assert!(log.contains("state 0"));
        assert!(log.contains("initialized"));
    }

    #[test]
    fn test_history_none_for_unknown_key() {
        let tree = Tree::new();
        assert!(tree.history("nonexistent").is_none());
    }

    // ── fig raw access ────────────────────────────────────────────────────────

    #[test]
    fn test_fig_returns_raw_fig() {
        let mut tree = Tree::new();
        tree.new_int("workers", 4, "");
        let fig = tree.fig("workers").unwrap();
        assert_eq!(fig.key, "workers");
    }

    #[test]
    fn test_fig_none_for_unknown_key() {
        let tree = Tree::new();
        assert!(tree.fig("nonexistent").is_none());
    }

    // ── int128 round-trip ─────────────────────────────────────────────────────

    #[test]
    fn test_int128_round_trips_beyond_i64_max() {
        let mut tree  = Tree::new();
        let big: i128 = i64::MAX as i128 + 1;
        tree.new_int128("big_id", big, "");
        tree.parse().unwrap();
        assert_eq!(tree.int128("big_id").unwrap(), big);
    }
}