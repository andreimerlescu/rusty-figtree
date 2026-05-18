use crate::error::FigtreeResult;
use crate::fig::FigValue;

// ── CallbackPhase ─────────────────────────────────────────────────────────────

/// Identifies at which point in a Fig's lifecycle a callback fires.
///
/// Multiple callbacks of different phases can be registered on the
/// same key. Callbacks of the same phase fire in registration order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallbackPhase {
    /// Fires after validation passes during parse() or load().
    /// The value has been resolved and validated but the Tree has
    /// not yet made it available to consumers.
    ///
    /// If this callback returns an error, parse() or load() fails.
    /// Use for side effects that must succeed before the application
    /// starts — connecting to a database, verifying a file exists, etc.
    AfterVerify,

    /// Fires every time the getter for this key is called.
    /// The value is the current resolved value at the time of the read.
    ///
    /// If this callback returns an error the getter propagates it.
    /// Use sparingly — this fires on every read and can affect
    /// performance if the callback does significant work.
    AfterRead,

    /// Fires after a successful store() changes this key's value.
    /// Receives the new value. Does not fire for unchanged sets,
    /// validation rejections, or rule blocks.
    ///
    /// If this callback returns an error the store() propagates it
    /// and the value change is rolled back.
    AfterChange,
}

impl std::fmt::Display for CallbackPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CallbackPhase::AfterVerify => write!(f, "AfterVerify"),
            CallbackPhase::AfterRead   => write!(f, "AfterRead"),
            CallbackPhase::AfterChange => write!(f, "AfterChange"),
        }
    }
}

// ── CallbackFn ────────────────────────────────────────────────────────────────

/// The function signature all callbacks must satisfy.
///
/// Receives a reference to the current FigValue. Returns Ok(()) to
/// allow the operation to proceed, or Err(FigtreeError) to halt it.
///
/// Stored as a boxed trait object so callbacks of any concrete type
/// can live together in a Vec without the container being generic.
/// The Send + Sync bounds are required because the Tree may be used
/// across threads.
pub type CallbackFn = Box<dyn Fn(&FigValue) -> FigtreeResult<()> + Send + Sync>;

// ── Callback ──────────────────────────────────────────────────────────────────

/// A single registered callback — a phase and the function to call.
///
/// Callbacks are stored inside Fig and are never exposed directly
/// to consumers. The Tree invokes them at the appropriate phase.
pub struct Callback {
    /// Which lifecycle phase triggers this callback.
    pub phase: CallbackPhase,

    /// The function to invoke.
    pub func: CallbackFn,
}

impl Callback {
    /// Creates a new Callback from a phase and any callable that
    /// satisfies the CallbackFn signature.
    pub fn new<F>(phase: CallbackPhase, func: F) -> Self
    where
        F: Fn(&FigValue) -> FigtreeResult<()> + Send + Sync + 'static,
    {
        Callback {
            phase,
            func: Box::new(func),
        }
    }

    /// Invokes the callback with the given value.
    /// Returns the callback's result directly.
    pub fn invoke(&self, value: &FigValue) -> FigtreeResult<()> {
        (self.func)(value)
    }
}

impl std::fmt::Debug for Callback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // CallbackFn is not Debug — we show the phase only
        f.debug_struct("Callback")
            .field("phase", &self.phase)
            .field("func",  &"<closure>")
            .finish()
    }
}

// ── CallbackRegistry ──────────────────────────────────────────────────────────

/// An ordered collection of callbacks for a single Fig.
///
/// Callbacks fire in registration order within each phase.
/// The registry is stored inside Fig and manipulated by the Tree
/// via register() and invoke_phase().
#[derive(Debug, Default)]
pub struct CallbackRegistry {
    callbacks: Vec<Callback>,
}

impl CallbackRegistry {
    /// Creates an empty registry.
    pub fn new() -> Self {
        CallbackRegistry {
            callbacks: Vec::new(),
        }
    }

    /// Registers a callback for the given phase.
    /// Multiple callbacks per phase are allowed and fire in
    /// registration order.
    pub fn register<F>(&mut self, phase: CallbackPhase, func: F)
    where
        F: Fn(&FigValue) -> FigtreeResult<()> + Send + Sync + 'static,
    {
        self.callbacks.push(Callback::new(phase, func));
    }

    /// Invokes all callbacks registered for the given phase, in
    /// registration order, passing each the current value.
    ///
    /// Stops and returns the first error encountered. Callbacks
    /// registered after the failing one are not invoked.
    pub fn invoke_phase(
        &self,
        phase: &CallbackPhase,
        value: &FigValue,
    ) -> FigtreeResult<()> {
        for callback in self.callbacks.iter().filter(|c| &c.phase == phase) {
            callback.invoke(value)?;
        }
        Ok(())
    }

    /// Returns the number of callbacks registered across all phases.
    pub fn len(&self) -> usize {
        self.callbacks.len()
    }

    /// Returns true if no callbacks are registered.
    pub fn is_empty(&self) -> bool {
        self.callbacks.is_empty()
    }

    /// Returns the number of callbacks registered for a specific phase.
    pub fn count_for_phase(&self, phase: &CallbackPhase) -> usize {
        self.callbacks.iter().filter(|c| &c.phase == phase).count()
    }

    /// Returns true if any callback is registered for the given phase.
    pub fn has_phase(&self, phase: &CallbackPhase) -> bool {
        self.callbacks.iter().any(|c| &c.phase == phase)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::FigtreeError;
    use crate::fig::FigValue;
    use std::sync::{Arc, Mutex};

    // helper — a registry with a counter callback on the given phase
    fn counting_registry(phase: CallbackPhase) -> (CallbackRegistry, Arc<Mutex<usize>>) {
        let mut registry = CallbackRegistry::new();
        let counter      = Arc::new(Mutex::new(0usize));
        let counter_ref  = Arc::clone(&counter);
        registry.register(phase, move |_value| {
            *counter_ref.lock().unwrap() += 1;
            Ok(())
        });
        (registry, counter)
    }

    // ── registration ─────────────────────────────────────────────────────────

    #[test]
    fn test_empty_registry_has_zero_callbacks() {
        let registry = CallbackRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_register_increments_len() {
        let mut registry = CallbackRegistry::new();
        registry.register(CallbackPhase::AfterVerify, |_| Ok(()));
        registry.register(CallbackPhase::AfterChange, |_| Ok(()));
        assert_eq!(registry.len(), 2);
    }

    #[test]
    fn test_count_for_phase_is_accurate() {
        let mut registry = CallbackRegistry::new();
        registry.register(CallbackPhase::AfterVerify, |_| Ok(()));
        registry.register(CallbackPhase::AfterVerify, |_| Ok(()));
        registry.register(CallbackPhase::AfterChange, |_| Ok(()));
        assert_eq!(registry.count_for_phase(&CallbackPhase::AfterVerify), 2);
        assert_eq!(registry.count_for_phase(&CallbackPhase::AfterChange), 1);
        assert_eq!(registry.count_for_phase(&CallbackPhase::AfterRead),   0);
    }

    #[test]
    fn test_has_phase_returns_correct_bool() {
        let mut registry = CallbackRegistry::new();
        registry.register(CallbackPhase::AfterVerify, |_| Ok(()));
        assert!(registry.has_phase(&CallbackPhase::AfterVerify));
        assert!(!registry.has_phase(&CallbackPhase::AfterRead));
        assert!(!registry.has_phase(&CallbackPhase::AfterChange));
    }

    // ── invocation ────────────────────────────────────────────────────────────

    #[test]
    fn test_invoke_phase_fires_correct_callbacks() {
        let (registry, counter) = counting_registry(CallbackPhase::AfterVerify);
        registry
            .invoke_phase(&CallbackPhase::AfterVerify, &FigValue::Int(10))
            .unwrap();
        assert_eq!(*counter.lock().unwrap(), 1);
    }

    #[test]
    fn test_invoke_phase_does_not_fire_other_phases() {
        let (registry, counter) = counting_registry(CallbackPhase::AfterVerify);
        registry
            .invoke_phase(&CallbackPhase::AfterChange, &FigValue::Int(10))
            .unwrap();
        // AfterVerify callback should not have fired
        assert_eq!(*counter.lock().unwrap(), 0);
    }

    #[test]
    fn test_invoke_phase_fires_multiple_callbacks_in_order() {
        let mut registry = CallbackRegistry::new();
        let order        = Arc::new(Mutex::new(Vec::<usize>::new()));

        let order_ref = Arc::clone(&order);
        registry.register(CallbackPhase::AfterChange, move |_| {
            order_ref.lock().unwrap().push(1);
            Ok(())
        });

        let order_ref = Arc::clone(&order);
        registry.register(CallbackPhase::AfterChange, move |_| {
            order_ref.lock().unwrap().push(2);
            Ok(())
        });

        let order_ref = Arc::clone(&order);
        registry.register(CallbackPhase::AfterChange, move |_| {
            order_ref.lock().unwrap().push(3);
            Ok(())
        });

        registry
            .invoke_phase(&CallbackPhase::AfterChange, &FigValue::Int(1))
            .unwrap();

        assert_eq!(*order.lock().unwrap(), vec![1, 2, 3]);
    }

    #[test]
    fn test_invoke_phase_stops_on_first_error() {
        let mut registry = CallbackRegistry::new();
        let second_fired = Arc::new(Mutex::new(false));

        registry.register(CallbackPhase::AfterVerify, |_| {
            Err(FigtreeError::Other("first callback failed".into()))
        });

        let second_fired_ref = Arc::clone(&second_fired);
        registry.register(CallbackPhase::AfterVerify, move |_| {
            *second_fired_ref.lock().unwrap() = true;
            Ok(())
        });

        let result = registry.invoke_phase(
            &CallbackPhase::AfterVerify,
            &FigValue::Int(10),
        );

        assert!(result.is_err());
        assert!(!*second_fired.lock().unwrap());
    }

    #[test]
    fn test_invoke_phase_passes_value_to_callback() {
        let mut registry   = CallbackRegistry::new();
        let received_value = Arc::new(Mutex::new(None::<FigValue>));
        let received_ref   = Arc::clone(&received_value);

        registry.register(CallbackPhase::AfterRead, move |value| {
            *received_ref.lock().unwrap() = Some(value.clone());
            Ok(())
        });

        registry
            .invoke_phase(&CallbackPhase::AfterRead, &FigValue::Int(42))
            .unwrap();

        assert_eq!(
            *received_value.lock().unwrap(),
            Some(FigValue::Int(42))
        );
    }

    // ── empty phase invocation ────────────────────────────────────────────────

    #[test]
    fn test_invoke_phase_with_no_registered_callbacks_is_ok() {
        let registry = CallbackRegistry::new();
        let result   = registry.invoke_phase(
            &CallbackPhase::AfterChange,
            &FigValue::Bool(true),
        );
        assert!(result.is_ok());
    }

    // ── callback phase display ────────────────────────────────────────────────

    #[test]
    fn test_callback_phase_display() {
        assert_eq!(CallbackPhase::AfterVerify.to_string(), "AfterVerify");
        assert_eq!(CallbackPhase::AfterRead.to_string(),   "AfterRead");
        assert_eq!(CallbackPhase::AfterChange.to_string(), "AfterChange");
    }

    // ── cross-thread safety ───────────────────────────────────────────────────

    #[test]
    fn test_callback_registry_is_send_across_threads() {
        let mut registry = CallbackRegistry::new();
        let counter      = Arc::new(Mutex::new(0usize));
        let counter_ref  = Arc::clone(&counter);

        registry.register(CallbackPhase::AfterChange, move |_| {
            *counter_ref.lock().unwrap() += 1;
            Ok(())
        });

        // move registry into a thread — proves Send bound is satisfied
        let handle = std::thread::spawn(move || {
            registry
                .invoke_phase(&CallbackPhase::AfterChange, &FigValue::Int(1))
                .unwrap();
        });

        handle.join().unwrap();
        assert_eq!(*counter.lock().unwrap(), 1);
    }

    // ── typed value matching inside callbacks ─────────────────────────────────

    #[test]
    fn test_callback_can_pattern_match_figvalue() {
        let mut registry  = CallbackRegistry::new();
        let matched_int   = Arc::new(Mutex::new(false));
        let matched_ref   = Arc::clone(&matched_int);

        registry.register(CallbackPhase::AfterChange, move |value| {
            match value {
                FigValue::Int(n) if *n == 99 => {
                    *matched_ref.lock().unwrap() = true;
                    Ok(())
                }
                _ => Ok(()),
            }
        });

        registry
            .invoke_phase(&CallbackPhase::AfterChange, &FigValue::Int(99))
            .unwrap();

        assert!(*matched_int.lock().unwrap());
    }
}
