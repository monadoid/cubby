use eframe::egui::{self, RichText, Vec2};
use egui_graphs::{
    Graph as GraphWidget, GraphView, Metadata, SettingsInteraction, SettingsNavigation,
    SettingsStyle,
};
use petgraph::stable_graph::StableGraph;
#[cfg(target_os = "macos")]
use std::collections::BTreeMap;
use std::collections::{HashMap, HashSet, VecDeque};
#[cfg(target_os = "macos")]
use std::fmt::Write as FmtWrite;
use std::fs::File;
use std::io::{self, Write};
#[cfg(target_os = "macos")]
use std::path::PathBuf;

use crate::tree::{
    recorder::{AxTreeRecorder, EventSummary},
    AxNodeData as TreeAxNodeData, AxNodeId, AxTreeDiff, AxTreeSnapshot,
};

#[cfg(target_os = "macos")]
use crate::{start_observer_background, ObserverShared};
#[cfg(target_os = "macos")]
use std::sync::Arc;

use chrono::{DateTime, Utc};

const SIDE_PANEL_WIDTH: f32 = 300.0;
const LIST_MODE_CHAR_LIMIT: usize = 3_000;
#[cfg(target_os = "macos")]
const EVENT_FEED_LIMIT: usize = 200;

#[derive(serde::Deserialize, serde::Serialize, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    Graph,
    List,
}

impl Default for AppMode {
    fn default() -> Self {
        AppMode::Graph
    }
}

#[derive(Clone)]
pub struct TextEntry {
    pub timestamp: DateTime<Utc>,
    pub window_id: String,
    pub texts: Vec<String>,
}

#[derive(serde::Deserialize, serde::Serialize, Clone)]
#[serde(default)]
pub struct DisplaySettings {
    pub zoom: f32,
    pub pan_x: f32,
    pub pan_y: f32,
    pub node_radius_normal: f32,
    pub node_radius_highlight: f32,
    pub edge_width_normal: f32,
    pub edge_width_highlight: f32,
    pub max_depth: usize,
    pub label_truncate_len: usize,
    pub horizontal_spacing: f32,
    pub vertical_spacing: f32,
}

impl Default for DisplaySettings {
    fn default() -> Self {
        Self {
            zoom: 1.6,
            pan_x: 80.0,
            pan_y: 80.0,
            node_radius_normal: 8.0,
            node_radius_highlight: 11.0,
            edge_width_normal: 2.5,
            edge_width_highlight: 4.0,
            max_depth: 5,
            label_truncate_len: 24,
            horizontal_spacing: 260.0,
            vertical_spacing: 90.0,
        }
    }
}

type AxGraph = GraphWidget<TreeAxNodeData, ()>;

fn new_graph() -> AxGraph {
    GraphWidget::new(StableGraph::default())
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct AxTreeApp {
    #[serde(skip)]
    graph: AxGraph,
    #[serde(skip)]
    graph_interaction: SettingsInteraction,
    #[serde(skip)]
    graph_navigation: SettingsNavigation,
    #[serde(skip)]
    graph_style: SettingsStyle,
    #[serde(skip)]
    graph_needs_reset: bool,
    #[serde(skip)]
    #[cfg(target_os = "macos")]
    observer_shared: Option<Arc<ObserverShared>>,
    #[serde(skip)]
    last_diff: Option<AxTreeDiff>,
    #[serde(skip)]
    snapshot_counter: usize,
    display_settings: DisplaySettings,
    show_sidebar: bool,
    #[serde(skip)]
    viewport_apply_pending: bool,
    mode: AppMode,
    #[serde(skip)]
    text_entries: Vec<TextEntry>,
    #[serde(skip)]
    current_window_id: Option<String>,
    #[serde(skip)]
    dump_tree: bool,
    #[serde(skip)]
    tree_dumped: bool,
    #[serde(skip)]
    event_feed: Vec<EventSummary>,
    #[serde(skip)]
    last_event_id: Option<u64>,
}

impl Default for AxTreeApp {
    fn default() -> Self {
        let graph_interaction = SettingsInteraction::new()
            .with_node_selection_enabled(true)
            .with_node_selection_multi_enabled(true)
            .with_node_clicking_enabled(true)
            .with_edge_selection_enabled(false)
            .with_edge_clicking_enabled(false)
            .with_dragging_enabled(false);

        let graph_navigation = SettingsNavigation::new()
            .with_fit_to_screen_enabled(false)
            .with_screen_padding(0.2)
            .with_zoom_and_pan_enabled(true);

        let graph_style = SettingsStyle::new().with_labels_always(true);

        Self {
            graph: new_graph(),
            graph_interaction,
            graph_navigation,
            graph_style,
            graph_needs_reset: true,
            #[cfg(target_os = "macos")]
            observer_shared: None,
            last_diff: None,
            snapshot_counter: 0,
            display_settings: DisplaySettings::default(),
            show_sidebar: true,
            viewport_apply_pending: false,
            mode: AppMode::Graph,
            text_entries: Vec::new(),
            current_window_id: None,
            dump_tree: false,
            tree_dumped: false,
            event_feed: Vec::new(),
            last_event_id: None,
        }
    }
}

impl AxTreeApp {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>, list_mode: bool, dump_tree: bool) -> Self {
        let mut app: Self = if let Some(storage) = cc.storage {
            eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default()
        } else {
            Default::default()
        };

        // Override mode from CLI argument
        if list_mode {
            app.mode = AppMode::List;
        }
        app.dump_tree = dump_tree;

        #[cfg(target_os = "macos")]
        {
            if app.observer_shared.is_none() {
                // Start the accessibility observer in a background thread
                let shared = Arc::new(ObserverShared::new());
                match start_observer_background(Arc::clone(&shared)) {
                    Ok(_handle) => {
                        app.observer_shared = Some(shared);
                        tracing::info!("started accessibility observer background thread");
                    }
                    Err(e) => {
                        tracing::error!(error = ?e, "failed to start accessibility observer");
                    }
                }
            }
        }

        app
    }

    #[cfg(target_os = "macos")]
    fn recorder(&self) -> Option<std::sync::Arc<std::sync::Mutex<AxTreeRecorder>>> {
        self.observer_shared
            .as_ref()
            .map(|s| Arc::clone(&s.recorder()))
    }
}

impl eframe::App for AxTreeApp {
    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let is_web = cfg!(target_arch = "wasm32");
                if !is_web {
                    if ui.button("Quit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    ui.separator();
                }

                ui.label("Theme:");
                if ui.button("☀").clicked() {
                    ctx.set_visuals(egui::Visuals::light());
                }
                if ui.button("🌙").clicked() {
                    ctx.set_visuals(egui::Visuals::dark());
                }
                ui.separator();
                if ui.checkbox(&mut self.show_sidebar, "settings").clicked() {
                    // Sidebar visibility toggled
                }
            });
        });

        if self.show_sidebar {
            egui::SidePanel::right("display_settings")
                .default_width(SIDE_PANEL_WIDTH)
                .min_width(250.0)
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("display_settings")
                        .show(ui, |ui| {
                            self.ui_viewport_settings(ui);
                            self.ui_graph_appearance(ui);
                            self.ui_layout_settings(ui);
                        });
                });
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            if self.mode == AppMode::List {
                self.render_list_mode(ui);
            } else {
                self.render_graph_mode(ui);
            }
        });
    }

    /// Called by the framework to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }
}

impl AxTreeApp {
    #[cfg(target_os = "macos")]
    fn render_graph_mode(&mut self, ui: &mut egui::Ui) {
        ui.heading("Accessibility Tree Viewer");

        let current_snapshot = self
            .recorder()
            .and_then(|recorder_arc| match recorder_arc.lock() {
                Ok(recorder) => recorder.current().cloned(),
                Err(poisoned) => poisoned.into_inner().current().cloned(),
            });

        self.refresh_recorder_state(current_snapshot.as_ref());

        if let Some(current) = current_snapshot.as_ref() {
            let displayed_count = {
                let (order, _) = collect_nodes_to_depth(current, self.display_settings.max_depth);
                order.len()
            };
            ui.horizontal(|ui| {
                ui.label(format!("Snapshots: {}", self.snapshot_counter));
                ui.label(format!(
                    "Nodes: {} / {} (showing / total)",
                    displayed_count,
                    current.nodes.len()
                ));
                if let Some(focused) = current.focused_node() {
                    ui.label(format!("Focused: {}", focused.role));
                }
            });

            if let Some(ref diff) = self.last_diff {
                if diff.has_changes() {
                    ui.horizontal(|ui| {
                        ui.label("Changes:");
                        ui.label(diff.summary());
                    });
                } else {
                    ui.label("No changes since last snapshot");
                }
            } else {
                ui.label("Initial snapshot (no diff available)");
            }

            ui.separator();

            let was_empty = self.graph.node_count() == 0;
            update_graph_from_snapshot(
                &mut self.graph,
                current,
                self.last_diff.as_ref(),
                &self.display_settings,
            );
            if was_empty {
                self.graph_needs_reset = true;
            }

            // Apply zoom/pan settings from display_settings to metadata when explicitly requested
            if self.viewport_apply_pending {
                let mut metadata = Metadata::default();
                metadata.zoom = self.display_settings.zoom;
                metadata.pan = Vec2::new(self.display_settings.pan_x, self.display_settings.pan_y);
                metadata.store_into_ui(ui);
                self.viewport_apply_pending = false;
            }

            if self.graph_needs_reset {
                self.graph_needs_reset = false;
            }

            {
                let mut view = GraphView::new(&mut self.graph)
                    .with_interactions(&self.graph_interaction)
                    .with_navigations(&self.graph_navigation)
                    .with_styles(&self.graph_style);

                ui.add(&mut view);
            }

            let selected_nodes: Vec<_> = self.graph.selected_nodes().iter().copied().collect();
            if !selected_nodes.is_empty() {
                ui.separator();
                ui.heading("Selected node details");
                for idx in selected_nodes {
                    if let Some(node) = self.graph.g.node_weight(idx) {
                        let payload = node.payload();
                        ui.group(|ui| {
                            ui.label(format!("Role: {}", payload.role));
                            if let Some(label) = payload.label.as_ref().filter(|l| !l.is_empty()) {
                                ui.label(format!("Label: {}", label));
                            }
                            if let Some(value) =
                                payload.value.as_ref().filter(|v| !v.trim().is_empty())
                            {
                                ui.label(format!("Value: {}", value.trim()));
                            }
                            if let Some(description) = payload
                                .description
                                .as_ref()
                                .filter(|d| !d.trim().is_empty())
                            {
                                ui.label(format!("Description: {}", description.trim()));
                            }
                            ui.label(format!("Children: {}", payload.children.len()));
                            if let Some(bounds) = &payload.bounds {
                                ui.label(format!(
                                    "Bounds: ({:.0}, {:.0}) {}×{}",
                                    bounds.x, bounds.y, bounds.width, bounds.height
                                ));
                            }
                        });
                    }
                }
            }

            self.render_event_feed(ui);
        } else {
            ui.label("No snapshot available. Waiting for accessibility events...");
            ui.separator();
            self.render_event_feed(ui);
        }
    }

    #[cfg(target_os = "macos")]
    fn refresh_recorder_state(&mut self, current_snapshot: Option<&AxTreeSnapshot>) {
        if let Some(recorder_arc) = self.recorder() {
            if let Ok(recorder) = recorder_arc.lock() {
                if let Some(diff) = recorder.last_diff() {
                    self.last_diff = Some(diff.clone());
                }

                let new_events = recorder.events_since(self.last_event_id);
                if let Some(last) = new_events.last() {
                    self.last_event_id = Some(last.id);
                }
                if !new_events.is_empty() {
                    self.event_feed.extend(new_events);
                    if self.event_feed.len() > EVENT_FEED_LIMIT {
                        let overflow = self.event_feed.len() - EVENT_FEED_LIMIT;
                        self.event_feed.drain(0..overflow);
                    }
                }

                if let Some(current) = current_snapshot {
                    if let Some(active) = recorder.current() {
                        if self.snapshot_counter == 0 || active.timestamp != current.timestamp {
                            self.snapshot_counter += 1;
                        }
                    }
                }
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    fn refresh_recorder_state(&mut self, _current_snapshot: Option<&AxTreeSnapshot>) {}

    #[cfg(target_os = "macos")]
    fn render_event_feed(&self, ui: &mut egui::Ui) {
        if self.event_feed.is_empty() {
            return;
        }

        ui.separator();
        ui.heading("Recent events");

        egui::ScrollArea::vertical()
            .id_salt("event_feed")
            .max_height(200.0)
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                for event in self.event_feed.iter().rev().take(25) {
                    let timestamp = event.timestamp.format("%H:%M:%S");
                    ui.label(format!(
                        "{} • {} • {} ({})",
                        timestamp, event.app_name, event.headline, event.trigger
                    ));

                    if let Some(detail) = &event.detail {
                        ui.label(RichText::new(detail.as_str()).weak());
                    }
                    if let Some(text) = &event.focused_text {
                        ui.label(RichText::new(format!("“{}”", text)).italics());
                    }

                    ui.add_space(6.0);
                }
            });
    }

    #[cfg(not(target_os = "macos"))]
    fn render_event_feed(&self, _ui: &mut egui::Ui) {}

    #[cfg(not(target_os = "macos"))]
    fn render_graph_mode(&mut self, ui: &mut egui::Ui) {
        ui.label("Accessibility tree capture is only available on macOS");
        ui.separator();
    }

    #[cfg(target_os = "macos")]
    fn render_list_mode(&mut self, ui: &mut egui::Ui) {
        ui.heading("Visible Text Tracker");

        let current_snapshot = self
            .recorder()
            .and_then(|recorder_arc| match recorder_arc.lock() {
                Ok(recorder) => recorder.current().cloned(),
                Err(poisoned) => poisoned.into_inner().current().cloned(),
            });

        self.refresh_recorder_state(current_snapshot.as_ref());

        if let Some(current) = current_snapshot.as_ref() {
            // Extract window identifier
            let window_id = get_window_identifier(current);

            // Check if window changed
            let window_changed = self.current_window_id.as_ref() != Some(&window_id);

            if window_changed || self.current_window_id.is_none() {
                // Extract app name for filtering
                let app_name = window_id
                    .split(" - ")
                    .next()
                    .or_else(|| window_id.split(' ').next());

                // Extract visible text (with app-specific filtering)
                let extraction = extract_visible_text(current, app_name);

                // Emit detailed debug logging for analysis
                log_extraction_debug(current, &window_id, &extraction);

                // Dump tree to files if requested
                if self.dump_tree && !self.tree_dumped {
                    match dump_tree_to_files(current, &window_id, &extraction) {
                        Ok((full_path, filtered_path)) => {
                            tracing::debug!(
                                target = "ax_tree::dump",
                                full = %full_path.display(),
                                filtered = %filtered_path.display(),
                                "wrote accessibility dump files"
                            );
                            self.tree_dumped = true;
                        }
                        Err(err) => {
                            tracing::warn!(
                                error = %err,
                                "failed to write accessibility dump files"
                            );
                        }
                    }
                }

                let texts = clamp_texts_to_limit(&extraction.entries, LIST_MODE_CHAR_LIMIT);

                // Only create entry if we have text
                if !texts.is_empty() {
                    let entry = TextEntry {
                        timestamp: current.timestamp,
                        window_id: window_id.clone(),
                        texts,
                    };
                    self.text_entries.push(entry);
                    self.current_window_id = Some(window_id);
                } else if window_changed {
                    // Update window ID even if no text (for next snapshot)
                    self.current_window_id = Some(window_id);
                }
            }

            // Display accumulated entries
            ui.label(format!("Entries: {}", self.text_entries.len()));
            ui.separator();

            egui::ScrollArea::vertical()
                .id_salt("list_entries")
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    // Display entries in reverse order (newest first)
                    for entry in self.text_entries.iter().rev() {
                        ui.group(|ui| {
                            // Timestamp
                            ui.label(format!("{}", entry.timestamp.format("%H:%M:%S")));
                            // Window identifier
                            ui.label(format!("Window: {}", entry.window_id));
                            // Texts
                            if entry.texts.is_empty() {
                                ui.label("(no text)");
                            } else {
                                for text in &entry.texts {
                                    ui.label(format!("  • {}", text));
                                }
                            }
                        });
                        ui.add_space(8.0);
                    }
                });

            self.render_event_feed(ui);
        } else {
            ui.label("No snapshot available. Waiting for accessibility events...");
            ui.separator();
            self.render_event_feed(ui);
        }
    }

    #[cfg(not(target_os = "macos"))]
    fn render_list_mode(&mut self, ui: &mut egui::Ui) {
        ui.label("Accessibility tree capture is only available on macOS");
        ui.separator();
    }

    fn ui_viewport_settings(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("Viewport")
            .default_open(true)
            .show(ui, |ui| {
                ui.add(egui::Slider::new(&mut self.display_settings.zoom, 0.1..=5.0).text("zoom"));
                ui.add(
                    egui::Slider::new(&mut self.display_settings.pan_x, -500.0..=500.0)
                        .text("pan x"),
                );
                ui.add(
                    egui::Slider::new(&mut self.display_settings.pan_y, -500.0..=500.0)
                        .text("pan y"),
                );
                ui.horizontal(|ui| {
                    if ui.button("apply").clicked() {
                        self.viewport_apply_pending = true;
                    }
                    if ui.button("reset").clicked() {
                        self.display_settings.zoom = DisplaySettings::default().zoom;
                        self.display_settings.pan_x = DisplaySettings::default().pan_x;
                        self.display_settings.pan_y = DisplaySettings::default().pan_y;
                        self.viewport_apply_pending = true;
                    }
                });
                ui.label("slider changes require 'apply'");
            });
    }

    fn ui_graph_appearance(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("Graph Appearance")
            .default_open(true)
            .show(ui, |ui| {
                ui.label("Node Sizes");
                ui.add(
                    egui::Slider::new(&mut self.display_settings.node_radius_normal, 3.0..=20.0)
                        .text("radius normal"),
                );
                ui.add(
                    egui::Slider::new(&mut self.display_settings.node_radius_highlight, 5.0..=25.0)
                        .text("radius highlight"),
                );
                ui.separator();
                ui.label("Edge Widths");
                ui.add(
                    egui::Slider::new(&mut self.display_settings.edge_width_normal, 1.0..=10.0)
                        .text("width normal"),
                );
                ui.add(
                    egui::Slider::new(&mut self.display_settings.edge_width_highlight, 2.0..=15.0)
                        .text("width highlight"),
                );
            });
    }

    fn ui_layout_settings(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("Layout")
            .default_open(true)
            .show(ui, |ui| {
                ui.add(
                    egui::Slider::new(&mut self.display_settings.max_depth, 1..=10)
                        .text("max depth"),
                );
                ui.add(
                    egui::Slider::new(&mut self.display_settings.label_truncate_len, 10..=100)
                        .text("label truncate length"),
                );
                ui.separator();
                ui.label("Spacing");
                ui.add(
                    egui::Slider::new(&mut self.display_settings.horizontal_spacing, 50.0..=500.0)
                        .text("horizontal spacing"),
                );
                ui.add(
                    egui::Slider::new(&mut self.display_settings.vertical_spacing, 30.0..=200.0)
                        .text("vertical spacing"),
                );
            });
    }
}

fn update_graph_from_snapshot(
    graph: &mut AxGraph,
    snapshot: &AxTreeSnapshot,
    diff: Option<&AxTreeDiff>,
    settings: &DisplaySettings,
) {
    let (order, nodes) = collect_nodes_to_depth(snapshot, settings.max_depth);

    if order.is_empty() {
        *graph = new_graph();
        return;
    }

    let added: HashSet<AxNodeId> = diff
        .map(|d| d.added_nodes.iter().copied().collect())
        .unwrap_or_default();
    let updated: HashSet<AxNodeId> = diff
        .map(|d| d.updated_nodes.iter().copied().collect())
        .unwrap_or_default();
    let focused = snapshot.focused_node_id;

    let mut new_graph = new_graph();
    let mut indices = HashMap::new();

    let highlight_ids: HashSet<AxNodeId> = added
        .iter()
        .chain(updated.iter())
        .chain(focused.iter())
        .copied()
        .collect();

    let positions = layout_tree_positions(
        &nodes,
        snapshot.root_id,
        settings.horizontal_spacing,
        settings.vertical_spacing,
    );

    for node_id in &order {
        let Some(node_data) = nodes.get(node_id) else {
            continue;
        };

        let label = node_data
            .label
            .clone()
            .unwrap_or_else(|| node_data.role.clone());

        let mut prefix = String::new();
        if added.contains(node_id) {
            prefix.push_str("➕ ");
        } else if updated.contains(node_id) {
            prefix.push_str("✎ ");
        }
        if Some(*node_id) == focused || node_data.is_focused {
            prefix.push_str("★ ");
        }

        let mut display_label = if prefix.is_empty() {
            label
        } else {
            format!("{prefix}{label}")
        };

        if let Some(value) = node_data.value.as_ref().and_then(|v| {
            (!v.trim().is_empty()).then(|| summarize_text(v, settings.label_truncate_len))
        }) {
            display_label = format!("{display_label} • {value}");
        } else if let Some(desc) = node_data.description.as_ref().and_then(|d| {
            (!d.trim().is_empty()).then(|| summarize_text(d, settings.label_truncate_len))
        }) {
            display_label = format!("{display_label} • {desc}");
        }

        let position = positions
            .get(node_id)
            .copied()
            .unwrap_or_else(|| egui::pos2(0.0, 0.0));

        // Use add_node_with_label_and_location - accepts Pos2 for position
        let node_index = new_graph.add_node_with_label_and_location(
            node_data.clone(),
            display_label,
            position, // Pos2 is what it expects
        );

        // Set display properties after adding node
        if let Some(node) = new_graph.node_mut(node_index) {
            let highlight = highlight_ids.contains(node_id);
            let radius = if highlight {
                settings.node_radius_highlight
            } else {
                settings.node_radius_normal
            };
            node.display_mut().radius = radius;

            if Some(*node_id) == focused || node_data.is_focused {
                node.set_selected(true);
            }
        }
        indices.insert(*node_id, node_index);
    }

    for node_id in &order {
        let Some(node_data) = nodes.get(node_id) else {
            continue;
        };
        let Some(&from_idx) = indices.get(node_id) else {
            continue;
        };

        for child in &node_data.children {
            if let Some(&to_idx) = indices.get(child) {
                let edge_idx = new_graph.add_edge(from_idx, to_idx, ());
                if let Some(edge) = new_graph.edge_mut(edge_idx) {
                    let highlight =
                        highlight_ids.contains(node_id) || highlight_ids.contains(child);
                    edge.display_mut().width = if highlight {
                        settings.edge_width_highlight
                    } else {
                        settings.edge_width_normal
                    };
                }
            }
        }
    }

    *graph = new_graph;
}

fn collect_nodes_to_depth(
    snapshot: &AxTreeSnapshot,
    max_depth: usize,
) -> (Vec<AxNodeId>, HashMap<AxNodeId, TreeAxNodeData>) {
    let mut order = Vec::new();
    let mut nodes = HashMap::new();

    if snapshot.nodes.is_empty() {
        return (order, nodes);
    }

    let mut queue = VecDeque::new();
    let mut visited = HashSet::new();
    queue.push_back((snapshot.root_id, 0usize));

    while let Some((node_id, depth)) = queue.pop_front() {
        if depth > max_depth || !visited.insert(node_id) {
            continue;
        }

        if let Some(node) = snapshot.node(node_id) {
            order.push(node_id);
            nodes.insert(node_id, node.clone());

            for child in &node.children {
                queue.push_back((*child, depth + 1));
            }
        }
    }

    let existing: HashSet<AxNodeId> = nodes.keys().copied().collect();
    for node in nodes.values_mut() {
        node.children.retain(|child| existing.contains(child));
    }

    (order, nodes)
}

fn layout_tree_positions(
    nodes: &HashMap<AxNodeId, TreeAxNodeData>,
    root_id: AxNodeId,
    horizontal_spacing: f32,
    vertical_spacing: f32,
) -> HashMap<AxNodeId, egui::Pos2> {
    fn layout_node(
        node_id: AxNodeId,
        nodes: &HashMap<AxNodeId, TreeAxNodeData>,
        positions: &mut HashMap<AxNodeId, egui::Pos2>,
        depth: usize,
        y_offset: &mut f32,
        horizontal_spacing: f32,
        vertical_spacing: f32,
    ) {
        if let Some(node) = nodes.get(&node_id) {
            let x = depth as f32 * horizontal_spacing + 40.0;
            let y = *y_offset;
            positions.insert(node_id, egui::pos2(x, y));

            *y_offset += vertical_spacing;

            for child in &node.children {
                layout_node(
                    *child,
                    nodes,
                    positions,
                    depth + 1,
                    y_offset,
                    horizontal_spacing,
                    vertical_spacing,
                );
            }
        }
    }

    let mut positions = HashMap::new();
    if nodes.contains_key(&root_id) {
        let mut y_offset = 40.0;
        layout_node(
            root_id,
            nodes,
            &mut positions,
            0,
            &mut y_offset,
            horizontal_spacing,
            vertical_spacing,
        );
    }

    positions
}

fn summarize_text(text: &str, max_chars: usize) -> String {
    let trimmed = text.trim();
    let mut chars = trimmed.chars();
    let mut result = String::new();
    for _ in 0..max_chars {
        if let Some(c) = chars.next() {
            result.push(c);
        } else {
            break;
        }
    }
    if result.len() < trimmed.len() {
        result.push('…');
    }
    result
}

// Allowed roles for text extraction (excludes buttons, menus, etc.)
const ALLOWED_ROLES: &[&str] = &[
    "AXStaticText",
    "AXTextField",
    "AXTextArea",
    "AXText",
    "AXHeading",
    "AXParagraph",
    "AXLink",
];

/// Text filtering configuration
struct TextFilter {
    /// URL patterns to filter out (e.g., "moz-extension://", "http://", "https://")
    url_patterns: Vec<String>,
    /// Text prefixes to filter out
    prefix_filters: Vec<String>,
    /// Text suffixes to filter out
    suffix_filters: Vec<String>,
    /// Substrings that, if present in text, cause it to be filtered
    contains_filters: Vec<String>,
    /// When enabled, skip text nodes that belong to browser tab bars
    skip_tab_bar_text: bool,
}

impl TextFilter {
    /// Create default text filter with common patterns
    fn default() -> Self {
        Self {
            url_patterns: vec!["moz-extension://".to_string()],
            prefix_filters: vec!["(Discarded Window)".to_string()],
            suffix_filters: vec![],
            contains_filters: vec![],
            skip_tab_bar_text: false,
        }
    }

    /// Check if text should be filtered out
    fn should_filter(&self, text: &str) -> bool {
        // Check URL patterns
        for pattern in &self.url_patterns {
            if text.contains(pattern) {
                return true;
            }
        }

        // Check prefix filters
        for prefix in &self.prefix_filters {
            if text.starts_with(prefix) {
                return true;
            }
        }

        // Check suffix filters
        for suffix in &self.suffix_filters {
            if text.ends_with(suffix) {
                return true;
            }
        }

        // Check contains filters
        for contains in &self.contains_filters {
            if text.contains(contains) {
                return true;
            }
        }

        false
    }
}

/// Result of extracting visible text from a snapshot
struct TextExtraction {
    entries: Vec<CollectedText>,
    root_node_id: AxNodeId,
}

struct CollectedText {
    text: String,
    node_id: AxNodeId,
}

/// Extract visible text from a snapshot based on allowed roles
/// For Firefox, only extracts from the focused tab's subtree
fn extract_visible_text(snapshot: &AxTreeSnapshot, app_name: Option<&str>) -> TextExtraction {
    let mut filter = TextFilter::default();

    // For Firefox, find focused tab and only extract from that subtree
    let root_node_id = if let Some(name) = app_name {
        let name_lower = name.to_lowercase();
        if name_lower.contains("firefox") {
            // Find the focused tab node
            filter.skip_tab_bar_text = true;
            find_focused_tab_root(snapshot).unwrap_or(snapshot.root_id)
        } else {
            snapshot.root_id
        }
    } else {
        snapshot.root_id
    };

    let entries = extract_visible_text_impl(snapshot, root_node_id, &filter);
    TextExtraction {
        entries,
        root_node_id,
    }
}

/// Find the root node of the focused tab in Firefox
/// Returns the node ID of the focused tab container, or None if not found
fn find_focused_tab_root(snapshot: &AxTreeSnapshot) -> Option<AxNodeId> {
    // Look for nodes with is_focused=true, then find their tab container parent
    fn find_focused_node(nodes: &HashMap<AxNodeId, TreeAxNodeData>) -> Option<AxNodeId> {
        nodes
            .iter()
            .find(|(_, node)| node.is_focused)
            .map(|(id, _)| *id)
    }

    // First, try to find a directly focused node
    if let Some(focused_id) = find_focused_node(&snapshot.nodes) {
        // Walk up the tree to find tab container
        // Tab containers in Firefox typically have specific roles or are parents of focused content
        let mut current_id = Some(focused_id);
        let mut visited = HashSet::new();

        while let Some(node_id) = current_id {
            if !visited.insert(node_id) {
                break; // Avoid cycles
            }

            if let Some(node) = snapshot.node(node_id) {
                // Check if this looks like a tab container
                // Tabs in Firefox might be identified by having children with focused nodes
                // or by specific role patterns
                if node.is_focused || node.role.contains("Tab") {
                    return Some(node_id);
                }

                // Find parent by checking all nodes for this one as a child
                current_id = snapshot
                    .nodes
                    .iter()
                    .find(|(_, n)| n.children.contains(&node_id))
                    .map(|(id, _)| *id);
            } else {
                break;
            }
        }
    }

    None
}

/// Internal implementation of text extraction from a specific root node
fn extract_visible_text_impl(
    snapshot: &AxTreeSnapshot,
    root_node_id: AxNodeId,
    filter: &TextFilter,
) -> Vec<CollectedText> {
    let parent_map = build_parent_map(snapshot);
    let mut texts = Vec::new();
    let mut visited = HashSet::new();

    fn collect_texts(
        node_id: AxNodeId,
        snapshot: &AxTreeSnapshot,
        visited: &mut HashSet<AxNodeId>,
        texts: &mut Vec<CollectedText>,
        filter: &TextFilter,
        parent_map: &HashMap<AxNodeId, AxNodeId>,
    ) {
        if !visited.insert(node_id) {
            return; // Skip already visited nodes (avoid cycles)
        }

        let Some(node) = snapshot.node(node_id) else {
            return;
        };

        let role_allowed = ALLOWED_ROLES
            .iter()
            .any(|&allowed| node.role.eq_ignore_ascii_case(allowed));

        if role_allowed {
            if filter.skip_tab_bar_text && is_tab_bar_text(node_id, parent_map, snapshot) {
                return;
            }

            if let Some(label) = node.label.as_ref() {
                let trimmed = label.trim();
                if !trimmed.is_empty() && !filter.should_filter(trimmed) {
                    texts.push(CollectedText {
                        text: trimmed.to_string(),
                        node_id,
                    });
                }
            }

            if let Some(value) = node.value.as_ref() {
                let trimmed = value.trim();
                if !trimmed.is_empty() && !filter.should_filter(trimmed) {
                    texts.push(CollectedText {
                        text: trimmed.to_string(),
                        node_id,
                    });
                }
            }

            if let Some(desc) = node.description.as_ref() {
                let trimmed = desc.trim();
                if !trimmed.is_empty() && !filter.should_filter(trimmed) {
                    texts.push(CollectedText {
                        text: trimmed.to_string(),
                        node_id,
                    });
                }
            }
        }

        for child_id in &node.children {
            collect_texts(*child_id, snapshot, visited, texts, filter, parent_map);
        }
    }

    collect_texts(
        root_node_id,
        snapshot,
        &mut visited,
        &mut texts,
        filter,
        &parent_map,
    );

    let mut unique_texts = Vec::new();
    let mut seen = HashSet::new();
    for entry in texts.into_iter() {
        if seen.insert(entry.text.clone()) {
            unique_texts.push(entry);
        }
    }

    unique_texts
}

fn clamp_texts_to_limit(entries: &[CollectedText], limit: usize) -> Vec<String> {
    if limit == 0 {
        return Vec::new();
    }

    let mut remaining = limit;
    let mut texts = Vec::new();

    for entry in entries {
        let text = entry.text.trim();
        if text.is_empty() {
            continue;
        }

        let text_chars = text.chars().count();
        if text_chars <= remaining {
            texts.push(text.to_string());
            remaining -= text_chars;
            if remaining == 0 {
                break;
            }
            continue;
        }

        if remaining == 0 {
            break;
        }

        let mut truncated = String::new();
        let mut taken = 0;
        for ch in text.chars() {
            if taken >= remaining {
                break;
            }
            truncated.push(ch);
            taken += 1;
        }

        let trimmed = truncated.trim_end();
        if !trimmed.is_empty() {
            let final_text = if trimmed.len() < text.len() {
                format!("{trimmed}…")
            } else {
                trimmed.to_string()
            };
            texts.push(final_text);
        }
        break;
    }

    texts
}

fn build_parent_map(snapshot: &AxTreeSnapshot) -> HashMap<AxNodeId, AxNodeId> {
    let mut parents = HashMap::new();
    for (parent_id, node) in &snapshot.nodes {
        for child in &node.children {
            parents.insert(*child, *parent_id);
        }
    }
    parents
}

fn format_node_path(
    node_id: AxNodeId,
    parent_map: &HashMap<AxNodeId, AxNodeId>,
    snapshot: &AxTreeSnapshot,
) -> String {
    let mut chain = Vec::new();
    let mut current = Some(node_id);
    while let Some(id) = current {
        chain.push(id);
        current = parent_map.get(&id).copied();
    }

    chain
        .iter()
        .rev()
        .map(|id| {
            if let Some(node) = snapshot.node(*id) {
                let mut part = node.role.clone();
                if let Some(label) = node.label.as_ref().filter(|l| !l.trim().is_empty()) {
                    part.push_str(&format!("['{}']", label.trim()));
                } else if let Some(value) = node.value.as_ref().filter(|v| !v.trim().is_empty()) {
                    part.push_str(&format!("(value='{}')", value.trim()));
                }
                part
            } else {
                format!("Unknown({:?})", id)
            }
        })
        .collect::<Vec<_>>()
        .join(" -> ")
}

fn is_tab_bar_text(
    mut node_id: AxNodeId,
    parent_map: &HashMap<AxNodeId, AxNodeId>,
    snapshot: &AxTreeSnapshot,
) -> bool {
    let mut saw_toolbar = false;
    let mut saw_tab_group = false;
    while let Some(node) = snapshot.node(node_id) {
        let role_lower = node.role.to_ascii_lowercase();
        if role_lower == "axtoolbar" {
            saw_toolbar = true;
        } else if role_lower == "axtabgroup" {
            saw_tab_group = true;
        }

        if saw_toolbar && saw_tab_group {
            return true;
        }

        if let Some(parent) = parent_map.get(&node_id) {
            node_id = *parent;
        } else {
            break;
        }
    }
    false
}

#[cfg(target_os = "macos")]
fn log_extraction_debug(snapshot: &AxTreeSnapshot, window_id: &str, extraction: &TextExtraction) {
    if !tracing::level_enabled!(tracing::Level::DEBUG) {
        return;
    }

    let parent_map = build_parent_map(snapshot);
    let mut log_output = String::new();
    let _ = FmtWrite::write_str(
        &mut log_output,
        "================ AX TREE DEBUG ================\n",
    );
    let _ = writeln!(log_output, "Timestamp: {}", snapshot.timestamp);
    let _ = writeln!(log_output, "Window: {}", window_id);
    let _ = writeln!(
        log_output,
        "Extraction root node: {:?}",
        extraction.root_node_id
    );

    if let Some(focused_id) = snapshot.focused_node_id {
        let _ = FmtWrite::write_str(
            &mut log_output,
            "Focused node chain (child -> ancestors):\n",
        );
        let mut current_id = Some(focused_id);
        let mut chain = Vec::new();
        while let Some(node_id) = current_id {
            chain.push(node_id);
            if let Some(node) = snapshot.node(node_id) {
                let _ = writeln!(
                    log_output,
                    "  id {:?} | {}",
                    node_id,
                    format_node_debug(node)
                );
                current_id = parent_map.get(&node_id).copied();
            } else {
                break;
            }
        }

        for (depth, node_id) in chain.iter().enumerate() {
            log_child_summary(snapshot, *node_id, depth + 1, &mut log_output);
        }
    } else {
        let _ = FmtWrite::write_str(&mut log_output, "Focused node: none\n");
        let mut focused_candidates = snapshot
            .nodes
            .iter()
            .filter(|(_, node)| node.is_focused)
            .map(|(id, _)| *id)
            .collect::<Vec<_>>();
        let candidate_count = focused_candidates.len();
        focused_candidates.sort_by_key(|id| id.0);
        if candidate_count > 0 {
            let _ = writeln!(
                log_output,
                "Other nodes with is_focused=true: {}",
                candidate_count
            );
            for candidate in focused_candidates.into_iter().take(5) {
                if let Some(node) = snapshot.node(candidate) {
                    let _ = writeln!(
                        log_output,
                        "  id {:?} | {}",
                        candidate,
                        format_node_debug(node)
                    );
                    let _ = writeln!(
                        log_output,
                        "    path={}",
                        format_node_path(candidate, &parent_map, snapshot)
                    );
                }
            }
        }
    }

    let _ = writeln!(
        log_output,
        "Filtered texts ({} entries):",
        extraction.entries.len()
    );
    for entry in &extraction.entries {
        let _ = writeln!(log_output, "  • {}", entry.text);
        let _ = writeln!(
            log_output,
            "      path={}",
            format_node_path(entry.node_id, &parent_map, snapshot)
        );
    }

    let _ = FmtWrite::write_str(
        &mut log_output,
        "================================================\n",
    );
    tracing::debug!(target = "ax_tree::extraction", "{}", log_output);
}

#[cfg(target_os = "macos")]
fn format_node_debug(node: &TreeAxNodeData) -> String {
    let mut parts = Vec::new();
    parts.push(format!("role={}", node.role));
    if let Some(label) = node.label.as_ref() {
        if !label.trim().is_empty() {
            parts.push(format!("label='{}'", label.trim()));
        }
    }
    if let Some(value) = node.value.as_ref() {
        if !value.trim().is_empty() {
            parts.push(format!("value='{}'", value.trim()));
        }
    }
    if node.is_focused {
        parts.push("focused=true".to_string());
    }
    if let Some(bounds) = node.bounds.as_ref() {
        parts.push(format!(
            "bounds=({}, {}), {}x{}",
            bounds.x, bounds.y, bounds.width, bounds.height
        ));
    }
    parts.join(", ")
}

#[cfg(target_os = "macos")]
fn log_child_summary(
    snapshot: &AxTreeSnapshot,
    node_id: AxNodeId,
    indent_depth: usize,
    buffer: &mut String,
) {
    if let Some(node) = snapshot.node(node_id) {
        if node.children.is_empty() {
            return;
        }

        let mut role_counts: BTreeMap<&str, usize> = BTreeMap::new();
        let mut sample_labels: BTreeMap<&str, Vec<String>> = BTreeMap::new();

        for child_id in &node.children {
            if let Some(child) = snapshot.node(*child_id) {
                let role = child.role.as_str();
                *role_counts.entry(role).or_insert(0) += 1;
                if let Some(label) = child.label.as_ref() {
                    if !label.trim().is_empty() {
                        sample_labels
                            .entry(role)
                            .or_default()
                            .push(label.trim().to_string());
                    }
                }
            }
        }

        if !role_counts.is_empty() {
            let indent = "  ".repeat(indent_depth);
            let _ = writeln!(buffer, "{}Child roles summary:", indent);
            for (role, count) in role_counts {
                let samples = sample_labels
                    .get(role)
                    .map(|labels| {
                        let preview: Vec<&str> =
                            labels.iter().take(3).map(String::as_str).collect();
                        if preview.is_empty() {
                            String::new()
                        } else {
                            format!(" [{}]", preview.join(", "))
                        }
                    })
                    .unwrap_or_default();
                let _ = writeln!(buffer, "{}  - {} ({}{})", indent, role, count, samples);
            }
        }
    }
}

/// Dump full tree and filtered results to files for analysis
#[cfg(target_os = "macos")]
fn dump_tree_to_files(
    snapshot: &AxTreeSnapshot,
    window_id: &str,
    extraction: &TextExtraction,
) -> io::Result<(PathBuf, PathBuf)> {
    let timestamp_str = snapshot.timestamp.format("%Y%m%d_%H%M%S").to_string();
    let safe_window_id = window_id
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(50)
        .collect::<String>();

    let full_tree_path = PathBuf::from(format!(
        "ax_tree_full_{}_{}.txt",
        safe_window_id, timestamp_str
    ));
    let filtered_path = PathBuf::from(format!(
        "ax_tree_filtered_{}_{}.txt",
        safe_window_id, timestamp_str
    ));

    let mut full_tree_file = File::create(&full_tree_path)?;
    writeln!(full_tree_file, "AX TREE FULL DUMP")?;
    writeln!(full_tree_file, "==================")?;
    writeln!(full_tree_file, "Timestamp: {}", snapshot.timestamp)?;
    writeln!(full_tree_file, "Window: {}", window_id)?;
    writeln!(full_tree_file, "PID: {}", snapshot.pid)?;
    writeln!(full_tree_file, "Total nodes: {}", snapshot.nodes.len())?;
    writeln!(full_tree_file, "Root node: {:?}", snapshot.root_id)?;
    writeln!(
        full_tree_file,
        "Focused node: {:?}",
        snapshot.focused_node_id
    )?;
    writeln!(full_tree_file)?;

    let parent_map = build_parent_map(snapshot);

    writeln!(full_tree_file, "TREE STRUCTURE:")?;
    writeln!(full_tree_file, "===============")?;
    let mut visited = HashSet::new();
    dump_node_tree(
        &mut full_tree_file,
        snapshot,
        snapshot.root_id,
        0,
        &mut visited,
    )?;

    writeln!(full_tree_file)?;
    writeln!(full_tree_file, "ALL NODES (by ID):")?;
    writeln!(full_tree_file, "==================")?;
    let mut sorted_nodes: Vec<_> = snapshot.nodes.iter().collect();
    sorted_nodes.sort_by_key(|(id, _)| id.0);
    for (id, node) in sorted_nodes {
        writeln!(full_tree_file, "Node {:?}:", id)?;
        writeln!(full_tree_file, "  role: {}", node.role)?;
        if let Some(label) = node.label.as_ref() {
            writeln!(full_tree_file, "  label: {}", label)?;
        }
        if let Some(value) = node.value.as_ref() {
            writeln!(full_tree_file, "  value: {}", value)?;
        }
        if let Some(desc) = node.description.as_ref() {
            writeln!(full_tree_file, "  description: {}", desc)?;
        }
        writeln!(full_tree_file, "  is_focused: {}", node.is_focused)?;
        if let Some(bounds) = node.bounds.as_ref() {
            writeln!(
                full_tree_file,
                "  bounds: x={}, y={}, width={}, height={}",
                bounds.x, bounds.y, bounds.width, bounds.height
            )?;
        }
        writeln!(full_tree_file, "  children: {:?}", node.children)?;
        writeln!(full_tree_file)?;
    }

    let mut filtered_file = File::create(&filtered_path)?;
    writeln!(filtered_file, "AX TREE FILTERED RESULTS")?;
    writeln!(filtered_file, "========================")?;
    writeln!(filtered_file, "Timestamp: {}", snapshot.timestamp)?;
    writeln!(filtered_file, "Window: {}", window_id)?;
    writeln!(
        filtered_file,
        "Extraction root node: {:?}",
        extraction.root_node_id
    )?;
    writeln!(filtered_file)?;

    writeln!(
        filtered_file,
        "FILTERED TEXTS ({} entries):",
        extraction.entries.len()
    )?;
    writeln!(filtered_file, "============================")?;
    for entry in &extraction.entries {
        writeln!(filtered_file, "Text: {}", entry.text)?;
        writeln!(
            filtered_file,
            "Path: {}",
            format_node_path(entry.node_id, &parent_map, snapshot)
        )?;
        if let Some(node) = snapshot.node(entry.node_id) {
            writeln!(filtered_file, "Role: {}", node.role)?;
            if let Some(label) = node.label.as_ref() {
                writeln!(filtered_file, "Label: {}", label)?;
            }
            if let Some(value) = node.value.as_ref() {
                writeln!(filtered_file, "Value: {}", value)?;
            }
            if let Some(desc) = node.description.as_ref() {
                writeln!(filtered_file, "Description: {}", desc)?;
            }
        }
        writeln!(filtered_file)?;
    }

    writeln!(filtered_file)?;
    writeln!(filtered_file, "RAW SNAPSHOT JSON (for debugging):")?;
    writeln!(filtered_file, "==============================")?;
    let json_dump =
        serde_json::to_string_pretty(&snapshot.nodes).unwrap_or_else(|_| "<failed>".into());
    writeln!(filtered_file, "{}", json_dump)?;

    Ok((full_tree_path, filtered_path))
}

/// Recursively dump tree structure
#[cfg(target_os = "macos")]
fn dump_node_tree(
    file: &mut File,
    snapshot: &AxTreeSnapshot,
    node_id: AxNodeId,
    depth: usize,
    visited: &mut HashSet<AxNodeId>,
) -> io::Result<()> {
    if !visited.insert(node_id) {
        return Ok(()); // Skip cycles
    }

    let indent = "  ".repeat(depth);
    if let Some(node) = snapshot.node(node_id) {
        writeln!(file, "{}Node {:?}:", indent, node_id)?;
        writeln!(file, "{}  role: {}", indent, node.role)?;
        if let Some(label) = node.label.as_ref().filter(|l| !l.trim().is_empty()) {
            writeln!(file, "{}  label: '{}'", indent, label.trim())?;
        }
        if let Some(value) = node.value.as_ref().filter(|v| !v.trim().is_empty()) {
            writeln!(file, "{}  value: '{}'", indent, value.trim())?;
        }
        if let Some(desc) = node.description.as_ref().filter(|d| !d.trim().is_empty()) {
            writeln!(file, "{}  description: '{}'", indent, desc.trim())?;
        }
        writeln!(file, "{}  is_focused: {}", indent, node.is_focused)?;
        if let Some(bounds) = node.bounds.as_ref() {
            writeln!(
                file,
                "{}  bounds: ({}, {}), {}x{}",
                indent, bounds.x, bounds.y, bounds.width, bounds.height
            )?;
        }
        writeln!(file, "{}  children ({}):", indent, node.children.len())?;

        for child_id in &node.children {
            dump_node_tree(file, snapshot, *child_id, depth + 1, visited)?;
        }
    }

    Ok(())
}

/// Get window identifier from snapshot (app name + window title or PID)
#[cfg(target_os = "macos")]
fn get_window_identifier(snapshot: &AxTreeSnapshot) -> String {
    use objc2_app_kit::NSRunningApplication;

    let pid = snapshot.pid;
    if pid != 0 {
        let running_app = NSRunningApplication::runningApplicationWithProcessIdentifier(pid);

        let app_name = running_app
            .as_ref()
            .and_then(|app| {
                app.localizedName().as_ref().map(|name| {
                    use crate::observer::ns_string_to_string;
                    ns_string_to_string(name)
                })
            })
            .or_else(|| {
                running_app.as_ref().and_then(|app| {
                    app.bundleIdentifier().as_ref().map(|id| {
                        use crate::observer::ns_string_to_string;
                        ns_string_to_string(id)
                    })
                })
            })
            .unwrap_or_else(|| format!("pid {}", pid));

        // Try to get window title from focused window
        let window_title = if let Some(root) = snapshot.root() {
            // Look for window title in root or focused node
            let focused_id = snapshot.focused_node_id;
            let node_to_check = focused_id
                .and_then(|id| snapshot.node(id))
                .or_else(|| Some(root));

            if let Some(node) = node_to_check {
                // Try to get title from properties or value
                node.value
                    .as_ref()
                    .or_else(|| node.label.as_ref())
                    .map(|s| s.trim().to_string())
            } else {
                None
            }
        } else {
            None
        };

        if let Some(title) = window_title {
            if !title.is_empty() && title != app_name {
                format!("{} - {}", app_name, title)
            } else {
                app_name
            }
        } else {
            app_name
        }
    } else {
        format!("unknown (pid {})", pid)
    }
}

#[cfg(not(target_os = "macos"))]
fn get_window_identifier(snapshot: &AxTreeSnapshot) -> String {
    format!("pid {}", snapshot.pid)
}
