//! Snapshot undo/redo.
//!
//! The editor commits a snapshot of the document whenever an edit is
//! finished (e.g. when a slider drag ends), so one drag is one undo step.
//! Snapshots are expected to be cheap to clone (large parts behind `Arc`).

/// Linear history of committed states with a cursor.
#[derive(Debug, Clone)]
pub struct History<T> {
    states: Vec<(T, String)>,
    cursor: usize,
    limit: usize,
}

impl<T: Clone + PartialEq> History<T> {
    /// Start with `initial` as the only state; keep at most `limit` steps.
    pub fn new(initial: T, limit: usize) -> Self {
        Self {
            states: vec![(initial, String::new())],
            cursor: 0,
            limit: limit.max(1),
        }
    }

    pub fn current(&self) -> &T {
        &self.states[self.cursor].0
    }

    /// Record `state` (labelled) as a new step, if it differs from the
    /// current one. Discards any redo steps. Returns whether it was recorded.
    pub fn commit(&mut self, state: T, label: impl Into<String>) -> bool {
        if *self.current() == state {
            return false;
        }
        self.states.truncate(self.cursor + 1);
        self.states.push((state, label.into()));
        if self.states.len() > self.limit + 1 {
            self.states.remove(0);
        }
        self.cursor = self.states.len() - 1;
        true
    }

    /// Replace history with a single state (e.g. after opening a file).
    pub fn reset(&mut self, state: T) {
        self.states = vec![(state, String::new())];
        self.cursor = 0;
    }

    pub fn can_undo(&self) -> bool {
        self.cursor > 0
    }

    pub fn can_redo(&self) -> bool {
        self.cursor + 1 < self.states.len()
    }

    /// Label of the step `undo` would revert.
    pub fn undo_label(&self) -> Option<&str> {
        self.can_undo().then(|| self.states[self.cursor].1.as_str())
    }

    pub fn redo_label(&self) -> Option<&str> {
        self.can_redo()
            .then(|| self.states[self.cursor + 1].1.as_str())
    }

    /// Step back; returns the state to restore and the label undone.
    pub fn undo(&mut self) -> Option<(T, String)> {
        if !self.can_undo() {
            return None;
        }
        let label = self.states[self.cursor].1.clone();
        self.cursor -= 1;
        Some((self.current().clone(), label))
    }

    pub fn redo(&mut self) -> Option<(T, String)> {
        if !self.can_redo() {
            return None;
        }
        self.cursor += 1;
        Some((self.current().clone(), self.states[self.cursor].1.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undo_redo_and_branching() {
        let mut h = History::new(0, 10);
        assert!(!h.commit(0, "same"));
        h.commit(1, "a");
        h.commit(2, "b");
        assert_eq!(h.undo_label(), Some("b"));
        assert_eq!(h.undo(), Some((1, "b".into())));
        assert_eq!(h.redo_label(), Some("b"));
        assert_eq!(h.undo(), Some((0, "a".into())));
        assert_eq!(h.undo(), None);
        assert_eq!(h.redo(), Some((1, "a".into())));
        h.commit(5, "c"); // drops the "b" redo step
        assert!(!h.can_redo());
        assert_eq!(*h.current(), 5);
    }

    #[test]
    fn limit_drops_oldest() {
        let mut h = History::new(0, 3);
        for i in 1..=10 {
            h.commit(i, format!("{i}"));
        }
        let mut n = 0;
        while h.undo().is_some() {
            n += 1;
        }
        assert_eq!(n, 3);
        assert_eq!(*h.current(), 7);
    }
}
