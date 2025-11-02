//! Recorder utility for capturing and tracking accessibility tree snapshots

use crate::ax_tree::{AxTreeDiff, AxTreeSnapshot};
use std::collections::VecDeque;

/// Maintains a history of tree snapshots and computes diffs
pub struct AxTreeRecorder {
    current_snapshot: Option<AxTreeSnapshot>,
    history: VecDeque<AxTreeSnapshot>,
    max_history: usize,
}

impl AxTreeRecorder {
    /// Create a new recorder
    pub fn new(max_history: usize) -> Self {
        Self {
            current_snapshot: None,
            history: VecDeque::with_capacity(max_history),
            max_history,
        }
    }

    /// Capture a new snapshot and compute diff from previous
    pub fn capture(&mut self, snapshot: AxTreeSnapshot) -> Option<AxTreeDiff> {
        let diff = if let Some(ref old_snapshot) = self.current_snapshot {
            Some(crate::ax_tree::compute_diff(old_snapshot, &snapshot))
        } else {
            None
        };

        // Add to history
        self.history.push_back(snapshot.clone());
        if self.history.len() > self.max_history {
            self.history.pop_front();
        }

        // Update current
        self.current_snapshot = Some(snapshot);

        diff
    }

    /// Get the current snapshot
    pub fn current(&self) -> Option<&AxTreeSnapshot> {
        self.current_snapshot.as_ref()
    }

    /// Get snapshot at a specific index (0 = oldest in history, len-1 = current)
    pub fn at_index(&self, index: usize) -> Option<&AxTreeSnapshot> {
        if index < self.history.len() {
            self.history.get(index)
        } else {
            None
        }
    }

    /// Get the number of snapshots in history
    pub fn len(&self) -> usize {
        self.history.len()
    }

    /// Check if history is empty
    pub fn is_empty(&self) -> bool {
        self.history.is_empty()
    }

    /// Get all snapshots as a slice
    pub fn history(&self) -> &[AxTreeSnapshot] {
        let (head, tail) = self.history.as_slices();
        if tail.is_empty() {
            head
        } else {
            // This shouldn't happen often since we use a VecDeque,
            // but handle it just in case
            panic!("VecDeque is not contiguous");
        }
    }

    /// Clear history
    pub fn clear(&mut self) {
        self.current_snapshot = None;
        self.history.clear();
    }
}

impl Default for AxTreeRecorder {
    fn default() -> Self {
        Self::new(100)
    }
}

