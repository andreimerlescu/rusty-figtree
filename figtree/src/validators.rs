use crate::error::{FigtreeError, FigtreeResult};
use crate::fig::FigValue;
use std::time::Duration;

// ── ValidatorFn ───────────────────────────────────────────────────────────────

/// The function signature all validators must satisfy.
///
/// Receives a reference to the FigValue to validate. Returns Ok(())
/// to accept the value or Err(FigtreeError::ValidationFailed) to
/// reject it. The key name is not available inside the validator —
/// the Tree wraps the error with the key name before propagating it.
///
/// Stored as a boxed trait object so validators of any concrete type
/// can live together in a Vec without the container being generic.
/// Send + Sync are required because the Tree may be used across threads.
pub type ValidatorFn = Box<dyn Fn(&FigValue) -> FigtreeResult<()> + Send + Sync>;

// ── ValidatorRegistry ─────────────────────────────────────────────────────────

/// An ordered collection of validators for a single Fig.
///
/// Validators run in registration order. The first failure stops
/// execution and returns its error — subsequent validators are not
/// run. This matches figtree Go behavior.
#[derive(Debug, Default)]
pub struct ValidatorRegistry {
    validators: Vec<NamedValidator>,
}

/// A validator paired with a human-readable name for diagnostics.
pub struct NamedValidator {
    /// Short name describing what this validator checks.
    /// Appears in error messages and debug output.
    pub name: String,

    /// The validator function.
    pub func: ValidatorFn,
}

impl std::fmt::Debug for NamedValidator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NamedValidator")
            .field("name", &self.name)
            .field("func", &"<closure>")
            .finish()
    }
}

impl ValidatorRegistry {
    /// Creates an empty registry.
    pub fn new() -> Self {
        ValidatorRegistry {
            validators: Vec::new(),
        }
    }

    /// Registers a named validator.
    pub fn register<F>(&mut self, name: impl Into<String>, func: F)
    where
        F: Fn(&FigValue) -> FigtreeResult<()> + Send + Sync + 'static,
    {
        self.validators.push(NamedValidator {
            name: name.into(),
            func: Box::new(func),
        });
    }

    /// Runs all registered validators against the given value in
    /// registration order. Stops and returns the first error.
    pub fn validate(&self, value: &FigValue) -> FigtreeResult<()> {
        for v in &self.validators {
            (v.func)(value)?;
        }
        Ok(())
    }

    /// Returns the number of registered validators.
    pub fn len(&self) -> usize {
        self.validators.len()
    }

    /// Returns true if no validators are registered.
    pub fn is_empty(&self) -> bool {
        self.validators.is_empty()
    }
}

// ── Rejection helper ──────────────────────────────────────────────────────────

/// Produces a ValidationFailed error with a formatted message.
/// Used by all built-in validators to keep error construction consistent.
fn reject(message: impl Into<String>) -> FigtreeResult<()> {
    Err(FigtreeError::ValidationFailed {
        key:     String::new(), // Tree fills this in with the actual key
        message: message.into(),
    })
}

// ── String validators ─────────────────────────────────────────────────────────

/// Rejects empty strings.
pub fn assure_string_not_empty(value: &FigValue) -> FigtreeResult<()> {
    match value {
        FigValue::String(s) if s.is_empty() => reject("string must not be empty"),
        FigValue::String(_)                 => Ok(()),
        _                                   => reject(format!("expected String, got {}", value.type_name())),
    }
}

/// Rejects strings whose length is not exactly `n`.
pub fn assure_string_length(n: usize) -> ValidatorFn {
    Box::new(move |value| match value {
        FigValue::String(s) if s.len() == n => Ok(()),
        FigValue::String(s) => reject(format!(
            "string length must be exactly {}, got {}", n, s.len()
        )),
        _ => reject(format!("expected String, got {}", value.type_name())),
    })
}

/// Rejects strings whose length is exactly `n`.
pub fn assure_string_not_length(n: usize) -> ValidatorFn {
    Box::new(move |value| match value {
        FigValue::String(s) if s.len() != n => Ok(()),
        FigValue::String(_) => reject(format!(
            "string length must not be {}", n
        )),
        _ => reject(format!("expected String, got {}", value.type_name())),
    })
}

/// Rejects strings shorter than `n` characters.
pub fn assure_string_length_greater_than(n: usize) -> ValidatorFn {
    Box::new(move |value| match value {
        FigValue::String(s) if s.len() > n => Ok(()),
        FigValue::String(s) => reject(format!(
            "string length must be greater than {}, got {}", n, s.len()
        )),
        _ => reject(format!("expected String, got {}", value.type_name())),
    })
}

/// Rejects strings that do not contain `substring`.
pub fn assure_string_contains(substring: impl Into<String>) -> ValidatorFn {
    let sub = substring.into();
    Box::new(move |value| match value {
        FigValue::String(s) if s.contains(&sub) => Ok(()),
        FigValue::String(_) => reject(format!(
            "string must contain '{}'", sub
        )),
        _ => reject(format!("expected String, got {}", value.type_name())),
    })
}

/// Rejects strings that contain `substring`.
pub fn assure_string_not_contains(substring: impl Into<String>) -> ValidatorFn {
    let sub = substring.into();
    Box::new(move |value| match value {
        FigValue::String(s) if !s.contains(&sub) => Ok(()),
        FigValue::String(_) => reject(format!(
            "string must not contain '{}'", sub
        )),
        _ => reject(format!("expected String, got {}", value.type_name())),
    })
}

/// Rejects strings that do not start with `prefix`.
pub fn assure_string_has_prefix(prefix: impl Into<String>) -> ValidatorFn {
    let p = prefix.into();
    Box::new(move |value| match value {
        FigValue::String(s) if s.starts_with(&p) => Ok(()),
        FigValue::String(_) => reject(format!(
            "string must have prefix '{}'", p
        )),
        _ => reject(format!("expected String, got {}", value.type_name())),
    })
}

/// Rejects strings that start with `prefix`.
pub fn assure_string_no_prefix(prefix: impl Into<String>) -> ValidatorFn {
    let p = prefix.into();
    Box::new(move |value| match value {
        FigValue::String(s) if !s.starts_with(&p) => Ok(()),
        FigValue::String(_) => reject(format!(
            "string must not have prefix '{}'", p
        )),
        _ => reject(format!("expected String, got {}", value.type_name())),
    })
}

/// Rejects strings that do not end with `suffix`.
pub fn assure_string_has_suffix(suffix: impl Into<String>) -> ValidatorFn {
    let s = suffix.into();
    Box::new(move |value| match value {
        FigValue::String(v) if v.ends_with(&s) => Ok(()),
        FigValue::String(_) => reject(format!(
            "string must have suffix '{}'", s
        )),
        _ => reject(format!("expected String, got {}", value.type_name())),
    })
}

/// Rejects strings that end with `suffix`.
pub fn assure_string_no_suffix(suffix: impl Into<String>) -> ValidatorFn {
    let s = suffix.into();
    Box::new(move |value| match value {
        FigValue::String(v) if !v.ends_with(&s) => Ok(()),
        FigValue::String(_) => reject(format!(
            "string must not have suffix '{}'", s
        )),
        _ => reject(format!("expected String, got {}", value.type_name())),
    })
}

/// Rejects strings that start with any of the given prefixes.
pub fn assure_string_no_prefixes(prefixes: Vec<String>) -> ValidatorFn {
    Box::new(move |value| match value {
        FigValue::String(s) => {
            for p in &prefixes {
                if s.starts_with(p.as_str()) {
                    return reject(format!("string must not have prefix '{}'", p));
                }
            }
            Ok(())
        }
        _ => reject(format!("expected String, got {}", value.type_name())),
    })
}

/// Rejects strings that end with any of the given suffixes.
pub fn assure_string_no_suffixes(suffixes: Vec<String>) -> ValidatorFn {
    Box::new(move |value| match value {
        FigValue::String(s) => {
            for sfx in &suffixes {
                if s.ends_with(sfx.as_str()) {
                    return reject(format!("string must not have suffix '{}'", sfx));
                }
            }
            Ok(())
        }
        _ => reject(format!("expected String, got {}", value.type_name())),
    })
}

// ── Bool validators ───────────────────────────────────────────────────────────

/// Rejects false values.
pub fn assure_bool_true(value: &FigValue) -> FigtreeResult<()> {
    match value {
        FigValue::Bool(true)  => Ok(()),
        FigValue::Bool(false) => reject("bool must be true"),
        _                     => reject(format!("expected Bool, got {}", value.type_name())),
    }
}

/// Rejects true values.
pub fn assure_bool_false(value: &FigValue) -> FigtreeResult<()> {
    match value {
        FigValue::Bool(false) => Ok(()),
        FigValue::Bool(true)  => reject("bool must be false"),
        _                     => reject(format!("expected Bool, got {}", value.type_name())),
    }
}

// ── Int validators ────────────────────────────────────────────────────────────

/// Rejects Int values that are not positive (greater than zero).
pub fn assure_int_positive(value: &FigValue) -> FigtreeResult<()> {
    match value {
        FigValue::Int(n) if *n > 0 => Ok(()),
        FigValue::Int(n)           => reject(format!("int must be positive, got {}", n)),
        _                          => reject(format!("expected Int, got {}", value.type_name())),
    }
}

/// Rejects Int values that are not negative (less than zero).
pub fn assure_int_negative(value: &FigValue) -> FigtreeResult<()> {
    match value {
        FigValue::Int(n) if *n < 0 => Ok(()),
        FigValue::Int(n)           => reject(format!("int must be negative, got {}", n)),
        _                          => reject(format!("expected Int, got {}", value.type_name())),
    }
}

/// Rejects Int values not greater than `n` (exclusive).
pub fn assure_int_greater_than(n: i32) -> ValidatorFn {
    Box::new(move |value| match value {
        FigValue::Int(v) if *v > n => Ok(()),
        FigValue::Int(v)           => reject(format!("int must be greater than {}, got {}", n, v)),
        _                          => reject(format!("expected Int, got {}", value.type_name())),
    })
}

/// Rejects Int values not less than `n` (exclusive).
pub fn assure_int_less_than(n: i32) -> ValidatorFn {
    Box::new(move |value| match value {
        FigValue::Int(v) if *v < n => Ok(()),
        FigValue::Int(v)           => reject(format!("int must be less than {}, got {}", n, v)),
        _                          => reject(format!("expected Int, got {}", value.type_name())),
    })
}

/// Rejects Int values outside the inclusive range [min, max].
pub fn assure_int_in_range(min: i32, max: i32) -> ValidatorFn {
    Box::new(move |value| match value {
        FigValue::Int(v) if *v >= min && *v <= max => Ok(()),
        FigValue::Int(v) => reject(format!(
            "int must be in range [{}, {}], got {}", min, max, v
        )),
        _ => reject(format!("expected Int, got {}", value.type_name())),
    })
}

// ── Int64 validators ──────────────────────────────────────────────────────────

/// Rejects Int64 values that are not positive.
pub fn assure_int64_positive(value: &FigValue) -> FigtreeResult<()> {
    match value {
        FigValue::Int64(n) if *n > 0 => Ok(()),
        FigValue::Int64(n)           => reject(format!("int64 must be positive, got {}", n)),
        _                            => reject(format!("expected Int64, got {}", value.type_name())),
    }
}

/// Rejects Int64 values not greater than `n` (exclusive).
pub fn assure_int64_greater_than(n: i64) -> ValidatorFn {
    Box::new(move |value| match value {
        FigValue::Int64(v) if *v > n => Ok(()),
        FigValue::Int64(v)           => reject(format!("int64 must be greater than {}, got {}", n, v)),
        _                            => reject(format!("expected Int64, got {}", value.type_name())),
    })
}

/// Rejects Int64 values not less than `n` (exclusive).
pub fn assure_int64_less_than(n: i64) -> ValidatorFn {
    Box::new(move |value| match value {
        FigValue::Int64(v) if *v < n => Ok(()),
        FigValue::Int64(v)           => reject(format!("int64 must be less than {}, got {}", n, v)),
        _                            => reject(format!("expected Int64, got {}", value.type_name())),
    })
}

/// Rejects Int64 values outside the inclusive range [min, max].
pub fn assure_int64_in_range(min: i64, max: i64) -> ValidatorFn {
    Box::new(move |value| match value {
        FigValue::Int64(v) if *v >= min && *v <= max => Ok(()),
        FigValue::Int64(v) => reject(format!(
            "int64 must be in range [{}, {}], got {}", min, max, v
        )),
        _ => reject(format!("expected Int64, got {}", value.type_name())),
    })
}

// ── Int128 validators ─────────────────────────────────────────────────────────

/// Rejects Int128 values that are not positive.
pub fn assure_int128_positive(value: &FigValue) -> FigtreeResult<()> {
    match value {
        FigValue::Int128(n) if *n > 0 => Ok(()),
        FigValue::Int128(n)           => reject(format!("int128 must be positive, got {}", n)),
        _                             => reject(format!("expected Int128, got {}", value.type_name())),
    }
}

/// Rejects Int128 values not greater than `n` (exclusive).
pub fn assure_int128_greater_than(n: i128) -> ValidatorFn {
    Box::new(move |value| match value {
        FigValue::Int128(v) if *v > n => Ok(()),
        FigValue::Int128(v)           => reject(format!("int128 must be greater than {}, got {}", n, v)),
        _                             => reject(format!("expected Int128, got {}", value.type_name())),
    })
}

/// Rejects Int128 values not less than `n` (exclusive).
pub fn assure_int128_less_than(n: i128) -> ValidatorFn {
    Box::new(move |value| match value {
        FigValue::Int128(v) if *v < n => Ok(()),
        FigValue::Int128(v)           => reject(format!("int128 must be less than {}, got {}", n, v)),
        _                             => reject(format!("expected Int128, got {}", value.type_name())),
    })
}

/// Rejects Int128 values outside the inclusive range [min, max].
pub fn assure_int128_in_range(min: i128, max: i128) -> ValidatorFn {
    Box::new(move |value| match value {
        FigValue::Int128(v) if *v >= min && *v <= max => Ok(()),
        FigValue::Int128(v) => reject(format!(
            "int128 must be in range [{}, {}], got {}", min, max, v
        )),
        _ => reject(format!("expected Int128, got {}", value.type_name())),
    })
}

// ── Float64 validators ────────────────────────────────────────────────────────

/// Rejects Float64 values that are NaN.
pub fn assure_float64_not_nan(value: &FigValue) -> FigtreeResult<()> {
    match value {
        FigValue::Float64(v) if !v.is_nan() => Ok(()),
        FigValue::Float64(_)                => reject("float64 must not be NaN"),
        _                                   => reject(format!("expected Float64, got {}", value.type_name())),
    }
}

/// Rejects Float64 values that are not positive (greater than zero).
pub fn assure_float64_positive(value: &FigValue) -> FigtreeResult<()> {
    match value {
        FigValue::Float64(v) if *v > 0.0 => Ok(()),
        FigValue::Float64(v)             => reject(format!("float64 must be positive, got {}", v)),
        _                                => reject(format!("expected Float64, got {}", value.type_name())),
    }
}

/// Rejects Float64 values not greater than `n` (exclusive).
pub fn assure_float64_greater_than(n: f64) -> ValidatorFn {
    Box::new(move |value| match value {
        FigValue::Float64(v) if *v > n => Ok(()),
        FigValue::Float64(v)           => reject(format!("float64 must be greater than {}, got {}", n, v)),
        _                              => reject(format!("expected Float64, got {}", value.type_name())),
    })
}

/// Rejects Float64 values not less than `n` (exclusive).
pub fn assure_float64_less_than(n: f64) -> ValidatorFn {
    Box::new(move |value| match value {
        FigValue::Float64(v) if *v < n => Ok(()),
        FigValue::Float64(v)           => reject(format!("float64 must be less than {}, got {}", n, v)),
        _                              => reject(format!("expected Float64, got {}", value.type_name())),
    })
}

/// Rejects Float64 values outside the inclusive range [min, max].
pub fn assure_float64_in_range(min: f64, max: f64) -> ValidatorFn {
    Box::new(move |value| match value {
        FigValue::Float64(v) if *v >= min && *v <= max => Ok(()),
        FigValue::Float64(v) => reject(format!(
            "float64 must be in range [{}, {}], got {}", min, max, v
        )),
        _ => reject(format!("expected Float64, got {}", value.type_name())),
    })
}

// ── Float128 validators ───────────────────────────────────────────────────────

/// Rejects Float128 values that are NaN.
pub fn assure_float128_not_nan(value: &FigValue) -> FigtreeResult<()> {
    match value {
        FigValue::Float128(v) if !v.is_nan() => Ok(()),
        FigValue::Float128(_)                => reject("float128 must not be NaN"),
        _                                    => reject(format!("expected Float128, got {}", value.type_name())),
    }
}

/// Rejects Float128 values that are not positive.
pub fn assure_float128_positive(value: &FigValue) -> FigtreeResult<()> {
    match value {
        FigValue::Float128(v) if *v > 0.0 => Ok(()),
        FigValue::Float128(v)             => reject(format!("float128 must be positive, got {}", v)),
        _                                 => reject(format!("expected Float128, got {}", value.type_name())),
    }
}

/// Rejects Float128 values outside the inclusive range [min, max].
pub fn assure_float128_in_range(min: f64, max: f64) -> ValidatorFn {
    Box::new(move |value| match value {
        FigValue::Float128(v) if *v >= min && *v <= max => Ok(()),
        FigValue::Float128(v) => reject(format!(
            "float128 must be in range [{}, {}], got {}", min, max, v
        )),
        _ => reject(format!("expected Float128, got {}", value.type_name())),
    })
}

// ── Duration validators ───────────────────────────────────────────────────────

/// Rejects Duration values of zero.
pub fn assure_duration_positive(value: &FigValue) -> FigtreeResult<()> {
    match value {
        FigValue::Duration(d) if *d > Duration::ZERO => Ok(()),
        FigValue::Duration(_)                        => reject("duration must be positive"),
        _                                            => reject(format!("expected Duration, got {}", value.type_name())),
    }
}

/// Rejects Duration values not greater than `d` (exclusive).
pub fn assure_duration_greater_than(d: Duration) -> ValidatorFn {
    Box::new(move |value| match value {
        FigValue::Duration(v) if *v > d => Ok(()),
        FigValue::Duration(v)           => reject(format!(
            "duration must be greater than {:?}, got {:?}", d, v
        )),
        _ => reject(format!("expected Duration, got {}", value.type_name())),
    })
}

/// Rejects Duration values not less than `d` (exclusive).
pub fn assure_duration_less_than(d: Duration) -> ValidatorFn {
    Box::new(move |value| match value {
        FigValue::Duration(v) if *v < d => Ok(()),
        FigValue::Duration(v)           => reject(format!(
            "duration must be less than {:?}, got {:?}", d, v
        )),
        _ => reject(format!("expected Duration, got {}", value.type_name())),
    })
}

/// Rejects Duration values less than `min` (inclusive lower bound).
pub fn assure_duration_min(min: Duration) -> ValidatorFn {
    Box::new(move |value| match value {
        FigValue::Duration(v) if *v >= min => Ok(()),
        FigValue::Duration(v)              => reject(format!(
            "duration must be at least {:?}, got {:?}", min, v
        )),
        _ => reject(format!("expected Duration, got {}", value.type_name())),
    })
}

/// Rejects Duration values greater than `max` (inclusive upper bound).
pub fn assure_duration_max(max: Duration) -> ValidatorFn {
    Box::new(move |value| match value {
        FigValue::Duration(v) if *v <= max => Ok(()),
        FigValue::Duration(v)              => reject(format!(
            "duration must not exceed {:?}, got {:?}", max, v
        )),
        _ => reject(format!("expected Duration, got {}", value.type_name())),
    })
}

// ── List validators ───────────────────────────────────────────────────────────

/// Rejects any List variant that is empty.
pub fn assure_list_not_empty(value: &FigValue) -> FigtreeResult<()> {
    let empty = match value {
        FigValue::ListString(v)   => v.is_empty(),
        FigValue::ListInt(v)      => v.is_empty(),
        FigValue::ListInt64(v)    => v.is_empty(),
        FigValue::ListInt128(v)   => v.is_empty(),
        FigValue::ListFloat64(v)  => v.is_empty(),
        FigValue::ListFloat128(v) => v.is_empty(),
        FigValue::ListBool(v)     => v.is_empty(),
        _                         => return reject(format!("expected a List variant, got {}", value.type_name())),
    };
    if empty { reject("list must not be empty") } else { Ok(()) }
}

/// Rejects any List variant with fewer than `min` elements.
pub fn assure_list_min_length(min: usize) -> ValidatorFn {
    Box::new(move |value| {
        let len = match value {
            FigValue::ListString(v)   => v.len(),
            FigValue::ListInt(v)      => v.len(),
            FigValue::ListInt64(v)    => v.len(),
            FigValue::ListInt128(v)   => v.len(),
            FigValue::ListFloat64(v)  => v.len(),
            FigValue::ListFloat128(v) => v.len(),
            FigValue::ListBool(v)     => v.len(),
            _ => return reject(format!("expected a List variant, got {}", value.type_name())),
        };
        if len >= min {
            Ok(())
        } else {
            reject(format!("list must have at least {} elements, got {}", min, len))
        }
    })
}

/// Rejects any List variant whose length is not exactly `n`.
pub fn assure_list_length(n: usize) -> ValidatorFn {
    Box::new(move |value| {
        let len = match value {
            FigValue::ListString(v)   => v.len(),
            FigValue::ListInt(v)      => v.len(),
            FigValue::ListInt64(v)    => v.len(),
            FigValue::ListInt128(v)   => v.len(),
            FigValue::ListFloat64(v)  => v.len(),
            FigValue::ListFloat128(v) => v.len(),
            FigValue::ListBool(v)     => v.len(),
            _ => return reject(format!("expected a List variant, got {}", value.type_name())),
        };
        if len == n {
            Ok(())
        } else {
            reject(format!("list must have exactly {} elements, got {}", n, len))
        }
    })
}

/// Rejects any List variant whose length is exactly `n`.
pub fn assure_list_not_length(n: usize) -> ValidatorFn {
    Box::new(move |value| {
        let len = match value {
            FigValue::ListString(v)   => v.len(),
            FigValue::ListInt(v)      => v.len(),
            FigValue::ListInt64(v)    => v.len(),
            FigValue::ListInt128(v)   => v.len(),
            FigValue::ListFloat64(v)  => v.len(),
            FigValue::ListFloat128(v) => v.len(),
            FigValue::ListBool(v)     => v.len(),
            _ => return reject(format!("expected a List variant, got {}", value.type_name())),
        };
        if len != n {
            Ok(())
        } else {
            reject(format!("list must not have exactly {} elements", n))
        }
    })
}

/// Rejects ListString values that do not contain `item`.
pub fn assure_list_contains(item: impl Into<String>) -> ValidatorFn {
    let item = item.into();
    Box::new(move |value| match value {
        FigValue::ListString(v) if v.contains(&item) => Ok(()),
        FigValue::ListString(_) => reject(format!(
            "list must contain '{}'", item
        )),
        _ => reject(format!("expected ListString, got {}", value.type_name())),
    })
}

/// Rejects ListString values that contain `item`.
pub fn assure_list_not_contains(item: impl Into<String>) -> ValidatorFn {
    let item = item.into();
    Box::new(move |value| match value {
        FigValue::ListString(v) if !v.contains(&item) => Ok(()),
        FigValue::ListString(_) => reject(format!(
            "list must not contain '{}'", item
        )),
        _ => reject(format!("expected ListString, got {}", value.type_name())),
    })
}

// ── Map validators ────────────────────────────────────────────────────────────

/// Rejects any Map variant that is empty.
pub fn assure_map_not_empty(value: &FigValue) -> FigtreeResult<()> {
    let empty = match value {
        FigValue::MapString(v)   => v.is_empty(),
        FigValue::MapInt(v)      => v.is_empty(),
        FigValue::MapInt64(v)    => v.is_empty(),
        FigValue::MapInt128(v)   => v.is_empty(),
        FigValue::MapFloat64(v)  => v.is_empty(),
        FigValue::MapFloat128(v) => v.is_empty(),
        FigValue::MapBool(v)     => v.is_empty(),
        _ => return reject(format!("expected a Map variant, got {}", value.type_name())),
    };
    if empty { reject("map must not be empty") } else { Ok(()) }
}

/// Rejects any Map variant whose length is not exactly `n`.
pub fn assure_map_length(n: usize) -> ValidatorFn {
    Box::new(move |value| {
        let len = match value {
            FigValue::MapString(v)   => v.len(),
            FigValue::MapInt(v)      => v.len(),
            FigValue::MapInt64(v)    => v.len(),
            FigValue::MapInt128(v)   => v.len(),
            FigValue::MapFloat64(v)  => v.len(),
            FigValue::MapFloat128(v) => v.len(),
            FigValue::MapBool(v)     => v.len(),
            _ => return reject(format!("expected a Map variant, got {}", value.type_name())),
        };
        if len == n {
            Ok(())
        } else {
            reject(format!("map must have exactly {} entries, got {}", n, len))
        }
    })
}

/// Rejects any Map variant whose length is exactly `n`.
pub fn assure_map_not_length(n: usize) -> ValidatorFn {
    Box::new(move |value| {
        let len = match value {
            FigValue::MapString(v)   => v.len(),
            FigValue::MapInt(v)      => v.len(),
            FigValue::MapInt64(v)    => v.len(),
            FigValue::MapInt128(v)   => v.len(),
            FigValue::MapFloat64(v)  => v.len(),
            FigValue::MapFloat128(v) => v.len(),
            FigValue::MapBool(v)     => v.len(),
            _ => return reject(format!("expected a Map variant, got {}", value.type_name())),
        };
        if len != n {
            Ok(())
        } else {
            reject(format!("map must not have exactly {} entries", n))
        }
    })
}

/// Rejects MapString values that do not contain `key`.
pub fn assure_map_has_key(key: impl Into<String>) -> ValidatorFn {
    let k = key.into();
    Box::new(move |value| match value {
        FigValue::MapString(m) if m.contains_key(&k) => Ok(()),
        FigValue::MapString(_) => reject(format!(
            "map must contain key '{}'", k
        )),
        _ => reject(format!("expected MapString, got {}", value.type_name())),
    })
}

/// Rejects MapString values that do not contain all of `keys`.
pub fn assure_map_has_keys(keys: Vec<String>) -> ValidatorFn {
    Box::new(move |value| match value {
        FigValue::MapString(m) => {
            for k in &keys {
                if !m.contains_key(k.as_str()) {
                    return reject(format!("map must contain key '{}'", k));
                }
            }
            Ok(())
        }
        _ => reject(format!("expected MapString, got {}", value.type_name())),
    })
}

/// Rejects MapString values where `key` does not map to `expected`.
pub fn assure_map_value_matches(
    key:      impl Into<String>,
    expected: impl Into<String>,
) -> ValidatorFn {
    let k = key.into();
    let e = expected.into();
    Box::new(move |value| match value {
        FigValue::MapString(m) => match m.get(&k) {
            Some(v) if v == &e => Ok(()),
            Some(v) => reject(format!(
                "map key '{}' must equal '{}', got '{}'", k, e, v
            )),
            None => reject(format!("map must contain key '{}'", k)),
        },
        _ => reject(format!("expected MapString, got {}", value.type_name())),
    })
}

// ── ValidatorRegistry — cross-thread safety ───────────────────────────────────

unsafe impl Send for ValidatorRegistry {}
unsafe impl Sync for ValidatorRegistry {}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::time::Duration;

    // ── ValidatorRegistry ─────────────────────────────────────────────────────

    #[test]
    fn test_registry_empty_on_creation() {
        assert!(ValidatorRegistry::new().is_empty());
    }

    #[test]
    fn test_registry_runs_validators_in_order() {
        let mut r = ValidatorRegistry::new();
        let log   = std::sync::Arc::new(std::sync::Mutex::new(Vec::<usize>::new()));

        let log1 = log.clone();
        r.register("first", move |_| { log1.lock().unwrap().push(1); Ok(()) });

        let log2 = log.clone();
        r.register("second", move |_| { log2.lock().unwrap().push(2); Ok(()) });

        r.validate(&FigValue::Int(1)).unwrap();
        assert_eq!(*log.lock().unwrap(), vec![1, 2]);
    }

    #[test]
    fn test_registry_stops_on_first_failure() {
        let mut r       = ValidatorRegistry::new();
        let second_ran  = std::sync::Arc::new(std::sync::Mutex::new(false));
        let second_ref  = second_ran.clone();

        r.register("fail", |_| Err(FigtreeError::ValidationFailed {
            key: String::new(), message: "nope".into(),
        }));
        r.register("second", move |_| {
            *second_ref.lock().unwrap() = true;
            Ok(())
        });

        assert!(r.validate(&FigValue::Int(1)).is_err());
        assert!(!*second_ran.lock().unwrap());
    }

    // ── String validators ─────────────────────────────────────────────────────

    #[test]
    fn test_assure_string_not_empty_accepts_non_empty() {
        assert!(assure_string_not_empty(&FigValue::String("hello".into())).is_ok());
    }

    #[test]
    fn test_assure_string_not_empty_rejects_empty() {
        assert!(assure_string_not_empty(&FigValue::String(String::new())).is_err());
    }

    #[test]
    fn test_assure_string_length_accepts_exact() {
        assert!(assure_string_length(5)(&FigValue::String("hello".into())).is_ok());
    }

    #[test]
    fn test_assure_string_length_rejects_wrong() {
        assert!(assure_string_length(5)(&FigValue::String("hi".into())).is_err());
    }

    #[test]
    fn test_assure_string_has_prefix_accepts() {
        assert!(assure_string_has_prefix("http")(&FigValue::String("https://x.com".into())).is_ok());
    }

    #[test]
    fn test_assure_string_has_prefix_rejects() {
        assert!(assure_string_has_prefix("http")(&FigValue::String("ftp://x.com".into())).is_err());
    }

    #[test]
    fn test_assure_string_has_suffix_accepts() {
        assert!(assure_string_has_suffix(".yaml")(&FigValue::String("config.yaml".into())).is_ok());
    }

    #[test]
    fn test_assure_string_contains_accepts() {
        assert!(assure_string_contains("world")(&FigValue::String("hello world".into())).is_ok());
    }

    #[test]
    fn test_assure_string_not_contains_rejects() {
        assert!(assure_string_not_contains("bad")(&FigValue::String("bad word".into())).is_err());
    }

    #[test]
    fn test_assure_string_no_prefixes_rejects_matching() {
        let v = assure_string_no_prefixes(vec!["http".into(), "ftp".into()]);
        assert!(v(&FigValue::String("http://x.com".into())).is_err());
        assert!(v(&FigValue::String("sftp://x.com".into())).is_ok());
    }

    #[test]
    fn test_assure_string_no_suffixes_rejects_matching() {
        let v = assure_string_no_suffixes(vec![".exe".into(), ".bat".into()]);
        assert!(v(&FigValue::String("setup.exe".into())).is_err());
        assert!(v(&FigValue::String("setup.sh".into())).is_ok());
    }

    // ── Bool validators ───────────────────────────────────────────────────────

    #[test]
    fn test_assure_bool_true_accepts_true() {
        assert!(assure_bool_true(&FigValue::Bool(true)).is_ok());
    }

    #[test]
    fn test_assure_bool_true_rejects_false() {
        assert!(assure_bool_true(&FigValue::Bool(false)).is_err());
    }

    #[test]
    fn test_assure_bool_false_accepts_false() {
        assert!(assure_bool_false(&FigValue::Bool(false)).is_ok());
    }

    // ── Int validators ────────────────────────────────────────────────────────

    #[test]
    fn test_assure_int_positive_accepts() {
        assert!(assure_int_positive(&FigValue::Int(1)).is_ok());
    }

    #[test]
    fn test_assure_int_positive_rejects_zero() {
        assert!(assure_int_positive(&FigValue::Int(0)).is_err());
    }

    #[test]
    fn test_assure_int_in_range_accepts_boundary() {
        assert!(assure_int_in_range(1, 10)(&FigValue::Int(1)).is_ok());
        assert!(assure_int_in_range(1, 10)(&FigValue::Int(10)).is_ok());
    }

    #[test]
    fn test_assure_int_in_range_rejects_outside() {
        assert!(assure_int_in_range(1, 10)(&FigValue::Int(0)).is_err());
        assert!(assure_int_in_range(1, 10)(&FigValue::Int(11)).is_err());
    }

    #[test]
    fn test_assure_int_greater_than_accepts() {
        assert!(assure_int_greater_than(5)(&FigValue::Int(6)).is_ok());
    }

    #[test]
    fn test_assure_int_greater_than_rejects_equal() {
        assert!(assure_int_greater_than(5)(&FigValue::Int(5)).is_err());
    }

    // ── Int64 validators ──────────────────────────────────────────────────────

    #[test]
    fn test_assure_int64_in_range_accepts() {
        assert!(assure_int64_in_range(0, i64::MAX)(&FigValue::Int64(1000)).is_ok());
    }

    #[test]
    fn test_assure_int64_positive_rejects_negative() {
        assert!(assure_int64_positive(&FigValue::Int64(-1)).is_err());
    }

    // ── Int128 validators ─────────────────────────────────────────────────────

    #[test]
    fn test_assure_int128_in_range_accepts_beyond_i64_max() {
        let big = i64::MAX as i128 + 1;
        assert!(assure_int128_in_range(0, i128::MAX)(&FigValue::Int128(big)).is_ok());
    }

    #[test]
    fn test_assure_int128_positive_rejects_zero() {
        assert!(assure_int128_positive(&FigValue::Int128(0)).is_err());
    }

    // ── Float64 validators ────────────────────────────────────────────────────

    #[test]
    fn test_assure_float64_not_nan_rejects_nan() {
        assert!(assure_float64_not_nan(&FigValue::Float64(f64::NAN)).is_err());
    }

    #[test]
    fn test_assure_float64_in_range_accepts() {
        assert!(assure_float64_in_range(0.0, 1.0)(&FigValue::Float64(0.5)).is_ok());
    }

    #[test]
    fn test_assure_float64_in_range_rejects_outside() {
        assert!(assure_float64_in_range(0.0, 1.0)(&FigValue::Float64(1.1)).is_err());
    }

    // ── Float128 validators ───────────────────────────────────────────────────

    #[test]
    fn test_assure_float128_not_nan_rejects_nan() {
        assert!(assure_float128_not_nan(&FigValue::Float128(f64::NAN)).is_err());
    }

    #[test]
    fn test_assure_float128_positive_accepts() {
        assert!(assure_float128_positive(&FigValue::Float128(0.001)).is_ok());
    }

    // ── Duration validators ───────────────────────────────────────────────────

    #[test]
    fn test_assure_duration_positive_rejects_zero() {
        assert!(assure_duration_positive(&FigValue::Duration(Duration::ZERO)).is_err());
    }

    #[test]
    fn test_assure_duration_min_accepts_exact() {
        let five = Duration::from_secs(5);
        assert!(assure_duration_min(five)(&FigValue::Duration(five)).is_ok());
    }

    #[test]
    fn test_assure_duration_max_rejects_over() {
        let max  = Duration::from_secs(60);
        let over = Duration::from_secs(61);
        assert!(assure_duration_max(max)(&FigValue::Duration(over)).is_err());
    }

    #[test]
    fn test_assure_duration_greater_than_rejects_equal() {
        let d = Duration::from_secs(10);
        assert!(assure_duration_greater_than(d)(&FigValue::Duration(d)).is_err());
    }

    // ── List validators ───────────────────────────────────────────────────────

    #[test]
    fn test_assure_list_not_empty_rejects_empty_liststring() {
        assert!(assure_list_not_empty(&FigValue::ListString(vec![])).is_err());
    }

    #[test]
    fn test_assure_list_not_empty_accepts_listint() {
        assert!(assure_list_not_empty(&FigValue::ListInt(vec![1, 2])).is_ok());
    }

    #[test]
    fn test_assure_list_min_length_rejects_short() {
        assert!(assure_list_min_length(3)(&FigValue::ListString(vec!["a".into(), "b".into()])).is_err());
    }

    #[test]
    fn test_assure_list_contains_accepts() {
        assert!(assure_list_contains("b")(&FigValue::ListString(vec!["a".into(), "b".into()])).is_ok());
    }

    #[test]
    fn test_assure_list_not_contains_rejects() {
        assert!(assure_list_not_contains("a")(&FigValue::ListString(vec!["a".into()])).is_err());
    }

    #[test]
    fn test_assure_list_length_accepts_exact() {
        assert!(assure_list_length(2)(&FigValue::ListInt64(vec![1, 2])).is_ok());
    }

    #[test]
    fn test_assure_list_not_length_rejects_exact() {
        assert!(assure_list_not_length(2)(&FigValue::ListInt64(vec![1, 2])).is_err());
    }

    // ── Map validators ────────────────────────────────────────────────────────

    #[test]
    fn test_assure_map_not_empty_rejects_empty() {
        assert!(assure_map_not_empty(&FigValue::MapString(HashMap::new())).is_err());
    }

    #[test]
    fn test_assure_map_has_key_accepts() {
        let mut m = HashMap::new();
        m.insert("env".to_string(), "prod".to_string());
        assert!(assure_map_has_key("env")(&FigValue::MapString(m)).is_ok());
    }

    #[test]
    fn test_assure_map_has_key_rejects_missing() {
        assert!(assure_map_has_key("env")(&FigValue::MapString(HashMap::new())).is_err());
    }

    #[test]
    fn test_assure_map_has_keys_rejects_partial() {
        let mut m = HashMap::new();
        m.insert("env".to_string(), "prod".to_string());
        let v = assure_map_has_keys(vec!["env".into(), "version".into()]);
        assert!(v(&FigValue::MapString(m)).is_err());
    }

    #[test]
    fn test_assure_map_value_matches_accepts() {
        let mut m = HashMap::new();
        m.insert("env".to_string(), "prod".to_string());
        assert!(assure_map_value_matches("env", "prod")(&FigValue::MapString(m)).is_ok());
    }

    #[test]
    fn test_assure_map_value_matches_rejects_wrong_value() {
        let mut m = HashMap::new();
        m.insert("env".to_string(), "dev".to_string());
        assert!(assure_map_value_matches("env", "prod")(&FigValue::MapString(m)).is_err());
    }

    #[test]
    fn test_assure_map_length_accepts_exact() {
        let mut m = HashMap::new();
        m.insert("a".to_string(), "1".to_string());
        m.insert("b".to_string(), "2".to_string());
        assert!(assure_map_length(2)(&FigValue::MapString(m)).is_ok());
    }

    #[test]
    fn test_assure_map_not_length_rejects_exact() {
        let mut m = HashMap::new();
        m.insert("a".to_string(), 1i32);
        assert!(assure_map_not_length(1)(&FigValue::MapInt(m)).is_err());
    }

    // ── wrong type returns error ───────────────────────────────────────────────

    #[test]
    fn test_validators_reject_wrong_figvalue_variant() {
        assert!(assure_string_not_empty(&FigValue::Int(1)).is_err());
        assert!(assure_int_positive(&FigValue::Bool(true)).is_err());
        assert!(assure_bool_true(&FigValue::String("true".into())).is_err());
        assert!(assure_duration_positive(&FigValue::Float64(1.0)).is_err());
        assert!(assure_list_not_empty(&FigValue::MapString(HashMap::new())).is_err());
        assert!(assure_map_not_empty(&FigValue::ListString(vec![])).is_err());
    }
}
