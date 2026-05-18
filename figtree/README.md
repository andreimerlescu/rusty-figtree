# figtree

figtree is a layered runtime configuration system for Rust. It resolves configuration
values from multiple sources — CLI flags, environment variables, YAML files, JSON files,
INI files, and programmatic assignment — according to a fixed priority order, and
notifies your application of changes via typed mutation channels.

## Core Concepts

### The Fig Tree

A fig tree is your application's configuration state. You grow one at startup,
register your configuration keys with their types, defaults, validators, and callbacks,
then call parse or load to resolve all values from their sources.

    use figtree::prelude::*;

    #[derive(Figtree)]
    #[figtree(
        file = "/etc/myapp/config.yaml",
        file = "/etc/myapp/local.yaml",
        tracking = true,
    )]
    struct AppConfig {
        #[figtree(
            default = 10,
            env = "WORKERS",
            validate = assure_int_in_range(1, 64),
        )]
        workers: i64,

        #[figtree(
            default = "http://localhost:8080",
            env = "ENDPOINT",
            validate = assure_string_has_prefix("http"),
            validate = assure_string_not_empty,
        )]
        endpoint: String,

        #[figtree(
            default = 0.75,
            env = "THRESHOLD",
            validate = assure_float_in_range(0.0, 1.0),
        )]
        threshold: f64,
    }

    fn main() -> Result<(), FigtreeError> {
        let figs = AppConfig::grow()?;

        println!("workers:   {}", figs.workers());
        println!("endpoint:  {}", figs.endpoint());
        println!("threshold: {}", figs.threshold());

        Ok(())
    }

### Priority Resolution (PEMDAS)

Every key is resolved by consulting sources in this fixed order. The first source
that has a value for the key wins. Lower sources only speak when all higher sources
are silent.

    1. CLI flag         --workers 20
    2. Environment      WORKERS=20
    3. Config files     workers: 20  (in the order files were declared)
    4. Programmatic     figs.store("workers", 20)
    5. Default          the value declared in #[figtree(default = ...)]

This order is invariant. It cannot be changed at runtime. It is baked into the
generated code by the figtree-derive macro at compile time.

### The Fig

A Fig is the internal container for a single configuration value. It holds the
current value, the default, the source that provided the current value, any
validators assigned to it, any callbacks assigned to it, and any rules governing
how it can change. You rarely interact with Fig directly — the generated getters
and setters on your struct are the normal interface.

### Mutations

When tracking is enabled, figtree emits a typed Mutation value every time a key
changes. Mutations carry the key identity, the old value, the new value, and the
time the change occurred. Because Mutation is a generated enum with one variant per
field in your struct, the Rust compiler enforces exhaustive handling — you cannot
silently ignore a field that changed.

    let mut rx = figs.mutations();

    tokio::spawn(async move {
        while let Some(mutation) = rx.recv().await {
            match mutation {
                Mutation::Workers { old, new }   => { }
                Mutation::Endpoint { old, new }  => { }
                Mutation::Threshold { old, new } => { }
            }
        }
    });

## The #[figtree()] Attribute

The #[figtree()] attribute is placed on your struct to configure tree-level options,
and on individual fields to configure per-key behavior.

### Struct-Level Options

    Option          Type        Description
    ------          ----        -----------
    file            &str        Path to a config file to load. Multiple allowed.
    tracking        bool        Enable the mutation channel. Default false.
    pollinate       bool        Re-check env vars on every getter call. Default false.
    germinate       bool        Ignore -test. flags from the test runner. Default false.

### Field-Level Options

    Option          Type                Description
    ------          ----                -----------
    default         literal             The fallback value when no source provides one.
    env             &str                Environment variable name to read from.
    validate        ValidatorFunc       A validator to run after resolution. Multiple allowed.
    on_change       closure             Called when the value changes.
    on_verify       closure             Called during parse/load after validation.
    on_read         closure             Called every time the getter is called.
    rule            Rule                A behavioral rule applied to this key.

## Validators

Validators are functions that accept a value and return Result<(), FigtreeError>.
Multiple validators can be assigned to a single key. All must pass for parse or
load to succeed. The built-in validators follow the naming convention
assure_type_rule.

### String Validators

    assure_string_not_empty
    assure_string_length(n)
    assure_string_not_length(n)
    assure_string_has_prefix(prefix)
    assure_string_has_suffix(suffix)
    assure_string_no_prefix(prefix)
    assure_string_no_suffix(suffix)
    assure_string_contains(substring)
    assure_string_not_contains(substring)

### Integer Validators

    assure_int_positive
    assure_int_negative
    assure_int_greater_than(n)
    assure_int_less_than(n)
    assure_int_in_range(min, max)

### Float Validators

    assure_float_positive
    assure_float_not_nan
    assure_float_greater_than(n)
    assure_float_less_than(n)
    assure_float_in_range(min, max)

### Duration Validators

    assure_duration_positive
    assure_duration_greater_than(d)
    assure_duration_less_than(d)
    assure_duration_min(d)
    assure_duration_max(d)

### Collection Validators

    assure_list_not_empty
    assure_list_min_length(n)
    assure_list_length(n)
    assure_list_contains(value)
    assure_list_not_contains(value)
    assure_map_not_empty
    assure_map_has_key(key)
    assure_map_has_keys(keys)
    assure_map_length(n)

## Rules

Rules govern how a key behaves after it has been resolved. A rule is applied to a
single field via #[figtree(rule = ...)].

    Rule                            Effect
    ----                            ------
    RulePreventChange               Any attempt to store a new value is rejected.
    RulePanicOnChange               Any attempt to store a new value panics.
    RuleNoValidations               All validators for this key are skipped.
    RuleNoCallbacks                 All callbacks for this key are skipped.
    RuleNoFlags                     CLI flag source is ignored for this key.
    RuleNoEnv                       Environment variable source is ignored for this key.

## Callbacks

Callbacks are closures attached to a field that fire at specific points in the
key's lifecycle. Multiple callbacks of different types can be attached to a single
key. Callbacks that return an error cause parse or load to fail.

    Callback        When It Fires
    --------        -------------
    on_verify       After validation passes during parse or load.
    on_read         Every time the getter for this key is called.
    on_change       Every time the key's value changes via store.

## Configuration Sources

Sources are enabled via feature flags in Cargo.toml.

    Source          Feature Flag    Description
    ------          ------------    -----------
    Environment     env             Reads from os environment variables.
    YAML            yaml            Parses .yaml and .yml files via serde_yaml.
    JSON            json            Parses .json files via serde_json.
    INI             ini             Parses .ini files via rust-ini.
    CLI             cli             Integrates with clap for flag parsing.
    Embedded        embedded        Reads from EEPROM or flash on no_std targets.

## Feature Flags

    [dependencies]
    figtree = { version = "0.0.1", features = ["yaml", "json", "cli", "async"] }

The default features are std, env, yaml, and json. The embedded feature disables
std and all file-based sources, enabling use on bare metal targets.

## Async Mutation Tracking

When the async feature is enabled, the mutations channel is backed by
tokio::sync::broadcast, making it suitable for use in async Tokio runtimes.
When async is disabled, mutations uses std::sync::mpsc instead.
