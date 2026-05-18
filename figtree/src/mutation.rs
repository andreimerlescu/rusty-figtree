use std::time::SystemTime;

use crate::fig::{FigSource, FigValue};

// ── Mutation ──────────────────────────────────────────────────────────────────

/// A Mutation represents a single observed change to a Fig's value.
///
/// Mutations are emitted by the Tree into the MutationReceiver channel
/// whenever a Fig's value changes and tracking is enabled. They are
/// distinct from history entries — history lives inside the Fig and
/// records everything including rejections. Mutations are outbound
/// notifications intended for consumers who want to react to changes.
///
/// A Mutation is only emitted when a change actually occurs —
/// unchanged sets, validation rejections, and rule blocks do not
/// produce Mutations.
#[derive(Debug, Clone)]
pub struct Mutation {
    pub key:    String,
    pub old:    Option<FigValue>,
    pub new:    FigValue,
    pub source: FigSource,
    pub when:   SystemTime,
}

impl Mutation {
    /// Creates a Mutation for a first resolution — old value is None.
    pub fn first(key: impl Into<String>, new: FigValue, source: FigSource) -> Self {
        Mutation {
            key:  key.into(),
            old:  None,
            new,
            source,
            when: SystemTime::now(),
        }
    }

    /// Creates a Mutation for a subsequent change — old value is known.
    pub fn changed(
        key:    impl Into<String>,
        old:    FigValue,
        new:    FigValue,
        source: FigSource,
    ) -> Self {
        Mutation {
            key: key.into(),
            old: Some(old),
            new,
            source,
            when: SystemTime::now(),
        }
    }

    /// Returns true if this Mutation represents a first resolution.
    pub fn is_first_resolution(&self) -> bool {
        self.old.is_none()
    }
}

impl std::fmt::Display for Mutation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let when = self.when
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default();
        match &self.old {
            None => write!(
                f,
                "[{}] first set to '{}' via {} (at {}s)",
                self.key,
                self.new,
                self.source,
                when.as_secs()
            ),
            Some(old) => write!(
                f,
                "[{}] changed from '{}' to '{}' via {} (at {}s)",
                self.key,
                old,
                self.new,
                self.source,
                when.as_secs()
            ),
        }
    }
}

// ── MutationSender / MutationReceiver ─────────────────────────────────────────

pub struct MutationSender {
    inner: std::sync::mpsc::Sender<Mutation>,
}

impl MutationSender {
    pub fn send(&self, mutation: Mutation) {
        let _ = self.inner.send(mutation);
    }
}

pub struct MutationReceiver {
    inner: std::sync::mpsc::Receiver<Mutation>,
}

impl MutationReceiver {
    pub fn recv(&self) -> Option<Mutation> {
        self.inner.recv().ok()
    }

    pub fn try_recv(&self) -> Option<Mutation> {
        self.inner.try_recv().ok()
    }

    pub fn iter(&self) -> impl Iterator<Item = Mutation> + '_ {
        self.inner.iter()
    }
}

pub fn mutation_channel() -> (MutationSender, MutationReceiver) {
    let (tx, rx) = std::sync::mpsc::channel();
    (
        MutationSender   { inner: tx },
        MutationReceiver { inner: rx },
    )
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fig::FigValue;

    #[test]
    fn test_mutation_first_has_no_old_value() {
        let m = Mutation::first(
            "workers",
            FigValue::Int(10),
            FigSource::Default,
        );
        assert!(m.old.is_none());
        assert!(m.is_first_resolution());
        assert_eq!(m.key, "workers");
        assert_eq!(m.new, FigValue::Int(10));
    }

    #[test]
    fn test_mutation_changed_carries_both_values() {
        let m = Mutation::changed(
            "workers",
            FigValue::Int(10),
            FigValue::Int(20),
            FigSource::Environment("WORKERS".into()),
        );
        assert_eq!(m.old, Some(FigValue::Int(10)));
        assert_eq!(m.new, FigValue::Int(20));
        assert!(!m.is_first_resolution());
    }

    #[test]
    fn test_mutation_channel_sends_and_receives() {
        let (tx, rx) = mutation_channel();
        tx.send(Mutation::first(
            "endpoint",
            FigValue::String("http://localhost".into()),
            FigSource::Default,
        ));
        let received = rx.recv().unwrap();
        assert_eq!(received.key, "endpoint");
        assert_eq!(received.new, FigValue::String("http://localhost".into()));
    }

    #[test]
    fn test_mutation_channel_try_recv_returns_none_when_empty() {
        let (_tx, rx) = mutation_channel();
        assert!(rx.try_recv().is_none());
    }

    #[test]
    fn test_mutation_channel_returns_none_when_sender_dropped() {
        let (tx, rx) = mutation_channel();
        drop(tx);
        assert!(rx.recv().is_none());
    }

    #[test]
    fn test_mutation_display_first_resolution() {
        let m = Mutation::first(
            "workers",
            FigValue::Int(10),
            FigSource::Default,
        );
        let s = m.to_string();
        assert!(s.contains("workers"));
        assert!(s.contains("first set"));
        assert!(s.contains("10"));
    }

    #[test]
    fn test_mutation_display_changed() {
        let m = Mutation::changed(
            "workers",
            FigValue::Int(10),
            FigValue::Int(20),
            FigSource::Flag("workers".into()),
        );
        let s = m.to_string();
        assert!(s.contains("changed"));
        assert!(s.contains("10"));
        assert!(s.contains("20"));
        assert!(s.contains("flag(workers)"));
    }

    #[test]
    fn test_mutation_iter_yields_all_sent() {
        let (tx, rx) = mutation_channel();
        tx.send(Mutation::first("a", FigValue::Bool(true),  FigSource::Default));
        tx.send(Mutation::first("b", FigValue::Bool(false), FigSource::Default));
        drop(tx);
        let collected: Vec<_> = rx.iter().collect();
        assert_eq!(collected.len(), 2);
        assert_eq!(collected[0].key, "a");
        assert_eq!(collected[1].key, "b");
    }
}
