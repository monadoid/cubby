//! Recorder utility for capturing and tracking accessibility tree snapshots

use crate::tree::{AxTreeDiff, AxTreeSnapshot};
use std::collections::{HashMap, VecDeque};

/// Maintains a history of accessibility snapshots grouped by PID and keeps track of the
/// snapshot that should currently be presented (generally the frontmost application).
#[derive(Debug)]
pub struct AxTreeRecorder {
    snapshots: HashMap<i32, AxTreeSnapshot>,
    current_pid: Option<i32>,
    history: VecDeque<AxTreeSnapshot>,
    max_history: usize,
    last_diff: Option<AxTreeDiff>,
}

impl AxTreeRecorder {
    /// Create a new recorder
    pub fn new(max_history: usize) -> Self {
        Self {
            snapshots: HashMap::new(),
            current_pid: None,
            history: VecDeque::with_capacity(max_history),
            max_history,
            last_diff: None,
        }
    }

    /// Capture a new snapshot and, when appropriate, promote it to be the active snapshot.
    ///
    /// `promote` should be true when the notification indicates that the originating process is
    /// now the frontmost application (e.g. focused window changed / application activated).
    /// `frontmost_pid` is the PID of the actual frontmost application (from NSWorkspace).
    pub fn capture(
        &mut self,
        snapshot: AxTreeSnapshot,
        promote: bool,
        frontmost_pid: Option<i32>,
    ) -> Option<AxTreeDiff> {
        let pid = snapshot.pid;
        let node_count = snapshot.nodes.len();

        let prior_snapshot_for_pid = self.snapshots.get(&pid).cloned();
        let prior_active_snapshot = self.current().cloned();

        // Store the snapshot for this PID so it can be referenced later even if it doesn't become
        // the active snapshot immediately.
        self.snapshots.insert(pid, snapshot.clone());

        let is_current_pid = self.current_pid == Some(pid);
        let is_frontmost = frontmost_pid == Some(pid);

        // Determine if we should promote this snapshot:
        // 1. If it's already the current PID (updates to current app)
        // 2. If we have no current PID yet (initial snapshot)
        // 3. If promote is true AND it's the frontmost app
        // 4. BUT: Don't promote helper processes (very few nodes) unless they're already current
        let is_helper_process = node_count <= 3;
        let should_promote = if is_current_pid {
            true
        } else if self.current_pid.is_none() {
            // Initial snapshot - only accept if it has reasonable nodes or is frontmost
            !is_helper_process || is_frontmost
        } else if promote && is_frontmost {
            // Frontmost app with promotion request - always accept
            true
        } else if promote && !is_helper_process {
            // Promotion request with reasonable nodes, but not frontmost - accept if better than current
            let current_node_count = prior_active_snapshot
                .as_ref()
                .map(|s| s.nodes.len())
                .unwrap_or(0);
            node_count > current_node_count
        } else {
            false
        };

        if !should_promote {
            tracing::debug!(
                pid,
                node_count,
                is_current = is_current_pid,
                promote,
                is_frontmost,
                is_helper = is_helper_process,
                "not promoting pid"
            );
            return None;
        }

        tracing::debug!(
            pid,
            node_count,
            is_current = is_current_pid,
            promote,
            is_frontmost,
            "promoting pid to current"
        );

        let baseline = if is_current_pid {
            prior_snapshot_for_pid
        } else {
            prior_active_snapshot
        };

        let diff = baseline
            .as_ref()
            .map(|old_snapshot| crate::tree::compute_diff(old_snapshot, &snapshot));

        self.last_diff = diff.clone();
        self.current_pid = Some(pid);

        self.history.push_back(snapshot.clone());
        if self.history.len() > self.max_history {
            self.history.pop_front();
        }

        diff
    }

    /// Get the current snapshot
    pub fn current(&self) -> Option<&AxTreeSnapshot> {
        self.current_pid.and_then(|pid| self.snapshots.get(&pid))
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

    /// Get the last computed diff
    pub fn last_diff(&self) -> Option<&AxTreeDiff> {
        self.last_diff.as_ref()
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
        self.snapshots.clear();
        self.current_pid = None;
        self.history.clear();
        self.last_diff = None;
    }
}

impl Default for AxTreeRecorder {
    fn default() -> Self {
        Self {
            snapshots: HashMap::new(),
            current_pid: None,
            history: VecDeque::new(),
            max_history: 100,
            last_diff: None,
        }
    }
}
