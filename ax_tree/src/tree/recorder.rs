//! Recorder utility for capturing and tracking accessibility tree snapshots

use crate::tree::{AxNodeData, AxTreeDiff, AxTreeSnapshot};
use chrono::{DateTime, Utc};
use std::collections::{HashMap, VecDeque};
use std::fmt;

const EVENT_LOG_LIMIT: usize = 200;

/// Classifies the accessibility notification that caused us to capture a snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerKind {
    AppActivated,
    FocusedWindow,
    FocusedElement,
    TitleChanged,
    ValueChanged,
    WindowCreated,
    VisibleChildren,
    SelectedChildren,
    Other,
}

impl TriggerKind {
    pub fn from_notification(notification: &str) -> Self {
        match notification {
            "AXApplicationActivated" => Self::AppActivated,
            "AXFocusedWindowChanged" => Self::FocusedWindow,
            "AXFocusedUIElementChanged" => Self::FocusedElement,
            "AXTitleChanged" => Self::TitleChanged,
            "AXValueChanged" => Self::ValueChanged,
            "AXWindowCreated" => Self::WindowCreated,
            "AXVisibleChildrenChanged" => Self::VisibleChildren,
            "AXSelectedChildrenChanged" => Self::SelectedChildren,
            _ => Self::Other,
        }
    }

    pub fn promotes_focus(self) -> bool {
        matches!(
            self,
            Self::AppActivated | Self::FocusedWindow | Self::WindowCreated
        )
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::AppActivated => "activated",
            Self::FocusedWindow => "focused window",
            Self::FocusedElement => "focus moved",
            Self::TitleChanged => "title changed",
            Self::ValueChanged => "value changed",
            Self::WindowCreated => "window created",
            Self::VisibleChildren => "children changed",
            Self::SelectedChildren => "selection changed",
            Self::Other => "notification",
        }
    }
}

impl fmt::Display for TriggerKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.label())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct NodeDelta {
    pub added: usize,
    pub removed: usize,
    pub updated: usize,
}

impl NodeDelta {
    fn from_diff(diff: Option<&AxTreeDiff>) -> Self {
        if let Some(diff) = diff {
            Self {
                added: diff.added_nodes.len(),
                removed: diff.removed_nodes.len(),
                updated: diff.updated_nodes.len(),
            }
        } else {
            Self {
                added: 0,
                removed: 0,
                updated: 0,
            }
        }
    }

    pub fn total(self) -> usize {
        self.added + self.removed + self.updated
    }
}

#[derive(Debug, Clone)]
pub struct EventSummary {
    pub id: u64,
    pub timestamp: DateTime<Utc>,
    pub pid: i32,
    pub app_name: String,
    pub trigger: TriggerKind,
    pub headline: String,
    pub detail: Option<String>,
    pub focused_text: Option<String>,
    pub node_delta: NodeDelta,
}

impl EventSummary {
    fn new(
        id: u64,
        app_name: &str,
        snapshot: &AxTreeSnapshot,
        trigger: TriggerKind,
        diff: Option<&AxTreeDiff>,
    ) -> Self {
        let node_delta = NodeDelta::from_diff(diff);
        let headline = best_headline(snapshot).unwrap_or_else(|| "Content changed".to_owned());
        let detail = diff.map(|d| {
            format!(
                "Nodes +{} / -{} / ~{}",
                d.added_nodes.len(),
                d.removed_nodes.len(),
                d.updated_nodes.len()
            )
        });
        let focused_text = snapshot
            .focused_node()
            .and_then(|node| best_value(node).map(|text| truncate(&text, 120)));

        Self {
            id,
            timestamp: snapshot.timestamp,
            pid: snapshot.pid,
            app_name: app_name.to_owned(),
            trigger,
            headline,
            detail,
            focused_text,
            node_delta,
        }
    }
}

/// Maintains a history of accessibility snapshots grouped by PID and keeps track of the
/// snapshot that should currently be presented (generally the frontmost application).
#[derive(Debug)]
pub struct AxTreeRecorder {
    snapshots: HashMap<i32, AxTreeSnapshot>,
    current_pid: Option<i32>,
    history: VecDeque<AxTreeSnapshot>,
    max_history: usize,
    last_diff: Option<AxTreeDiff>,
    event_log: VecDeque<EventSummary>,
    next_event_id: u64,
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
            event_log: VecDeque::with_capacity(EVENT_LOG_LIMIT),
            next_event_id: 1,
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
        trigger: TriggerKind,
        app_name: &str,
        promote: bool,
        frontmost_pid: Option<i32>,
    ) -> Option<AxTreeDiff> {
        let pid = snapshot.pid;
        let node_count = snapshot.nodes.len();

        let prior_snapshot_for_pid = self.snapshots.get(&pid).cloned();
        let prior_active_snapshot = self.current().cloned();

        // Store the snapshot by PID so it can be reused for comparisons later.
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
            !is_helper_process || is_frontmost
        } else if promote && is_frontmost {
            true
        } else if promote && !is_helper_process {
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
                notification = %trigger,
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
            notification = %trigger,
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

        if self.should_emit_event(trigger, diff.as_ref(), node_count) {
            let summary = EventSummary::new(
                self.next_event_id,
                app_name,
                &snapshot,
                trigger,
                diff.as_ref(),
            );
            self.event_log.push_back(summary);
            self.next_event_id = self.next_event_id.wrapping_add(1);
            while self.event_log.len() > EVENT_LOG_LIMIT {
                self.event_log.pop_front();
            }
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

    /// Return event summaries whose id is greater than `last_seen`.
    pub fn events_since(&self, last_seen: Option<u64>) -> Vec<EventSummary> {
        self.event_log
            .iter()
            .filter(|event| last_seen.map_or(true, |id| event.id > id))
            .cloned()
            .collect()
    }

    /// ID of the most recent event logged, if any.
    pub fn last_event_id(&self) -> Option<u64> {
        self.event_log.back().map(|event| event.id)
    }

    /// Get all snapshots as a slice
    pub fn history(&self) -> &[AxTreeSnapshot] {
        let (head, tail) = self.history.as_slices();
        if tail.is_empty() {
            head
        } else {
            panic!("VecDeque is not contiguous");
        }
    }

    /// Clear history and event log
    pub fn clear(&mut self) {
        self.snapshots.clear();
        self.current_pid = None;
        self.history.clear();
        self.last_diff = None;
        self.event_log.clear();
        self.next_event_id = 1;
    }

    fn should_emit_event(
        &self,
        trigger: TriggerKind,
        diff: Option<&AxTreeDiff>,
        node_count: usize,
    ) -> bool {
        match trigger {
            TriggerKind::AppActivated
            | TriggerKind::FocusedWindow
            | TriggerKind::WindowCreated
            | TriggerKind::TitleChanged => true,
            TriggerKind::ValueChanged
            | TriggerKind::VisibleChildren
            | TriggerKind::FocusedElement => diff
                .map(|d| {
                    let change_score =
                        d.added_nodes.len() + d.removed_nodes.len() + d.updated_nodes.len();
                    change_score >= 12 || (change_score >= 6 && node_count > 10)
                })
                .unwrap_or(false),
            TriggerKind::SelectedChildren => false,
            TriggerKind::Other => diff.map_or(false, |d| d.has_changes()),
        }
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
            event_log: VecDeque::with_capacity(EVENT_LOG_LIMIT),
            next_event_id: 1,
        }
    }
}

fn best_headline(snapshot: &AxTreeSnapshot) -> Option<String> {
    snapshot
        .focused_node()
        .and_then(|node| best_text(node))
        .or_else(|| snapshot.root().and_then(|node| best_text(node)))
}

fn best_text(node: &AxNodeData) -> Option<String> {
    node.label
        .as_ref()
        .and_then(|text| non_empty(text))
        .or_else(|| node.value.as_ref().and_then(|text| non_empty(text)))
        .or_else(|| node.description.as_ref().and_then(|text| non_empty(text)))
        .map(|text| truncate(text, 140))
        .or_else(|| Some(node.role.clone()))
}

fn best_value(node: &AxNodeData) -> Option<String> {
    node.value
        .as_ref()
        .and_then(|text| non_empty(text))
        .map(|text| text.to_owned())
        .or_else(|| {
            node.description
                .as_ref()
                .and_then(|text| non_empty(text).map(|s| s.to_owned()))
        })
}

fn non_empty(text: &str) -> Option<&str> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn truncate(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_owned();
    }
    let mut truncated = String::with_capacity(max_chars + 1);
    for c in text.chars().take(max_chars) {
        truncated.push(c);
    }
    truncated.push('…');
    truncated
}
