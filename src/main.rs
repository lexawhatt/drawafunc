use std::fs;
use std::path::Path;

use eframe::egui::{
    self, Align2, CentralPanel, Color32, Context, FontId, Key, PointerButton, Pos2, Rect, RichText,
    Sense, SidePanel, Slider, Stroke as EguiStroke, TopBottomPanel, Ui, Vec2,
};
use serde::{Deserialize, Serialize};

const PROJECT_FILE: &str = "drawafunc_project.json";
const MIN_ZOOM: f32 = 8.0;
const MAX_ZOOM: f32 = 260.0;
const INITIAL_ZOOM: f32 = 42.0;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Drawafunc")
            .with_inner_size([1180.0, 760.0])
            .with_min_inner_size([900.0, 560.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Drawafunc",
        options,
        Box::new(|cc| Ok(Box::new(DrawafuncApp::new(cc)))),
    )
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
struct Point {
    x: f32,
    y: f32,
}

impl Point {
    fn distance(self, other: Self) -> f32 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
    }

    fn lerp(self, other: Self, t: f32) -> Self {
        Self {
            x: self.x + (other.x - self.x) * t,
            y: self.y + (other.y - self.y) * t,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct DrawStroke {
    points: Vec<Point>,
    color: [u8; 4],
    width: f32,
}

impl DrawStroke {
    fn new(color: Color32, width: f32) -> Self {
        Self {
            points: Vec::new(),
            color: [color.r(), color.g(), color.b(), color.a()],
            width,
        }
    }

    fn color32(&self) -> Color32 {
        Color32::from_rgba_unmultiplied(self.color[0], self.color[1], self.color[2], self.color[3])
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Project {
    version: u32,
    strokes: Vec<DrawStroke>,
}

impl Project {
    fn from_strokes(strokes: &[DrawStroke]) -> Self {
        Self {
            version: 1,
            strokes: strokes.to_vec(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tool {
    Pencil,
    Eraser,
    Pan,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ThemeChoice {
    System,
    Light,
    Dark,
}

struct DrawafuncApp {
    strokes: Vec<DrawStroke>,
    redo_stack: Vec<DrawStroke>,
    current_stroke: Option<DrawStroke>,
    tool: Tool,
    theme: ThemeChoice,
    zoom: f32,
    pan: Vec2,
    stroke_width: f32,
    simplify_tolerance: f32,
    generated_preview: Vec<Vec<Point>>,
    generated_text: String,
    status: String,
    last_saved_hash: u64,
    dirty: bool,
    show_original: bool,
    show_generated: bool,
    show_grid: bool,
}

impl DrawafuncApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_theme(egui::ThemePreference::System);

        let strokes = Vec::new();
        let last_saved_hash = project_hash(&strokes);

        Self {
            strokes,
            redo_stack: Vec::new(),
            current_stroke: None,
            tool: Tool::Pencil,
            theme: ThemeChoice::System,
            zoom: INITIAL_ZOOM,
            pan: Vec2::ZERO,
            stroke_width: 2.0,
            simplify_tolerance: 0.08,
            generated_preview: Vec::new(),
            generated_text: String::new(),
            status: "Draw on the canvas, then press Generate.".to_owned(),
            last_saved_hash,
            dirty: false,
            show_original: true,
            show_generated: true,
            show_grid: true,
        }
    }

    fn set_theme(&mut self, ctx: &Context, theme: ThemeChoice) {
        self.theme = theme;
        match theme {
            ThemeChoice::System => ctx.set_theme(egui::ThemePreference::System),
            ThemeChoice::Light => ctx.set_theme(egui::ThemePreference::Light),
            ThemeChoice::Dark => ctx.set_theme(egui::ThemePreference::Dark),
        }
    }

    fn screen_to_math(&self, pos: Pos2, rect: Rect) -> Point {
        let origin = rect.center() + self.pan;
        Point {
            x: (pos.x - origin.x) / self.zoom,
            y: -(pos.y - origin.y) / self.zoom,
        }
    }

    fn math_to_screen(&self, point: Point, rect: Rect) -> Pos2 {
        let origin = rect.center() + self.pan;
        Pos2::new(
            origin.x + point.x * self.zoom,
            origin.y - point.y * self.zoom,
        )
    }

    fn push_current_point(&mut self, point: Point) {
        let Some(stroke) = self.current_stroke.as_mut() else {
            return;
        };

        let min_distance = (1.5 / self.zoom).max(0.01);
        if stroke
            .points
            .last()
            .is_none_or(|last| last.distance(point) >= min_distance)
        {
            stroke.points.push(point);
        }
    }

    fn finish_current_stroke(&mut self) {
        let Some(mut stroke) = self.current_stroke.take() else {
            return;
        };

        stroke.points = smooth_points(&stroke.points);
        if stroke.points.len() >= 2 {
            self.strokes.push(stroke);
            self.redo_stack.clear();
            self.invalidate_generation("Stroke added. Press Generate to refresh output.");
        }
    }

    fn erase_at(&mut self, point: Point) {
        let radius = 14.0 / self.zoom;
        let before = self.strokes.len();
        self.strokes
            .retain(|stroke| distance_to_polyline(point, &stroke.points) > radius);

        if self.strokes.len() != before {
            self.redo_stack.clear();
            self.invalidate_generation("Erased stroke. Press Generate to refresh output.");
        }
    }

    fn undo(&mut self) {
        if let Some(stroke) = self.strokes.pop() {
            self.redo_stack.push(stroke);
            self.invalidate_generation("Undo.");
        }
    }

    fn redo(&mut self) {
        if let Some(stroke) = self.redo_stack.pop() {
            self.strokes.push(stroke);
            self.invalidate_generation("Redo.");
        }
    }

    fn clear(&mut self) {
        if self.strokes.is_empty() {
            return;
        }

        self.strokes.clear();
        self.redo_stack.clear();
        self.generated_preview.clear();
        self.generated_text.clear();
        self.status = "Canvas cleared.".to_owned();
        self.dirty = true;
    }

    fn invalidate_generation(&mut self, status: impl Into<String>) {
        self.generated_preview.clear();
        self.generated_text.clear();
        self.status = status.into();
        self.dirty = true;
    }

    fn generate(&mut self) {
        self.generated_preview.clear();
        self.generated_text.clear();

        if self.strokes.is_empty() {
            self.status = "Nothing to generate yet.".to_owned();
            return;
        }

        let mut expression_count = 0;
        let mut skipped = 0;
        let mut segments = Vec::new();

        for stroke in &self.strokes {
            let simplified = simplify_points(&stroke.points, self.simplify_tolerance);
            if simplified.len() < 2 {
                skipped += 1;
                continue;
            }

            for pair in simplified.windows(2) {
                let a = pair[0];
                let b = pair[1];
                if a.distance(b) >= f32::EPSILON {
                    segments.push((a, b));
                    expression_count += 1;
                }
            }
            self.generated_preview.push(simplified);
        }

        self.generated_text = desmos_batched_segments(&segments);
        self.status = if expression_count == 0 {
            "Generation failed: strokes are too short after simplification.".to_owned()
        } else if skipped > 0 {
            format!(
                "Generated {expression_count} segments as one Desmos list-batched parametric curve. Skipped {skipped} tiny stroke(s)."
            )
        } else {
            format!(
                "Generated {expression_count} segments as one Desmos list-batched parametric curve."
            )
        };
    }

    fn save_project(&mut self) {
        let project = Project::from_strokes(&self.strokes);
        match serde_json::to_string_pretty(&project)
            .map_err(|err| err.to_string())
            .and_then(|json| {
                fs::write(PROJECT_FILE, json.as_bytes()).map_err(|err| err.to_string())
            }) {
            Ok(()) => {
                self.last_saved_hash = project_hash(&self.strokes);
                self.dirty = false;
                self.status = format!("Saved to {PROJECT_FILE}.");
            }
            Err(err) => {
                self.status = format!("Save failed: {err}");
            }
        }
    }

    fn load_project(&mut self) {
        let path = Path::new(PROJECT_FILE);
        match fs::read_to_string(path)
            .map_err(|err| err.to_string())
            .and_then(|json| serde_json::from_str::<Project>(&json).map_err(|err| err.to_string()))
        {
            Ok(project) => {
                self.strokes = project.strokes;
                self.redo_stack.clear();
                self.current_stroke = None;
                self.generated_preview.clear();
                self.generated_text.clear();
                self.last_saved_hash = project_hash(&self.strokes);
                self.dirty = false;
                self.status = format!(
                    "Loaded {} stroke(s) from {PROJECT_FILE}.",
                    self.strokes.len()
                );
            }
            Err(err) => {
                self.status = format!("Load failed: {err}");
            }
        }
    }

    fn copy_desmos(&mut self, ctx: &Context) {
        if self.generated_text.trim().is_empty() {
            self.generate();
        }

        if self.generated_text.trim().is_empty() {
            self.status = "Nothing to copy yet.".to_owned();
            return;
        }

        ctx.copy_text(self.generated_text.clone());
        self.status = "Copied Desmos expressions to clipboard.".to_owned();
    }

    fn apply_shortcuts(&mut self, ctx: &Context) {
        let (undo, redo, save, load, generate, copy, clear) = ctx.input(|input| {
            (
                input.modifiers.command && input.key_pressed(Key::Z) && !input.modifiers.shift,
                input.modifiers.command
                    && (input.key_pressed(Key::Y)
                        || (input.key_pressed(Key::Z) && input.modifiers.shift)),
                input.modifiers.command && input.key_pressed(Key::S),
                input.modifiers.command && input.key_pressed(Key::O),
                input.key_pressed(Key::G),
                input.modifiers.command && input.key_pressed(Key::C),
                input.key_pressed(Key::Delete),
            )
        });

        if undo {
            self.undo();
        }
        if redo {
            self.redo();
        }
        if save {
            self.save_project();
        }
        if load {
            self.load_project();
        }
        if generate {
            self.generate();
        }
        if copy {
            self.copy_desmos(ctx);
        }
        if clear {
            self.clear();
        }
    }

    fn top_bar(&mut self, ctx: &Context, ui: &mut Ui) {
        ui.horizontal_wrapped(|ui| {
            if ui.button("New").clicked() {
                self.clear();
            }
            if ui.button("Save").clicked() {
                self.save_project();
            }
            if ui.button("Load").clicked() {
                self.load_project();
            }

            ui.separator();

            if ui
                .add_enabled(!self.strokes.is_empty(), egui::Button::new("Undo"))
                .clicked()
            {
                self.undo();
            }
            if ui
                .add_enabled(!self.redo_stack.is_empty(), egui::Button::new("Redo"))
                .clicked()
            {
                self.redo();
            }

            ui.separator();

            if ui.button("Generate").clicked() {
                self.generate();
            }
            if ui.button("Copy to Desmos").clicked() {
                self.copy_desmos(ctx);
            }

            ui.separator();

            ui.label("Theme");
            let mut next_theme = self.theme;
            egui::ComboBox::from_id_salt("theme")
                .selected_text(match self.theme {
                    ThemeChoice::System => "System",
                    ThemeChoice::Light => "Light",
                    ThemeChoice::Dark => "Dark",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut next_theme, ThemeChoice::System, "System");
                    ui.selectable_value(&mut next_theme, ThemeChoice::Light, "Light");
                    ui.selectable_value(&mut next_theme, ThemeChoice::Dark, "Dark");
                });
            if next_theme != self.theme {
                self.set_theme(ctx, next_theme);
            }

            ui.separator();

            let save_state = if self.dirty || self.last_saved_hash != project_hash(&self.strokes) {
                "Unsaved"
            } else {
                "Saved"
            };
            ui.label(RichText::new(save_state).weak());
        });
    }

    fn side_bar(&mut self, ui: &mut Ui) {
        ui.heading("Tools");
        ui.radio_value(&mut self.tool, Tool::Pencil, "Pencil");
        ui.radio_value(&mut self.tool, Tool::Eraser, "Eraser");
        ui.radio_value(&mut self.tool, Tool::Pan, "Pan");

        ui.add_space(10.0);
        ui.heading("Stroke");
        ui.add(Slider::new(&mut self.stroke_width, 1.0..=8.0).text("Width"));
        ui.add(Slider::new(&mut self.simplify_tolerance, 0.01..=0.6).text("Simplify"));

        ui.add_space(10.0);
        ui.heading("View");
        ui.checkbox(&mut self.show_grid, "Grid");
        ui.checkbox(&mut self.show_original, "Original");
        ui.checkbox(&mut self.show_generated, "Generated");
        if ui.button("Reset view").clicked() {
            self.zoom = INITIAL_ZOOM;
            self.pan = Vec2::ZERO;
        }

        ui.add_space(10.0);
        ui.heading("Project");
        ui.label(format!("Strokes: {}", self.strokes.len()));
        ui.label(format!("Generated paths: {}", self.generated_preview.len()));
        ui.label(format!("Zoom: {:.0} px/unit", self.zoom));

        ui.add_space(10.0);
        ui.heading("Status");
        ui.label(self.status.as_str());

        ui.add_space(10.0);
        ui.heading("Desmos output");
        ui.add(
            egui::TextEdit::multiline(&mut self.generated_text)
                .desired_rows(14)
                .code_editor()
                .hint_text("Press Generate to create one Desmos list-batched parametric curve."),
        );
    }

    fn canvas(&mut self, ctx: &Context, ui: &mut Ui) {
        let available = ui.available_size_before_wrap();
        let (rect, response) = ui.allocate_exact_size(available, Sense::click_and_drag());
        let painter = ui.painter_at(rect);

        let visuals = ui.visuals().clone();
        let canvas_bg = if visuals.dark_mode {
            Color32::from_rgb(26, 28, 31)
        } else {
            Color32::from_rgb(250, 250, 248)
        };
        painter.rect_filled(rect, 0.0, canvas_bg);

        if response.hovered() {
            let scroll = ctx.input(|input| input.smooth_scroll_delta.y);
            if scroll.abs() > 0.0 {
                let factor = (scroll * 0.0015).exp();
                self.zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
            }
        }

        let pointer_pos = response.interact_pointer_pos();
        let secondary_pan = response.dragged_by(PointerButton::Secondary);
        let middle_pan = response.dragged_by(PointerButton::Middle);
        let should_pan = self.tool == Tool::Pan && response.dragged_by(PointerButton::Primary)
            || secondary_pan
            || middle_pan;
        if should_pan {
            let delta = ctx.input(|input| input.pointer.delta());
            self.pan += delta;
        }

        if self.tool == Tool::Pencil && response.drag_started_by(PointerButton::Primary) {
            let mut stroke = DrawStroke::new(
                if visuals.dark_mode {
                    Color32::from_rgb(235, 238, 244)
                } else {
                    Color32::from_rgb(23, 26, 31)
                },
                self.stroke_width,
            );
            if let Some(pos) = pointer_pos {
                stroke.points.push(self.screen_to_math(pos, rect));
            }
            self.current_stroke = Some(stroke);
        }

        if self.tool == Tool::Pencil && response.dragged_by(PointerButton::Primary) {
            if let Some(pos) = pointer_pos {
                self.push_current_point(self.screen_to_math(pos, rect));
            }
        }

        if self.tool == Tool::Pencil && response.drag_stopped_by(PointerButton::Primary) {
            self.finish_current_stroke();
        }

        if self.tool == Tool::Eraser && response.dragged_by(PointerButton::Primary) {
            if let Some(pos) = pointer_pos {
                self.erase_at(self.screen_to_math(pos, rect));
            }
        }

        if self.show_grid {
            self.paint_grid(&painter, rect, visuals.dark_mode);
        }

        if self.show_original {
            for stroke in &self.strokes {
                self.paint_polyline(
                    &painter,
                    rect,
                    &stroke.points,
                    stroke.color32(),
                    stroke.width,
                );
            }
            if let Some(stroke) = &self.current_stroke {
                self.paint_polyline(
                    &painter,
                    rect,
                    &stroke.points,
                    stroke.color32(),
                    stroke.width,
                );
            }
        }

        if self.show_generated {
            let generated_color = if visuals.dark_mode {
                Color32::from_rgb(73, 209, 151)
            } else {
                Color32::from_rgb(0, 128, 96)
            };
            for path in &self.generated_preview {
                self.paint_polyline(&painter, rect, path, generated_color, 2.5);
            }
        }

        if self.tool == Tool::Eraser {
            if let Some(pos) = pointer_pos {
                painter.circle_stroke(
                    pos,
                    14.0,
                    EguiStroke::new(1.5, Color32::from_rgb(232, 85, 85)),
                );
            }
        }

        if self.strokes.is_empty() && self.current_stroke.is_none() {
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                "Draw a shape, then Generate",
                FontId::proportional(22.0),
                visuals.weak_text_color(),
            );
        }
    }

    fn paint_grid(&self, painter: &egui::Painter, rect: Rect, dark_mode: bool) {
        let origin = rect.center() + self.pan;
        let grid_color = if dark_mode {
            Color32::from_rgba_unmultiplied(255, 255, 255, 24)
        } else {
            Color32::from_rgba_unmultiplied(0, 0, 0, 22)
        };
        let axis_color = if dark_mode {
            Color32::from_rgba_unmultiplied(255, 255, 255, 80)
        } else {
            Color32::from_rgba_unmultiplied(0, 0, 0, 80)
        };

        let step = nice_grid_step(self.zoom);
        let screen_step = step * self.zoom;

        let mut x = origin.x - ((origin.x - rect.left()) / screen_step).ceil() * screen_step;
        while x <= rect.right() {
            painter.line_segment(
                [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
                EguiStroke::new(1.0, grid_color),
            );
            x += screen_step;
        }

        let mut y = origin.y - ((origin.y - rect.top()) / screen_step).ceil() * screen_step;
        while y <= rect.bottom() {
            painter.line_segment(
                [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
                EguiStroke::new(1.0, grid_color),
            );
            y += screen_step;
        }

        painter.line_segment(
            [
                Pos2::new(rect.left(), origin.y),
                Pos2::new(rect.right(), origin.y),
            ],
            EguiStroke::new(1.5, axis_color),
        );
        painter.line_segment(
            [
                Pos2::new(origin.x, rect.top()),
                Pos2::new(origin.x, rect.bottom()),
            ],
            EguiStroke::new(1.5, axis_color),
        );
    }

    fn paint_polyline(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        points: &[Point],
        color: Color32,
        width: f32,
    ) {
        for pair in points.windows(2) {
            painter.line_segment(
                [
                    self.math_to_screen(pair[0], rect),
                    self.math_to_screen(pair[1], rect),
                ],
                EguiStroke::new(width, color),
            );
        }
    }
}

impl eframe::App for DrawafuncApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.apply_shortcuts(ctx);

        TopBottomPanel::top("top_bar").show(ctx, |ui| {
            self.top_bar(ctx, ui);
        });

        SidePanel::right("right_panel")
            .resizable(true)
            .default_width(310.0)
            .width_range(250.0..=520.0)
            .show(ctx, |ui| {
                self.side_bar(ui);
            });

        CentralPanel::default().show(ctx, |ui| {
            self.canvas(ctx, ui);
        });
    }
}

fn smooth_points(points: &[Point]) -> Vec<Point> {
    if points.len() < 3 {
        return points.to_vec();
    }

    let mut smoothed = Vec::with_capacity(points.len());
    smoothed.push(points[0]);
    for index in 1..points.len() - 1 {
        let previous = points[index - 1];
        let current = points[index];
        let next = points[index + 1];
        smoothed.push(Point {
            x: previous.x * 0.2 + current.x * 0.6 + next.x * 0.2,
            y: previous.y * 0.2 + current.y * 0.6 + next.y * 0.2,
        });
    }
    smoothed.push(*points.last().unwrap());
    smoothed
}

fn simplify_points(points: &[Point], tolerance: f32) -> Vec<Point> {
    if points.len() <= 2 {
        return points.to_vec();
    }

    let mut keep = vec![false; points.len()];
    keep[0] = true;
    keep[points.len() - 1] = true;
    simplify_range(points, 0, points.len() - 1, tolerance, &mut keep);

    points
        .iter()
        .zip(keep)
        .filter_map(|(point, keep)| keep.then_some(*point))
        .collect()
}

fn simplify_range(points: &[Point], start: usize, end: usize, tolerance: f32, keep: &mut [bool]) {
    if end <= start + 1 {
        return;
    }

    let mut max_distance = 0.0;
    let mut max_index = start;
    for index in start + 1..end {
        let distance = distance_to_segment(points[index], points[start], points[end]);
        if distance > max_distance {
            max_distance = distance;
            max_index = index;
        }
    }

    if max_distance > tolerance {
        keep[max_index] = true;
        simplify_range(points, start, max_index, tolerance, keep);
        simplify_range(points, max_index, end, tolerance, keep);
    }
}

fn distance_to_polyline(point: Point, points: &[Point]) -> f32 {
    match points.len() {
        0 => f32::INFINITY,
        1 => point.distance(points[0]),
        _ => points
            .windows(2)
            .map(|pair| distance_to_segment(point, pair[0], pair[1]))
            .fold(f32::INFINITY, f32::min),
    }
}

fn distance_to_segment(point: Point, a: Point, b: Point) -> f32 {
    let ab_x = b.x - a.x;
    let ab_y = b.y - a.y;
    let ap_x = point.x - a.x;
    let ap_y = point.y - a.y;
    let ab_len_sq = ab_x * ab_x + ab_y * ab_y;

    if ab_len_sq <= f32::EPSILON {
        return point.distance(a);
    }

    let t = ((ap_x * ab_x + ap_y * ab_y) / ab_len_sq).clamp(0.0, 1.0);
    point.distance(a.lerp(b, t))
}

fn nice_grid_step(zoom: f32) -> f32 {
    let target_pixels = 64.0;
    let raw = target_pixels / zoom;
    let power = 10.0_f32.powf(raw.log10().floor());
    let normalized = raw / power;

    let nice = if normalized < 1.5 {
        1.0
    } else if normalized < 3.5 {
        2.0
    } else if normalized < 7.5 {
        5.0
    } else {
        10.0
    };

    nice * power
}

fn fmt_num(value: f32) -> String {
    let rounded = if value.abs() < 0.000_001 { 0.0 } else { value };
    let mut text = format!("{rounded:.4}");
    while text.contains('.') && text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    text
}

fn desmos_batched_segments(segments: &[(Point, Point)]) -> String {
    if segments.is_empty() {
        return String::new();
    }

    let x1: Vec<_> = segments.iter().map(|(a, _)| a.x).collect();
    let x2: Vec<_> = segments.iter().map(|(_, b)| b.x).collect();
    let y1: Vec<_> = segments.iter().map(|(a, _)| a.y).collect();
    let y2: Vec<_> = segments.iter().map(|(_, b)| b.y).collect();

    [
        format!("X_1={}", fmt_num_list(&x1)),
        format!("X_2={}", fmt_num_list(&x2)),
        format!("Y_1={}", fmt_num_list(&y1)),
        format!("Y_2={}", fmt_num_list(&y2)),
        "(X_1+(X_2-X_1)*t,Y_1+(Y_2-Y_1)*t)".to_owned(),
    ]
    .join("\n")
}

fn fmt_num_list(values: &[f32]) -> String {
    let values = values
        .iter()
        .map(|value| fmt_num(*value))
        .collect::<Vec<_>>()
        .join(",");
    format!("[{values}]")
}

fn project_hash(strokes: &[DrawStroke]) -> u64 {
    let mut hash = 14_695_981_039_346_656_037_u64;
    for stroke in strokes {
        hash ^= stroke.points.len() as u64;
        hash = hash.wrapping_mul(1_099_511_628_211);
        for point in &stroke.points {
            hash ^= point.x.to_bits() as u64;
            hash = hash.wrapping_mul(1_099_511_628_211);
            hash ^= point.y.to_bits() as u64;
            hash = hash.wrapping_mul(1_099_511_628_211);
        }
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fmt_num_trims_noise_and_zeroes() {
        assert_eq!(fmt_num(0.0), "0");
        assert_eq!(fmt_num(1.5000), "1.5");
        assert_eq!(fmt_num(-2.1250), "-2.125");
        assert_eq!(fmt_num(0.000_000_1), "0");
    }

    #[test]
    fn simplify_keeps_line_endpoints() {
        let points = vec![
            Point { x: 0.0, y: 0.0 },
            Point { x: 1.0, y: 0.01 },
            Point { x: 2.0, y: 0.0 },
        ];

        let simplified = simplify_points(&points, 0.05);

        assert_eq!(simplified.len(), 2);
        assert_eq!(simplified[0].x, 0.0);
        assert_eq!(simplified[1].x, 2.0);
    }

    #[test]
    fn simplify_preserves_visible_corner() {
        let points = vec![
            Point { x: 0.0, y: 0.0 },
            Point { x: 1.0, y: 1.0 },
            Point { x: 2.0, y: 0.0 },
        ];

        let simplified = simplify_points(&points, 0.05);

        assert_eq!(simplified.len(), 3);
    }

    #[test]
    fn desmos_batched_export_uses_lists_and_one_curve() {
        let export = desmos_batched_segments(&[
            (Point { x: 0.0, y: 1.0 }, Point { x: 2.0, y: 3.0 }),
            (Point { x: -2.0, y: 1.0 }, Point { x: -2.0, y: -3.0 }),
        ]);

        assert_eq!(
            export,
            [
                "X_1=[0,-2]",
                "X_2=[2,-2]",
                "Y_1=[1,1]",
                "Y_2=[3,-3]",
                "(X_1+(X_2-X_1)*t,Y_1+(Y_2-Y_1)*t)",
            ]
            .join("\n")
        );
    }

    #[test]
    fn desmos_batched_export_is_empty_without_segments() {
        assert!(desmos_batched_segments(&[]).is_empty());
    }
}
