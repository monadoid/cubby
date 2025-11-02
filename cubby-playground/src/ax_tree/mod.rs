//! Accessibility tree abstraction for macOS AX API
//!
//! This module provides a clean node/tree interface similar to AccessKit,
//! abstracting away the low-level objc2 code for easier tree manipulation
//! and diff computation.

use chrono::{DateTime, Utc};
use cubby_core::operator::UIElement;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod recorder;

/// Unique identifier for a node in the accessibility tree
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AxNodeId(pub u64);

impl AxNodeId {
    /// Generate a stable ID from a UIElement
    pub fn from_element(element: &UIElement) -> Self {
        // Use the element's object_id as the basis, but convert to u64
        let object_id = element
            .id()
            .and_then(|id| id.parse::<u64>().ok())
            .unwrap_or_else(|| {
                // Fallback: hash the element's role and position
                use std::hash::{Hash, Hasher};
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                element.role().hash(&mut hasher);
                let attrs = element.attributes();
                attrs.label.hash(&mut hasher);
                if let Some(pid) = attrs.properties.get("AXProcessIdentifier") {
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
    pub fn from_element(element: &UIElement) -> Option<Self> {
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
    /// Create node data from a UIElement (without children - children are populated during tree building)
    pub fn from_element(element: &UIElement, pid: Option<i32>) -> Result<Self, BuildTreeError> {
        let attrs = element.attributes();
        let id = AxNodeId::from_element(element);
        
        // Note: children are empty here - they will be populated during tree building
        // This avoids circular dependencies during tree construction

        // Check if focused
        let is_focused = element.is_focused().unwrap_or(false);

        Ok(AxNodeData {
            id,
            role: attrs.role.clone(),
            label: attrs.label.clone(),
            value: attrs.value.clone(),
            description: attrs.description.clone(),
            bounds: AxBounds::from_element(element),
            children: Vec::new(), // Will be populated during tree building
            properties: attrs.properties.clone(),
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
            if self.focused_changed {
                "focus"
            } else {
                ""
            }
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

/// Build a complete tree snapshot from a root UIElement
pub fn build_tree_snapshot(
    root_element: &UIElement,
    pid: i32,
    notification: &str,
) -> Result<AxTreeSnapshot, BuildTreeError> {
    let timestamp = Utc::now();
    let root_id = AxNodeId::from_element(root_element);
    
    let mut nodes = HashMap::new();
    let mut focused_node_id = None;

    // Recursively build the tree
    fn build_node(
        element: &UIElement,
        pid: Option<i32>,
        nodes: &mut HashMap<AxNodeId, AxNodeData>,
        focused_node_id: &mut Option<AxNodeId>,
    ) -> Result<AxNodeId, BuildTreeError> {
        let id = AxNodeId::from_element(element);
        
        // Get children first
        let actual_children = element.children()
            .map_err(|e| BuildTreeError::GetChildrenFailed(format!("{:?}", e)))?;
        
        // Recursively build children and collect their IDs
        let mut child_ids = Vec::new();
        for child in actual_children {
            let child_id = build_node(&child, pid, nodes, focused_node_id)?;
            child_ids.push(child_id);
        }

        // Now build the node data with actual child IDs
        let node_data = AxNodeData::from_element(element, pid)?;
        
        // Track focused node
        if node_data.is_focused {
            *focused_node_id = Some(id);
        }

        // Create final node data with actual child IDs
        let final_node_data = AxNodeData {
            id,
            children: child_ids,
            ..node_data
        };
        
        nodes.insert(id, final_node_data);
        Ok(id)
    }

    build_node(root_element, Some(pid), &mut nodes, &mut focused_node_id)?;

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

