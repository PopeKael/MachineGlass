#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui::{self, Color32, RichText};
use machineglass::{
    config::{self, Config},
    engine::Engine,
    model::*,
    storage::{self, Drive, Entry, StorageSession},
    tray::{self, Tray},
};
use std::collections::HashMap;
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

const ACCENT: Color32 = Color32::from_rgb(63, 213, 183);
const BLUE: Color32 = Color32::from_rgb(98, 167, 244);
const MUTED: Color32 = Color32::from_rgb(150, 166, 183);

#[derive(Clone, Copy, PartialEq)]
enum Page {
    Overview,
    Processes,
    Instrument,
    Storage,
}
#[derive(Clone, Copy, PartialEq)]
enum Sort {
    Cpu,
    Memory,
    Name,
    Pid,
}

struct MachineGlass {
    engine: Engine,
    tray: Option<Tray>,
    config: Config,
    page: Page,
    query: String,
    sort: Sort,
    descending: bool,
    selected: Option<ProcessIdentity>,
    capture: Option<PathBuf>,
    capture_requested: bool,
    quit_observed: bool,
    always_on_top: bool,
    drives: Vec<Drive>,
    drive_index: Option<usize>,
    storage_session: Option<StorageSession>,
    storage_root: Option<PathBuf>,
    storage_selected: Option<PathBuf>,
    storage_hovered: Option<PathBuf>,
}

fn bytes(v: u64) -> String {
    if v >= 1 << 30 {
        format!("{:.2} GiB", v as f64 / (1u64 << 30) as f64)
    } else {
        format!("{:.1} MiB", v as f64 / (1u64 << 20) as f64)
    }
}
fn compact_bytes(v: u64) -> String {
    if v >= 1 << 30 {
        format!("{:.1}G", v as f64 / (1u64 << 30) as f64)
    } else if v >= 1 << 20 {
        format!("{:.0}M", v as f64 / (1u64 << 20) as f64)
    } else {
        format!("{:.0}K", v as f64 / 1024.0)
    }
}
fn percent(r: &Reading<f64>) -> String {
    match r {
        Reading::Value(v) => format!("{v:.1}%"),
        Reading::WarmingUp => "Sampling…".into(),
        Reading::Unsupported(_) => "Unsupported".into(),
        _ => "Unavailable".into(),
    }
}
fn memory_value(r: &Reading<u64>) -> String {
    r.value()
        .map(|v| bytes(*v))
        .unwrap_or_else(|| "Unavailable".into())
}

fn storage_color(index: usize) -> Color32 {
    const COLORS: [Color32; 8] = [
        Color32::from_rgb(36, 112, 113),
        Color32::from_rgb(52, 89, 132),
        Color32::from_rgb(112, 91, 46),
        Color32::from_rgb(94, 70, 119),
        Color32::from_rgb(42, 113, 85),
        Color32::from_rgb(130, 74, 70),
        Color32::from_rgb(52, 105, 115),
        Color32::from_rgb(105, 100, 55),
    ];
    COLORS[index % COLORS.len()]
}

struct StorageDraw<'a> {
    children: &'a HashMap<PathBuf, Vec<PathBuf>>,
    entries: &'a HashMap<PathBuf, Entry>,
    hovered: Option<PathBuf>,
    selected: &'a mut Option<PathBuf>,
    capacity: Option<u64>,
}

impl StorageDraw<'_> {
    fn draw(
        &mut self,
        ui: &mut egui::Ui,
        parent: &Path,
        rect: egui::Rect,
        depth: usize,
        parent_color: Color32,
    ) {
        if depth >= 8 || rect.width() < 14.0 || rect.height() < 14.0 {
            return;
        }
        let mut children: Vec<&Entry> = self
            .children
            .get(parent)
            .into_iter()
            .flatten()
            .filter_map(|path| self.entries.get(path))
            .collect();
        if children.is_empty() {
            return;
        }
        let total_discovered = children.iter().map(|entry| entry.size).sum::<u64>();
        let mut remainder_size = 0;
        let mut remainder_count = 0;
        if children.len() > 79 {
            children.select_nth_unstable_by(79, |a, b| b.size.cmp(&a.size));
            remainder_size = children[79..].iter().map(|entry| entry.size).sum();
            remainder_count = children.len() - 79;
            children.truncate(79);
        }
        children.sort_by_key(|entry| std::cmp::Reverse(entry.size));
        let mut visible: Vec<Entry> = children.into_iter().cloned().collect();
        if remainder_count > 0 {
            visible.push(Entry {
                path: parent.join(".machineglass-other-entries"),
                parent: parent.to_path_buf(),
                name: "Other discovered items".into(),
                is_dir: false,
                size: remainder_size,
                aggregate_count: remainder_count as u64,
                aggregate: true,
                unmapped: false,
            });
        }
        if depth == 0
            && let Some(capacity) = self.capacity
            && capacity > total_discovered
        {
            visible.push(Entry {
                path: parent.join(".machineglass-unmapped-space"),
                parent: parent.to_path_buf(),
                name: "Free + not yet mapped".into(),
                is_dir: false,
                size: capacity - total_discovered,
                aggregate_count: 0,
                aggregate: false,
                unmapped: true,
            });
        }
        visible.retain(|entry| entry.size > 0);
        let layout = squarified_layout(&visible, rect);
        for (entry, piece) in visible.iter().zip(layout) {
            let piece = piece.shrink(1.0);
            let fill = if entry.unmapped {
                Color32::from_rgb(29, 39, 51)
            } else if entry.is_dir {
                storage_color(path_color_index(&entry.path))
            } else {
                parent_color
            };
            let painter = ui.painter();
            painter.rect_filled(piece, 2.0, fill);
            painter.rect_stroke(
                piece,
                2.0,
                egui::Stroke::new(1.0_f32, Color32::from_rgb(12, 18, 26)),
                egui::StrokeKind::Inside,
            );
            let response = ui.interact(
                piece,
                egui::Id::new(("storage", &entry.path)),
                egui::Sense::click(),
            );
            if response.hovered() {
                self.hovered = Some(entry.path.clone());
            }
            if response.clicked() && entry.is_dir {
                *self.selected = Some(entry.path.clone());
            }
            if piece.width() > 80.0 && piece.height() > 36.0 {
                let title = if entry.unmapped {
                    format!("{} · {}", entry.name, bytes(entry.size))
                } else if entry.aggregate {
                    format!("{} ({} items)", entry.name, entry.aggregate_count)
                } else {
                    entry.name.clone()
                };
                painter.text(
                    piece.left_top() + egui::vec2(5.0, 4.0),
                    egui::Align2::LEFT_TOP,
                    title,
                    egui::FontId::proportional(12.0),
                    Color32::WHITE,
                );
                if !entry.unmapped {
                    painter.text(
                        piece.left_bottom() + egui::vec2(5.0, -4.0),
                        egui::Align2::LEFT_BOTTOM,
                        bytes(entry.size),
                        egui::FontId::proportional(10.0),
                        Color32::WHITE,
                    );
                }
            } else if !entry.unmapped && piece.width() >= 28.0 && piece.height() >= 70.0 {
                let compact_name: String = entry.name.chars().take(6).collect();
                painter.text(
                    piece.center_top() + egui::vec2(0.0, 4.0),
                    egui::Align2::CENTER_TOP,
                    compact_name,
                    egui::FontId::proportional(9.0),
                    Color32::WHITE,
                );
                painter.text(
                    piece.center_bottom() - egui::vec2(0.0, 4.0),
                    egui::Align2::CENTER_BOTTOM,
                    compact_bytes(entry.size),
                    egui::FontId::proportional(9.0),
                    Color32::WHITE,
                );
            }
            if entry.is_dir && piece.width() > 34.0 && piece.height() > 34.0 {
                let nested = egui::Rect::from_min_max(
                    piece.min + egui::vec2(1.0, 20.0),
                    piece.max - egui::vec2(1.0, 1.0),
                );
                self.draw(ui, &entry.path, nested, depth + 1, fill);
            }
        }
    }
}

fn path_color_index(path: &Path) -> usize {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in path.to_string_lossy().to_ascii_lowercase().bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash as usize
}

fn squarified_layout(entries: &[Entry], bounds: egui::Rect) -> Vec<egui::Rect> {
    let total = entries
        .iter()
        .map(|entry| entry.size.max(1) as f64)
        .sum::<f64>();
    if total <= 0.0 || bounds.is_negative() || entries.is_empty() {
        return Vec::new();
    }
    let area = f64::from(bounds.width()) * f64::from(bounds.height());
    let mut remaining: Vec<(usize, f64)> = entries
        .iter()
        .enumerate()
        .map(|(index, entry)| (index, area * entry.size.max(1) as f64 / total))
        .collect();
    let mut layout = vec![egui::Rect::NOTHING; entries.len()];
    let mut frame = bounds;
    while !remaining.is_empty() && frame.width() > 0.0 && frame.height() > 0.0 {
        let short_side = f64::from(frame.width().min(frame.height()));
        let mut row = Vec::new();
        while let Some(next) = remaining.first().copied() {
            let current_worst = squarify_worst(&row, short_side);
            row.push(next);
            if !remaining.is_empty() && squarify_worst(&row, short_side) > current_worst {
                row.pop();
                break;
            }
            remaining.remove(0);
        }
        if row.is_empty() {
            row.push(remaining.remove(0));
        }
        let row_area = row.iter().map(|(_, item_area)| item_area).sum::<f64>();
        if frame.width() >= frame.height() {
            let strip_width = (row_area / f64::from(frame.height())) as f32;
            let mut y = frame.top();
            for (position, (index, item_area)) in row.iter().enumerate() {
                let height = if position + 1 == row.len() {
                    frame.bottom() - y
                } else {
                    (*item_area / f64::from(strip_width.max(0.001))) as f32
                };
                layout[*index] = egui::Rect::from_min_size(
                    egui::pos2(frame.left(), y),
                    egui::vec2(strip_width.min(frame.width()), height.max(0.0)),
                );
                y += height;
            }
            frame.min.x += strip_width.min(frame.width());
        } else {
            let strip_height = (row_area / f64::from(frame.width())) as f32;
            let mut x = frame.left();
            for (position, (index, item_area)) in row.iter().enumerate() {
                let width = if position + 1 == row.len() {
                    frame.right() - x
                } else {
                    (*item_area / f64::from(strip_height.max(0.001))) as f32
                };
                layout[*index] = egui::Rect::from_min_size(
                    egui::pos2(x, frame.top()),
                    egui::vec2(width.max(0.0), strip_height.min(frame.height())),
                );
                x += width;
            }
            frame.min.y += strip_height.min(frame.height());
        }
    }
    layout
}

fn squarify_worst(row: &[(usize, f64)], short_side: f64) -> f64 {
    if row.is_empty() || short_side <= 0.0 {
        return f64::INFINITY;
    }
    let sum = row.iter().map(|(_, area)| area).sum::<f64>();
    let min = row
        .iter()
        .map(|(_, area)| *area)
        .fold(f64::INFINITY, f64::min);
    let max = row.iter().map(|(_, area)| *area).fold(0.0, f64::max);
    let side_squared = short_side * short_side;
    (side_squared * max / (sum * sum)).max((sum * sum) / (side_squared * min))
}

impl MachineGlass {
    fn new(
        cc: &eframe::CreationContext<'_>,
        capture: Option<PathBuf>,
        page: Page,
        query: String,
    ) -> Self {
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = Color32::from_rgb(16, 23, 32);
        visuals.window_fill = Color32::from_rgb(23, 32, 44);
        visuals.extreme_bg_color = Color32::from_rgb(12, 18, 26);
        visuals.selection.bg_fill = Color32::from_rgb(28, 76, 74);
        cc.egui_ctx.set_visuals(visuals);
        cc.egui_ctx.style_mut(|s| {
            s.spacing.item_spacing = egui::vec2(12.0, 9.0);
            s.spacing.button_padding = egui::vec2(14.0, 8.0);
        });
        let config = Config::load();
        config::log("Starting MachineGlass 0.1.0; direct Win32 provider");
        let tray = Tray::new(cc)
            .map_err(|e| {
                config::log(&format!(
                    "Tray unavailable; closing window exits normally: {e}"
                ))
            })
            .ok();
        Self {
            engine: Engine::start(config.clone(), cc.egui_ctx.clone()),
            tray,
            config,
            page,
            query,
            sort: Sort::Cpu,
            descending: true,
            selected: None,
            capture,
            capture_requested: false,
            quit_observed: false,
            always_on_top: false,
            drives: storage::local_drives(),
            drive_index: None,
            storage_session: None,
            storage_root: None,
            storage_selected: None,
            storage_hovered: None,
        }
    }

    fn start_storage_scan(&mut self) {
        let Some(drive) = self.drive_index.and_then(|i| self.drives.get(i)).cloned() else {
            return;
        };
        self.storage_session = None; // Cancels the prior worker without blocking egui.
        self.storage_root = Some(drive.root.clone());
        self.storage_selected = Some(drive.root.clone());
        self.storage_session = Some(StorageSession::start(&drive));
    }

    fn storage(&mut self, ui: &mut egui::Ui) {
        ui.heading("Storage");
        ui.label("Logical file sizes; small files grouped. The remaining area is free space plus items not yet mapped.");
        let mut changed = false;
        ui.horizontal_wrapped(|ui| {
            ui.label("Drive");
            let selected_text = self
                .drive_index
                .and_then(|i| self.drives.get(i))
                .map(|d| d.label.as_str())
                .unwrap_or("Select a local drive");
            egui::ComboBox::from_id_salt("storage_drive")
                .selected_text(selected_text)
                .show_ui(ui, |ui| {
                    for (index, drive) in self.drives.iter().enumerate() {
                        if ui
                            .selectable_value(&mut self.drive_index, Some(index), &drive.label)
                            .clicked()
                        {
                            changed = true;
                        }
                    }
                });
            if self
                .storage_session
                .as_ref()
                .is_some_and(|session| session.tree().is_scanning())
                && ui.button("Cancel scan").clicked()
                && let Some(scan) = &self.storage_session
            {
                scan.cancel();
            }
        });
        if changed {
            self.start_storage_scan();
        }
        if self.drives.is_empty() {
            ui.colored_label(MUTED, "No fixed or removable local drives are available.");
            return;
        }
        if self.storage_root.is_none() {
            ui.label("Choose a drive to begin.");
            return;
        }
        if let Some(session) = &mut self.storage_session {
            session.poll();
        }
        let Some(session) = &self.storage_session else {
            return;
        };
        let tree = session.tree();
        let entries = tree.entries();
        let root = self.storage_root.clone().unwrap_or_default();
        ui.horizontal_wrapped(|ui| {
            if let Some(selected) = &self.storage_selected {
                if selected != &root && ui.button("← Parent").clicked() {
                    self.storage_selected = selected
                        .parent()
                        .map(Path::to_path_buf)
                        .or(Some(root.clone()));
                }
                ui.label(
                    self.storage_selected
                        .as_deref()
                        .unwrap_or(&root)
                        .display()
                        .to_string(),
                );
            }
            let action_path = self
                .storage_hovered
                .as_ref()
                .and_then(|path| entries.get(path))
                .filter(|entry| !entry.aggregate && !entry.unmapped)
                .map(|entry| entry.path.clone())
                .or_else(|| self.storage_selected.clone());
            if let Some(path) = action_path
                && ui.button("Open in Explorer").clicked()
                && let Err(error) = std::process::Command::new("explorer.exe").arg(path).spawn()
            {
                config::log(&format!(
                    "Could not open storage location in Explorer: {error}"
                ));
            }
            let count = tree.discovered_count();
            ui.label(format!("{count} discovered entries"));
            if tree.inaccessible_count() > 0 {
                let response = ui.label(format!("{} unreadable", tree.inaccessible_count()));
                response.on_hover_ui(|ui| {
                    ui.heading("Unreadable items");
                    ui.label("The scanner could not complete this filesystem operation:");
                    let issues = tree.inaccessible_issues();
                    for issue in issues.iter().take(8) {
                        ui.separator();
                        ui.label(issue.path.display().to_string());
                        ui.label(format!("{}: {}", issue.operation, issue.reason));
                    }
                    if issues.len() > 8 {
                        ui.label(format!("{} more unreadable items", issues.len() - 8));
                    }
                });
            }
            if tree.is_scanning() {
                ui.label(if tree.has_cached_snapshot() {
                    "Cached map · verifying…"
                } else {
                    "Scanning…"
                });
            } else if let Some(error) = tree.error() {
                ui.colored_label(Color32::YELLOW, format!("Scan could not start: {error}"));
            } else if let Some(cancelled) = tree.finished() {
                ui.label(if cancelled {
                    "Scan cancelled"
                } else if tree.inaccessible_count() > 0 {
                    "Scan complete · some locations unreadable"
                } else if tree.changed() == Some(false) {
                    "Scan complete · no changes"
                } else if tree.changed() == Some(true) {
                    "Scan complete · map updated"
                } else {
                    "Scan complete"
                });
            }
        });
        let selected = self.storage_selected.clone().unwrap_or(root.clone());
        let size = tree.entry(&selected).map(|e| e.size).unwrap_or(0);
        ui.label(format!(
            "{} · {} discovered{}",
            selected.display(),
            bytes(size),
            if selected == root {
                let total = tree
                    .total_bytes()
                    .map(bytes)
                    .unwrap_or_else(|| "unavailable".into());
                let free = tree
                    .free_bytes()
                    .map(bytes)
                    .unwrap_or_else(|| "unavailable".into());
                format!(" · total {total} · free {free}")
            } else {
                String::new()
            }
        ));
        let available = ui.available_size().max(egui::vec2(1.0, 1.0));
        let (rect, _) = ui.allocate_exact_size(available, egui::Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 4.0, Color32::from_rgb(12, 18, 26));
        self.storage_hovered = None;
        let has_children = tree
            .children()
            .get(&selected)
            .is_some_and(|children| !children.is_empty());
        let capacity = if selected == root {
            tree.total_bytes()
        } else {
            None
        };
        if selected == root && capacity.is_none() {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "Drive capacity unavailable; treemap scale unavailable",
                egui::FontId::proportional(14.0),
                MUTED,
            );
        } else if !has_children {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "Waiting for directory entries…",
                egui::FontId::proportional(14.0),
                MUTED,
            );
        } else {
            let mut drawing = StorageDraw {
                children: tree.children(),
                entries,
                hovered: None,
                selected: &mut self.storage_selected,
                capacity,
            };
            drawing.draw(
                ui,
                &selected,
                rect,
                0,
                storage_color(path_color_index(&selected)),
            );
            self.storage_hovered = drawing.hovered;
            if let Some(path) = &self.storage_hovered
                && let Some(entry) = entries.get(path)
            {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                if entry.unmapped {
                    ui.label(format!("{} · {}", entry.name, bytes(entry.size)));
                } else {
                    ui.label(format!(
                        "{} · {}{}",
                        entry.path.display(),
                        bytes(entry.size),
                        if entry.is_dir {
                            " · directory"
                        } else {
                            " · file"
                        }
                    ));
                }
            }
        }
    }

    fn overview(
        &mut self,
        ui: &mut egui::Ui,
        s: &Snapshot,
        history: &[HistoryPoint],
        gpu_history: &[GpuHistoryPoint],
    ) {
        ui.heading("System overview");
        ui.label(RichText::new("See what your machine is actually doing.").color(MUTED));
        ui.add_space(12.0);
        ui.columns(2, |cols| {
            card(
                &mut cols[0],
                "CPU",
                &percent(&s.cpu_percent),
                ACCENT,
                |ui| {
                    ui.label(format!("{} logical processors", s.logical_processors));
                    chart(ui, history, true, ACCENT, self.config.sample_interval_ms);
                },
            );
            let value = s
                .memory
                .value()
                .map(|m| format!("{} / {}", bytes(m.used_bytes()), bytes(m.total_bytes)))
                .unwrap_or_else(|| "Unavailable".into());
            card(
                &mut cols[1],
                "PHYSICAL MEMORY · USED / TOTAL",
                &value,
                BLUE,
                |ui| {
                    if let Some(m) = s.memory.value() {
                        ui.label(format!(
                            "{} available · {:.1}% used",
                            bytes(m.available_bytes),
                            m.percent()
                        ));
                    } else {
                        ui.label(s.memory.reason());
                    }
                    chart(ui, history, false, BLUE, self.config.sample_interval_ms);
                },
            );
        });
        self.gpus(ui, s, gpu_history);
        ui.add_space(14.0);
        let days = s.uptime_seconds / 86400;
        let hours = s.uptime_seconds / 3600 % 24;
        let mins = s.uptime_seconds / 60 % 60;
        ui.horizontal(|ui| {
            ui.label(format!("UPTIME   {days}d {hours:02}h {mins:02}m"));
            ui.separator();
            ui.label(format!("COLLECTION   {:.1} ms", s.collection_ms));
            ui.separator();
            ui.label(format!(
                "PROCESSES   {}",
                s.processes
                    .value()
                    .map(|rows| rows.len().to_string())
                    .unwrap_or_else(|| "Unavailable".into())
            ));
        });
        ui.add_space(18.0);
        ui.heading("Processes using CPU");
        ui.label(RichText::new("CPU is normalized to total machine capacity. Click an accessible process for details.").color(MUTED));
        if let Some(rows) = s.processes.value() {
            let mut top: Vec<_> = rows
                .iter()
                .filter(|p| p.cpu_percent.value().is_some())
                .collect();
            top.sort_by(|a, b| {
                b.cpu_percent
                    .value()
                    .unwrap()
                    .total_cmp(a.cpu_percent.value().unwrap())
            });
            egui::Grid::new("top")
                .num_columns(4)
                .striped(true)
                .min_col_width(120.0)
                .show(ui, |ui| {
                    for label in ["Process", "PID", "CPU", "Working set"] {
                        ui.strong(label);
                    }
                    ui.end_row();
                    for p in top.iter().take(7) {
                        if ui
                            .selectable_label(self.selected == p.identity, &p.name)
                            .clicked()
                        {
                            self.selected = p.identity;
                        }
                        ui.monospace(p.pid.to_string());
                        ui.monospace(percent(&p.cpu_percent));
                        ui.monospace(memory_value(&p.working_set_bytes));
                        ui.end_row();
                    }
                });
        } else {
            ui.colored_label(Color32::YELLOW, s.processes.reason());
        }
        ui.add_space(20.0);
        ui.separator();
        ui.label(RichText::new("Storage / network / additional sensors: collectors not implemented in this milestone.").color(MUTED));
        ui.label(RichText::new("Core history stays in memory for 30 minutes by default. Trace replay and process history arrive later.").color(MUTED));
    }

    fn gpus(&self, ui: &mut egui::Ui, s: &Snapshot, history: &[GpuHistoryPoint]) {
        ui.add_space(12.0);
        let Some(gpus) = s.gpus.value() else {
            ui.heading("GPU");
            ui.label(s.gpus.reason());
            return;
        };
        if gpus.is_empty() {
            ui.label("No hardware GPU adapters detected");
        }
        for gpu in gpus {
            ui.columns(2, |cols| {
                gpu_memory_card(&mut cols[0], "DEDICATED GPU MEMORY", gpu, history, false);
                gpu_memory_card(&mut cols[1], "SHARED GPU MEMORY", gpu, history, true);
            });
            ui.add_space(6.0);
            ui.heading(&gpu.name);
            let dominant = gpu
                .engines
                .value()
                .and_then(|engines| busiest_engine(engines, |_| true));
            let overall = gpu.activity(|_| true);
            let dominant_label = if overall.value().is_some() {
                dominant.map(|e| e.label.as_str()).unwrap_or("Unavailable")
            } else {
                overall.reason()
            };
            ui.horizontal_wrapped(|ui| {
                ui.label(format!("Dominant: {dominant_label}"));
                ui.separator();
                ui.label(format!(
                    "Temperature: {}",
                    gpu.temperature_c
                        .value()
                        .map(|t| format!("{t:.1} °C"))
                        .unwrap_or_else(|| "Unavailable".into())
                ));
            });
            ui.columns(3, |cols| {
                for (col, title, filter) in [
                    (
                        0,
                        "OVERALL GPU ACTIVITY",
                        (|_: &GpuEngine| true) as fn(&GpuEngine) -> bool,
                    ),
                    (
                        1,
                        "COMPUTE · BUSIEST",
                        compute_engine as fn(&GpuEngine) -> bool,
                    ),
                    (2, "3D ENGINE", three_d_engine as fn(&GpuEngine) -> bool),
                ] {
                    let activity = gpu.activity(filter);
                    let label = gpu
                        .engines
                        .value()
                        .and_then(|es| busiest_engine(es, filter))
                        .map(|e| e.label.as_str());
                    card(&mut cols[col], title, &percent(&activity), ACCENT, |ui| {
                        ui.label(label.unwrap_or_else(|| activity.reason()));
                        let points: Vec<_> = history
                            .iter()
                            .map(|p| HistoryPoint {
                                unix_ms: p.unix_ms,
                                cpu_percent: p
                                    .adapters
                                    .iter()
                                    .find(|g| g.id == gpu.id)
                                    .and_then(|g| g.activity(filter).value().copied()),
                                memory_percent: None,
                            })
                            .collect();
                        chart_height(ui, &points, true, ACCENT, 1000, 24.0);
                    });
                }
            });
            egui::CollapsingHeader::new("All Windows engines")
                .id_salt(&gpu.id)
                .show(ui, |ui| {
                    if let Some(engines) = gpu.engines.value() {
                        for engine in engines {
                            ui.label(format!(
                                "{} · {} · {}",
                                engine.label,
                                engine.id,
                                percent(&engine.percent)
                            ));
                        }
                    } else {
                        ui.label(gpu.engines.reason());
                    }
                });
        }
    }
    fn processes(&mut self, ui: &mut egui::Ui, s: &Snapshot) {
        ui.heading("Process inspection");
        ui.horizontal_wrapped(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.query)
                    .hint_text("Search name, PID or executable path")
                    .desired_width(330.0),
            );
            ui.label("Sort by");
            egui::ComboBox::from_id_salt("sort")
                .selected_text(match self.sort {
                    Sort::Cpu => "CPU",
                    Sort::Memory => "Memory",
                    Sort::Name => "Name",
                    Sort::Pid => "PID",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.sort, Sort::Cpu, "CPU");
                    ui.selectable_value(&mut self.sort, Sort::Memory, "Memory");
                    ui.selectable_value(&mut self.sort, Sort::Name, "Name");
                    ui.selectable_value(&mut self.sort, Sort::Pid, "PID");
                });
            ui.checkbox(&mut self.descending, "Descending");
        });
        ui.label(RichText::new("Read only · unavailable values retain the Windows error · parent PID is an observed relationship, not proof of current parent identity").color(MUTED));
        let Some(processes) = s.processes.value() else {
            ui.label(s.processes.reason());
            return;
        };
        let q = self.query.to_lowercase();
        let mut rows: Vec<_> = processes
            .iter()
            .filter(|p| {
                q.is_empty()
                    || p.name.to_lowercase().contains(&q)
                    || p.pid.to_string().contains(&q)
                    || p.path
                        .value()
                        .is_some_and(|path| path.to_lowercase().contains(&q))
            })
            .collect();
        rows.sort_by(|a, b| {
            // Missing measurements always sort last, in either direction.
            let cmp = match self.sort {
                Sort::Cpu => match (a.cpu_percent.value(), b.cpu_percent.value()) {
                    (None, Some(_)) => return std::cmp::Ordering::Greater,
                    (Some(_), None) => return std::cmp::Ordering::Less,
                    (Some(a), Some(b)) => a.total_cmp(b),
                    _ => a.pid.cmp(&b.pid),
                },
                Sort::Memory => match (a.working_set_bytes.value(), b.working_set_bytes.value()) {
                    (None, Some(_)) => return std::cmp::Ordering::Greater,
                    (Some(_), None) => return std::cmp::Ordering::Less,
                    (Some(a), Some(b)) => a.cmp(b),
                    _ => a.pid.cmp(&b.pid),
                },
                Sort::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                Sort::Pid => a.pid.cmp(&b.pid),
            };
            if self.descending { cmp.reverse() } else { cmp }
        });
        ui.label(format!("{} of {} processes", rows.len(), processes.len()));
        ui.separator();
        let name_width = (ui.available_width() - 438.0).clamp(180.0, 280.0);
        let row_ui =
            |ui: &mut egui::Ui, p: Option<&Process>, selected: &mut Option<ProcessIdentity>| {
                ui.horizontal(|ui| {
                    ui.allocate_ui_with_layout(
                        egui::vec2(name_width, 22.0),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            ui.set_min_width(name_width);
                            if let Some(p) = p {
                                let response = ui.add_sized(
                                    [name_width, 22.0],
                                    egui::Button::selectable(
                                        p.identity.is_some() && *selected == p.identity,
                                        &p.name,
                                    )
                                    .truncate(),
                                );
                                if response.clicked() {
                                    *selected = p.identity;
                                }
                                response.on_hover_text(
                                    p.path
                                        .value()
                                        .map(String::as_str)
                                        .unwrap_or_else(|| p.path.reason()),
                                );
                            } else {
                                ui.strong("Process");
                            }
                        },
                    );
                    for (width, value) in [
                        (75.0, p.map_or("PID".into(), |p| p.pid.to_string())),
                        (
                            90.0,
                            p.map_or("Parent PID".into(), |p| p.parent_pid.to_string()),
                        ),
                        (100.0, p.map_or("CPU".into(), |p| percent(&p.cpu_percent))),
                        (
                            125.0,
                            p.map_or("Working set".into(), |p| memory_value(&p.working_set_bytes)),
                        ),
                    ] {
                        ui.allocate_ui(egui::vec2(width, 22.0), |ui| {
                            ui.set_min_width(width);
                            let response = ui.monospace(value);
                            if let Some(p) = p {
                                if width == 100.0 {
                                    response.on_hover_text(p.cpu_percent.reason());
                                } else if width == 125.0 {
                                    response.on_hover_text(p.working_set_bytes.reason());
                                }
                            }
                        });
                    }
                });
            };
        row_ui(ui, None, &mut self.selected);
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show_rows(ui, 31.0, rows.len(), |ui, range| {
                for i in range {
                    row_ui(ui, Some(rows[i]), &mut self.selected);
                }
            });
    }

    fn instrument(&self, ui: &mut egui::Ui, s: &Snapshot, history: &[HistoryPoint]) {
        ui.heading("The instrument");
        ui.label("MachineGlass 0.1.0 · Milestone 1 technical proof of concept");
        ui.add_space(16.0);
        ui.heading("MachineGlass monitoring itself");
        if let Some(p) = s
            .processes
            .value()
            .and_then(|rows| rows.iter().find(|p| p.pid == std::process::id()))
        {
            ui.label(format!(
                "PID {} · CPU {} · working set {}",
                p.pid,
                percent(&p.cpu_percent),
                memory_value(&p.working_set_bytes)
            ));
        }
        ui.label(format!(
            "Last acquisition: {:.2} ms · configured interval: {} ms",
            s.collection_ms, self.config.sample_interval_ms
        ));
        ui.label(format!(
            "History: {} / {} core snapshots · configured retention: {} minutes",
            history.len(),
            self.config.history_capacity(),
            self.config.history_seconds / 60
        ));
        ui.add_space(16.0);
        ui.heading("Measurement sources");
        for line in [
            "CPU: GetSystemTimes, kernel + user − idle deltas",
            "Physical RAM: GlobalMemoryStatusEx",
            "Processes: Toolhelp snapshot, GetProcessTimes, K32GetProcessMemoryInfo",
            "Executable path: QueryFullProcessImageNameW",
            "Uptime: GetTickCount64 (includes sleep; Windows fast startup can retain uptime)",
            "Identity: PID + Windows creation timestamp; inaccessible identity is not invented",
        ] {
            ui.label(line);
        }
        ui.add_space(16.0);
        ui.heading("Capabilities");
        ui.label("Live: CPU, RAM, processes, uptime, WDDM GPU engines and adapter memory.");
        ui.label(
            "Not implemented: temperature, storage, network, services, scans, trace persistence.",
        );
        ui.label(
            "No administrator requirement, telemetry upload, process control or file cleanup.",
        );
        ui.add_space(16.0);
        ui.heading("Local application data");
        ui.label(config::data_dir().display().to_string());
        ui.label("config.json: sampling / retention. machineglass.log: bounded internal errors and lifecycle.");
        ui.label("Restart after editing configuration. Invalid configuration is preserved and defaults are used.");
    }

    fn detail(&mut self, ctx: &egui::Context, s: &Snapshot) {
        let Some(id) = self.selected else {
            return;
        };
        let mut open = true;
        egui::Window::new("Process details").open(&mut open).default_width(620.0).show(ctx, |ui| {
            let p = s.processes.value().and_then(|rows| rows.iter().find(|p| p.identity == Some(id)));
            if let Some(p) = p {
                ui.heading(&p.name);
                ui.label(format!("PID {} · parent PID {}", p.pid, p.parent_pid));
                ui.label(format!("CPU {} · working set {}", percent(&p.cpu_percent), memory_value(&p.working_set_bytes)));
                ui.label(format!("Creation identity: {} Windows FILETIME ticks", id.creation_ticks));
                ui.label("Executable path");
                ui.label(p.path.value().map(String::as_str).unwrap_or_else(|| p.path.reason()));
                ui.label(format!("CPU status: {}", p.cpu_percent.reason()));
                ui.label(format!("Memory status: {}", p.working_set_bytes.reason()));
            } else { ui.label("This process exited or is no longer accessible. Its PID has not been matched to a different process."); }
        });
        if !open {
            self.selected = None;
        }
    }
}

fn gpu_memory_card(
    ui: &mut egui::Ui,
    title: &str,
    gpu: &Gpu,
    history: &[GpuHistoryPoint],
    shared: bool,
) {
    let (used, total) = if shared {
        (&gpu.shared_used_bytes, &gpu.shared_limit_bytes)
    } else {
        (&gpu.dedicated_used_bytes, &gpu.dedicated_total_bytes)
    };
    let value = format!("{} / {}", memory_value(used), memory_value(total));
    card(ui, title, &value, BLUE, |ui| {
        let available = match (used.value(), total.value()) {
            (Some(u), Some(t)) if u <= t => format!("{} available", bytes(t - u)),
            _ => "Available: unavailable".into(),
        };
        ui.label(available).on_hover_text(&gpu.name);
        let points: Vec<_> = history
            .iter()
            .map(|p| {
                let value = p.adapters.iter().find(|g| g.id == gpu.id).and_then(|g| {
                    let (used, total) = if shared {
                        (&g.shared_used_bytes, &g.shared_limit_bytes)
                    } else {
                        (&g.dedicated_used_bytes, &g.dedicated_total_bytes)
                    };
                    match (used.value(), total.value()) {
                        (Some(u), Some(t)) if *t > 0 => Some(*u as f64 / *t as f64 * 100.0),
                        _ => None,
                    }
                });
                HistoryPoint {
                    unix_ms: p.unix_ms,
                    cpu_percent: value,
                    memory_percent: None,
                }
            })
            .collect();
        chart_height(ui, &points, true, BLUE, 1000, 44.0);
    });
}

fn card(
    ui: &mut egui::Ui,
    title: &str,
    value: &str,
    color: Color32,
    body: impl FnOnce(&mut egui::Ui),
) {
    egui::Frame::new()
        .fill(Color32::from_rgb(23, 33, 46))
        .inner_margin(8.0)
        .corner_radius(7.0)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 4.0;
            ui.label(RichText::new(title).size(11.0).color(MUTED));
            ui.label(RichText::new(value).size(11.0).color(color));
            body(ui);
        });
}

fn chart(ui: &mut egui::Ui, history: &[HistoryPoint], cpu: bool, color: Color32, interval_ms: u64) {
    chart_height(ui, history, cpu, color, interval_ms, 44.0);
}
fn chart_height(
    ui: &mut egui::Ui,
    history: &[HistoryPoint],
    cpu: bool,
    color: Color32,
    interval_ms: u64,
    height: f32,
) {
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), height),
        egui::Sense::hover(),
    );
    let painter = ui.painter_at(rect);
    for i in 0..=4 {
        let y = rect.bottom() - rect.height() * i as f32 / 4.0;
        painter.line_segment(
            [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
            egui::Stroke::new(1.0_f32, Color32::from_rgb(40, 53, 68)),
        );
    }
    if let Some(last) = history.last() {
        let start = last.unix_ms.saturating_sub(90_000);
        let mut previous: Option<(u64, egui::Pos2)> = None;
        for p in history.iter().filter(|p| p.unix_ms >= start) {
            let value = if cpu { p.cpu_percent } else { p.memory_percent };
            if let Some(v) = value {
                let point = egui::pos2(
                    rect.left() + (p.unix_ms - start) as f32 / 90000.0 * rect.width(),
                    rect.bottom() - (v / 100.0).clamp(0.0, 1.0) as f32 * rect.height(),
                );
                if let Some((time, old)) = previous
                    && p.unix_ms >= time
                    && p.unix_ms.saturating_sub(time) <= interval_ms * 3
                {
                    painter.line_segment([old, point], egui::Stroke::new(2.0_f32, color));
                }
                painter.circle_filled(point, 1.6, color);
                previous = Some((p.unix_ms, point));
            } else {
                previous = None;
            }
        }
    }
    response.on_hover_text("Last 90 seconds · 0–100% scale · gaps mean unavailable readings");
}

impl eframe::App for MachineGlass {
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        config::log("Window framework entered on_exit; renderer cleanup follows");
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.quit_observed && self.tray.as_ref().is_some_and(Tray::quitting) {
            self.quit_observed = true;
            config::log("UI observed tray Quit request");
        }
        if ctx.input(|i| i.viewport().close_requested()) {
            config::log(if self.tray.as_ref().is_some_and(Tray::quitting) {
                "UI received close event; tray Quit permits shutdown"
            } else {
                "UI received close event without tray Quit"
            });
        }
        if ctx.input(|i| i.viewport().close_requested())
            && self.tray.as_ref().is_some_and(|tray| !tray.quitting())
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            config::log("Window closed to tray; collection continues");
        }
        let (latest, history, gpu_history, age, error) = {
            let state = self.engine.state.lock().unwrap_or_else(|e| e.into_inner());
            (
                state.latest.clone(),
                state.history.iter().cloned().collect::<Vec<_>>(),
                state.gpu_history.iter().cloned().collect::<Vec<_>>(),
                state.last_arrival.map(|i| i.elapsed()),
                state.error.clone(),
            )
        };
        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("MACHINEGLASS")
                        .size(22.0)
                        .strong()
                        .color(ACCENT),
                );
                ui.label(
                    RichText::new("SYSTEM OBSERVABILITY")
                        .size(11.0)
                        .color(MUTED),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let fresh = error.is_none()
                        && age.is_some_and(|a| {
                            a.as_millis() < (self.config.sample_interval_ms * 3) as u128
                        });
                    ui.colored_label(
                        if fresh { ACCENT } else { Color32::YELLOW },
                        if fresh {
                            "● LIVE · READ ONLY"
                        } else {
                            "● WAITING / STALE"
                        },
                    );
                });
            });
        });
        egui::SidePanel::left("navigation")
            .default_width(136.0)
            .min_width(96.0)
            .max_width(180.0)
            .resizable(true)
            .show(ctx, |ui| {
                ui.add_space(20.0);
                for (page, label) in [
                    (Page::Overview, "System overview"),
                    (Page::Processes, "Processes"),
                    (Page::Instrument, "The instrument"),
                    (Page::Storage, "Storage"),
                ] {
                    if ui
                        .add_sized(
                            [ui.available_width(), 36.0],
                            egui::Button::new(label).selected(self.page == page),
                        )
                        .clicked()
                    {
                        self.page = page;
                    }
                }
                ui.add_space(15.0);
                if ui
                    .checkbox(&mut self.always_on_top, "Always on top")
                    .on_hover_text("Keep MachineGlass above other windows while they have focus. Applies for this session.")
                    .changed()
                {
                    ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(
                        if self.always_on_top {
                            egui::WindowLevel::AlwaysOnTop
                        } else {
                            egui::WindowLevel::Normal
                        },
                    ));
                }
                ui.add_space(10.0);
                ui.separator();
                ui.label(
                    RichText::new("NATIVE WINDOWS\nOFFLINE\nNO ELEVATION")
                        .size(11.0)
                        .color(MUTED),
                );
                ui.add_space(15.0);
                ui.label(
                    RichText::new("v0.1.0\nTechnical skeleton")
                        .size(11.0)
                        .color(MUTED),
                );
            });
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("SOURCE  Win32").color(MUTED));
                ui.separator();
                ui.label(format!(
                    "Sample every {} ms",
                    self.config.sample_interval_ms
                ));
                if let Some(a) = age {
                    ui.label(format!("Updated {:.1}s ago", a.as_secs_f64()));
                }
                if let Some(e) = &error {
                    ui.colored_label(Color32::YELLOW, e);
                }
                if self.tray.is_none() {
                    ui.colored_label(Color32::YELLOW, "Tray unavailable · closing exits");
                }
            });
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(12.0);
            if let Some(s) = &latest {
                if self.page == Page::Processes {
                    self.processes(ui, s);
                } else {
                    egui::ScrollArea::vertical().show(ui, |ui| match self.page {
                        Page::Overview => self.overview(ui, s, &history, &gpu_history),
                        Page::Instrument => self.instrument(ui, s, &history),
                        Page::Storage => self.storage(ui),
                        Page::Processes => (),
                    });
                }
            } else {
                ui.spinner();
                ui.label("Acquiring the first Windows snapshot…");
            }
        });
        if let Some(s) = &latest {
            self.detail(ctx, s);
        }
        // Optional local UI verification: capture the actual native framebuffer.
        if self.capture.is_some() && !self.capture_requested && history.len() >= 4 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            self.capture_requested = true;
        }
        if let Some(path) = &self.capture {
            ctx.input(|i| {
                for event in &i.events {
                    if let egui::Event::Screenshot { image, .. } = event
                        && let Err(e) = save_bmp(path, image)
                    {
                        config::log(&format!("Screenshot failed: {e}"));
                    }
                }
            });
        }
        // Repainting only on samples/input; periodic check exposes a stalled provider.
        ctx.request_repaint_after(
            if self
                .storage_session
                .as_ref()
                .is_some_and(|session| session.tree().is_scanning())
            {
                Duration::from_millis(150)
            } else {
                Duration::from_secs(1)
            },
        );
    }
}

impl Drop for MachineGlass {
    fn drop(&mut self) {
        config::log("App dropping; collector and tray cleanup follow");
    }
}

fn save_bmp(path: &std::path::Path, image: &egui::ColorImage) -> std::io::Result<()> {
    use std::io::Write;
    let [w, h] = image.size;
    let pixel_bytes = w * h * 4;
    let mut f = std::fs::File::create(path)?;
    f.write_all(b"BM")?;
    f.write_all(&((54 + pixel_bytes) as u32).to_le_bytes())?;
    f.write_all(&[0; 4])?;
    f.write_all(&54u32.to_le_bytes())?;
    f.write_all(&40u32.to_le_bytes())?;
    f.write_all(&(w as i32).to_le_bytes())?;
    f.write_all(&(-(h as i32)).to_le_bytes())?;
    f.write_all(&1u16.to_le_bytes())?;
    f.write_all(&32u16.to_le_bytes())?;
    f.write_all(&[0; 24])?;
    let mut pixels = Vec::with_capacity(pixel_bytes);
    for p in &image.pixels {
        pixels.extend_from_slice(&[p.b(), p.g(), p.r(), 255]);
    }
    f.write_all(&pixels)
}

fn main() -> eframe::Result {
    std::panic::set_hook(Box::new(|info| config::log(&format!("Panic: {info}"))));
    let args: Vec<_> = std::env::args_os().collect();
    let capture = args
        .windows(2)
        .find(|w| w[0] == "--capture")
        .map(|w| PathBuf::from(&w[1]));
    let page = args
        .windows(2)
        .find(|w| w[0] == "--view")
        .map(|w| w[1].to_string_lossy())
        .map_or(Page::Overview, |v| match v.as_ref() {
            "processes" => Page::Processes,
            "instrument" => Page::Instrument,
            "storage" => Page::Storage,
            _ => Page::Overview,
        });
    let query = args
        .windows(2)
        .find(|w| w[0] == "--search")
        .map(|w| w[1].to_string_lossy().into_owned())
        .unwrap_or_default();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("MachineGlass")
            .with_icon(tray::window_icon())
            .with_inner_size([944.0, 720.0])
            .with_min_inner_size([640.0, 480.0]),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    let result = eframe::run_native(
        "MachineGlass",
        options,
        Box::new(move |cc| Ok(Box::new(MachineGlass::new(cc, capture, page, query)))),
    );
    if let Err(e) = &result {
        config::log(&format!("Native window failed: {e}"));
    }
    config::log("Native event loop returned; main exiting");
    result
}

#[cfg(test)]
mod storage_layout_tests {
    use super::*;

    #[test]
    fn squarified_layout_preserves_area_weights_and_bounds() {
        let entries = [500, 300, 200]
            .into_iter()
            .enumerate()
            .map(|(index, size)| Entry {
                path: PathBuf::from(format!("F:\\{index}")),
                parent: PathBuf::from("F:\\"),
                name: index.to_string(),
                is_dir: false,
                size,
                aggregate_count: 0,
                aggregate: false,
                unmapped: false,
            })
            .collect::<Vec<_>>();
        let bounds = egui::Rect::from_min_size(egui::pos2(10.0, 20.0), egui::vec2(200.0, 100.0));
        let pieces = squarified_layout(&entries, bounds);
        assert_eq!(pieces.len(), entries.len());
        for piece in &pieces {
            assert!(piece.is_positive());
            assert!(bounds.contains_rect(*piece));
        }
        let areas: Vec<_> = pieces
            .iter()
            .map(|piece| f64::from(piece.width()) * f64::from(piece.height()))
            .collect();
        assert!((areas[0] / areas[1] - 500.0 / 300.0).abs() < 0.02);
        assert!((areas[1] / areas[2] - 300.0 / 200.0).abs() < 0.02);
    }
}
