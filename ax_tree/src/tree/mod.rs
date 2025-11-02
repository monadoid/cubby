//! Accessibility tree abstraction for macOS AX API
//!
//! This module provides a clean node/tree interface similar to AccessKit,
//! abstracting away the low-level objc2 code for easier tree manipulation
//! and diff computation.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[cfg(target_os = "macos")]
mod ax_element;

#[cfg(target_os = "macos")]
pub use ax_element::{AxElement, BuildElementError};

pub mod recorder;

/// Unique identifier for a node in the accessibility tree
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AxNodeId(pub u64);

impl AxNodeId {
    /// Generate a stable ID from an AxElement
    #[cfg(target_os = "macos")]
    pub fn from_element(element: &AxElement) -> Self {
        // Use the element's id as the basis, but convert to u64
        let object_id = element
            .id()
            .and_then(|id| {
                // Try to parse as hex number (ax_...)
                if id.starts_with("ax_") {
                    u64::from_str_radix(&id[3..], 16).ok()
                } else {
                    id.parse::<u64>().ok()
                }
            })
            .unwrap_or_else(|| {
                // Fallback: hash the element's role and position
                use std::hash::{Hash, Hasher};
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                if let Ok(role) = element.role() {
                    role.hash(&mut hasher);
                }
                if let Some(label) = element.label() {
                    label.hash(&mut hasher);
                }
                if let Some(pid) = element.pid() {
                    pid.hash(&mut hasher);
                }
                hasher.finish()
            });
        AxNodeId(object_id)
    }
}

/// Bounds information for a node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AxBounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl AxBounds {
    #[cfg(target_os = "macos")]
    pub fn from_element(element: &AxElement) -> Option<Self> {
        element.bounds().ok().map(|(x, y, w, h)| AxBounds {
            x,
            y,
            width: w,
            height: h,
        })
    }
}

/// Data associated with a node in the accessibility tree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AxNodeData {
    pub id: AxNodeId,
    pub role: String,
    pub label: Option<String>,
    pub value: Option<String>,
    pub description: Option<String>,
    pub bounds: Option<AxBounds>,
    pub children: Vec<AxNodeId>,
    pub properties: HashMap<String, Option<serde_json::Value>>,
    pub is_focused: bool,
    pub pid: Option<i32>,
}

impl AxNodeData {
    /// Create node data from an AxElement (without children - children are populated during tree building)
    #[cfg(target_os = "macos")]
    pub fn from_element(element: &AxElement, pid: Option<i32>) -> Result<Self, BuildTreeError> {
        let id = AxNodeId::from_element(element);
        let role = element
            .role()
            .map_err(|e| BuildTreeError::GetAttributesFailed(format!("{:?}", e)))?;
        let label = element.label();
        let value = element.value();
        let description = element.description();
        let bounds = AxBounds::from_element(element);
        let is_focused = element.is_focused();
        let properties = element.properties();

        Ok(AxNodeData {
            id,
            role,
            label,
            value,
            description,
            bounds,
            children: Vec::new(), // Will be populated during tree building
            properties,
            is_focused,
            pid,
        })
    }
}

/// Complete snapshot of an accessibility tree at a point in time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AxTreeSnapshot {
    pub timestamp: DateTime<Utc>,
    pub root_id: AxNodeId,
    pub nodes: HashMap<AxNodeId, AxNodeData>,
    pub focused_node_id: Option<AxNodeId>,
    pub pid: i32,
    pub notification: String,
}

impl AxTreeSnapshot {
    /// Get the root node
    pub fn root(&self) -> Option<&AxNodeData> {
        self.nodes.get(&self.root_id)
    }

    /// Get a node by ID
    pub fn node(&self, id: AxNodeId) -> Option<&AxNodeData> {
        self.nodes.get(&id)
    }

    /// Get focused node
    pub fn focused_node(&self) -> Option<&AxNodeData> {
        self.focused_node_id.and_then(|id| self.node(id))
    }
}

/// Represents changes between two tree snapshots
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AxTreeDiff {
    pub timestamp: DateTime<Utc>,
    pub added_nodes: Vec<AxNodeId>,
    pub removed_nodes: Vec<AxNodeId>,
    pub updated_nodes: Vec<AxNodeId>,
    pub focused_changed: bool,
    pub old_focused_id: Option<AxNodeId>,
    pub new_focused_id: Option<AxNodeId>,
}

impl AxTreeDiff {
    /// Check if there are any changes
    pub fn has_changes(&self) -> bool {
        !self.added_nodes.is_empty()
            || !self.removed_nodes.is_empty()
            || !self.updated_nodes.is_empty()
            || self.focused_changed
    }

    /// Get summary string
    pub fn summary(&self) -> String {
        format!(
            "+{} -{} ~{} {}",
            self.added_nodes.len(),
            self.removed_nodes.len(),
            self.updated_nodes.len(),
            if self.focused_changed { "focus" } else { "" }
        )
        .trim()
        .to_string()
    }
}

/// Error types for tree building
#[derive(Debug, thiserror::Error)]
pub enum BuildTreeError {
    #[error("failed to get children: {0}")]
    GetChildrenFailed(String),
    #[error("failed to get attributes: {0}")]
    GetAttributesFailed(String),
}

#[cfg(target_os = "macos")]
impl From<BuildElementError> for BuildTreeError {
    fn from(err: BuildElementError) -> Self {
        BuildTreeError::GetAttributesFailed(format!("{:?}", err))
    }
}

/// Build a complete tree snapshot from a root AxElement
#[cfg(target_os = "macos")]
pub fn build_tree_snapshot(
    root_element: &AxElement,
    pid: i32,
    notification: &str,
) -> Result<AxTreeSnapshot, BuildTreeError> {
    use std::collections::HashSet;

    let timestamp = Utc::now();
    let root_id = AxNodeId::from_element(root_element);

    let mut nodes = HashMap::new();
    let mut focused_node_id = None;
    let mut visited = HashSet::new(); // Track visited nodes to prevent cycles

    // Recursively build the tree
    // Add a maximum depth limit to prevent stack overflow from very deep trees
    // Increased to 25 to capture full application trees (display settings can still filter)
    const MAX_DEPTH: usize = 25;

    fn build_node(
        element: &AxElement,
        pid: Option<i32>,
        nodes: &mut HashMap<AxNodeId, AxNodeData>,
        focused_node_id: &mut Option<AxNodeId>,
        visited: &mut HashSet<AxNodeId>,
        depth: usize,
    ) -> Result<AxNodeId, BuildTreeError> {
        // Safety: Prevent stack overflow from extremely deep trees
        if depth > MAX_DEPTH {
            tracing::warn!(
                "Node {:?} at depth {} exceeded max depth {}",
                element.id(),
                depth,
                MAX_DEPTH
            );
            // Return a minimal node
            let id = AxNodeId::from_element(element);
            if !nodes.contains_key(&id) {
                let node_data = AxNodeData::from_element(element, pid)?;
                nodes.insert(id, node_data.clone());
            }
            return Ok(id);
        }

        let id = AxNodeId::from_element(element);

        // Check for cycles - if we've already visited this node, skip it
        if visited.contains(&id) {
            tracing::debug!(
                "Cycle detected at depth {} for node {:?}, returning existing",
                depth,
                id
            );
            // If node already exists in nodes, return it
            if nodes.contains_key(&id) {
                return Ok(id);
            }
            // Otherwise, create a minimal entry to prevent infinite loops
            let node_data = AxNodeData::from_element(element, pid)?;
            nodes.insert(id, node_data);
            return Ok(id);
        }

        // Mark as visited before recursing
        visited.insert(id);

        if depth % 10 == 0 {
            tracing::debug!(
                "Building tree node at depth {}, visited {} nodes so far",
                depth,
                visited.len()
            );
        }

        // Get children first - limit the number to prevent stack overflow
        let actual_children = match element.children() {
            Ok(children) => {
                // Limit number of children processed to prevent deep recursion
                const MAX_CHILDREN_PER_NODE: usize = 50;
                if children.len() > MAX_CHILDREN_PER_NODE {
                    tracing::debug!(
                        "Node at depth {} has {} children, limiting to {}",
                        depth,
                        children.len(),
                        MAX_CHILDREN_PER_NODE
                    );
                    children.into_iter().take(MAX_CHILDREN_PER_NODE).collect()
                } else {
                    children
                }
            }
            Err(e) => {
                if depth == 0 {
                    let role = element.role().unwrap_or_else(|_| "unknown".to_string());
                    tracing::warn!(role = %role, error = ?e, "root element failed to get children");
                }
                // Return empty children if we can't get them
                Vec::new()
            }
        };

        // Recursively build children and collect their IDs
        let mut child_ids = Vec::new();
        for (child_idx, child) in actual_children.iter().enumerate() {
            if depth < 5 || child_idx % 10 == 0 {
                tracing::debug!(
                    "Processing child {} of {} at depth {}",
                    child_idx + 1,
                    actual_children.len(),
                    depth
                );
            }

            let child_id = AxNodeId::from_element(child);

            // Check if we've already visited this child to avoid cycles
            if visited.contains(&child_id) {
                tracing::debug!(
                    "Skipping already visited child {} at depth {}",
                    child_idx,
                    depth
                );
                // Add the existing node ID if it's already in the graph
                if nodes.contains_key(&child_id) {
                    if !child_ids.contains(&child_id) {
                        child_ids.push(child_id);
                    }
                }
                continue;
            }

            match build_node(child, pid, nodes, focused_node_id, visited, depth + 1) {
                Ok(id) => {
                    // Only add child if it's not already in child_ids (avoid duplicates from cycles)
                    if !child_ids.contains(&id) {
                        child_ids.push(id);
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to build child {} at depth {}: {:?}",
                        child_idx,
                        depth,
                        e
                    );
                    // Continue with other children even if one fails
                }
            }
        }

        // Now build the node data with actual child IDs
        let mut node_data = AxNodeData::from_element(element, pid)?;

        // Track focused node
        if node_data.is_focused {
            *focused_node_id = Some(id);
        }

        // Set children
        node_data.children = child_ids;

        nodes.insert(id, node_data);
        Ok(id)
    }

    build_node(
        root_element,
        Some(pid),
        &mut nodes,
        &mut focused_node_id,
        &mut visited,
        0,
    )?;

    Ok(AxTreeSnapshot {
        timestamp,
        root_id,
        nodes,
        focused_node_id,
        pid,
        notification: notification.to_string(),
    })
}

/// Compute the difference between two snapshots
pub fn compute_diff(old: &AxTreeSnapshot, new: &AxTreeSnapshot) -> AxTreeDiff {
    let mut added_nodes = Vec::new();
    let mut removed_nodes = Vec::new();
    let mut updated_nodes = Vec::new();

    // Find added and updated nodes
    for (id, new_node) in &new.nodes {
        match old.nodes.get(id) {
            None => added_nodes.push(*id),
            Some(old_node) => {
                // Simple comparison: if role, label, value, or children changed, it's updated
                if old_node.role != new_node.role
                    || old_node.label != new_node.label
                    || old_node.value != new_node.value
                    || old_node.children != new_node.children
                    || old_node.is_focused != new_node.is_focused
                {
                    updated_nodes.push(*id);
                }
            }
        }
    }

    // Find removed nodes
    for id in old.nodes.keys() {
        if !new.nodes.contains_key(id) {
            removed_nodes.push(*id);
        }
    }

    // Check if focus changed
    let focused_changed = old.focused_node_id != new.focused_node_id;

    AxTreeDiff {
        timestamp: new.timestamp,
        added_nodes,
        removed_nodes,
        updated_nodes,
        focused_changed,
        old_focused_id: old.focused_node_id,
        new_focused_id: new.focused_node_id,
    }
}
