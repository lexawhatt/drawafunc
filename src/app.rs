use eframe::egui::{self, Color32, Context, Key, Pos2, Rect, Vec2};

use crate::generation;
use crate::geometry::{distance_to_polyline, smooth_points};
use crate::model::{DrawStroke, Point};
use crate::persistence::{self, PROJECT_FILE, project_hash};
use crate::settings::{GenerationSettings, OutputMode, QualityPreset};
use crate::shapes::ShapeTool;

pub(crate) const MIN_ZOOM: f32 = 8.0;
pub(crate) const MAX_ZOOM: f32 = 260.0;
pub(crate) const INITIAL_ZOOM: f32 = 42.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Tool {
    Pencil,
    Line,
    Rectangle,
    Circle,
    Heart,
    Star,
    Eraser,
    Pan,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ThemeChoice {
    System,
    Light,
    Dark,
}

pub(crate) struct DrawafuncApp {
    pub(crate) strokes: Vec<DrawStroke>,
    pub(crate) redo_stack: Vec<DrawStroke>,
    pub(crate) current_stroke: Option<DrawStroke>,
    pub(crate) tool: Tool,
    pub(crate) theme: ThemeChoice,
    pub(crate) zoom: f32,
    pub(crate) pan: Vec2,
    pub(crate) stroke_width: f32,
    pub(crate) simplify_tolerance: f32,
    pub(crate) quality: QualityPreset,
    pub(crate) output_mode: OutputMode,
    pub(crate) polynomial_degree: usize,
    pub(crate) generated_preview: Vec<Vec<Point>>,
    pub(crate) generated_text: String,
    pub(crate) status: String,
    pub(crate) last_saved_hash: u64,
    pub(crate) dirty: bool,
    pub(crate) show_original: bool,
    pub(crate) show_generated: bool,
    pub(crate) show_grid: bool,
    pub(crate) drag_shape_start: Option<Point>,
}

impl DrawafuncApp {
    pub(crate) fn new(cc: &eframe::CreationContext<'_>) -> Self {
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
            quality: QualityPreset::Default,
            output_mode: OutputMode::Auto,
            polynomial_degree: 4,
            generated_preview: Vec::new(),
            generated_text: String::new(),
            status: "Draw on the canvas, then press Generate.".to_owned(),
            last_saved_hash,
            dirty: false,
            show_original: true,
            show_generated: true,
            show_grid: true,
            drag_shape_start: None,
        }
    }

    pub(crate) fn set_theme(&mut self, ctx: &Context, theme: ThemeChoice) {
        self.theme = theme;
        match theme {
            ThemeChoice::System => ctx.set_theme(egui::ThemePreference::System),
            ThemeChoice::Light => ctx.set_theme(egui::ThemePreference::Light),
            ThemeChoice::Dark => ctx.set_theme(egui::ThemePreference::Dark),
        }
    }

    pub(crate) fn screen_to_math(&self, pos: Pos2, rect: Rect) -> Point {
        let origin = rect.center() + self.pan;
        Point {
            x: (pos.x - origin.x) / self.zoom,
            y: -(pos.y - origin.y) / self.zoom,
        }
    }

    pub(crate) fn math_to_screen(&self, point: Point, rect: Rect) -> Pos2 {
        let origin = rect.center() + self.pan;
        Pos2::new(
            origin.x + point.x * self.zoom,
            origin.y - point.y * self.zoom,
        )
    }

    pub(crate) fn push_current_point(&mut self, point: Point) {
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

    pub(crate) fn start_stroke(&mut self, color: Color32, point: Point) {
        let mut stroke = DrawStroke::new(color, self.stroke_width);
        stroke.points.push(point);
        self.current_stroke = Some(stroke);
    }

    pub(crate) fn finish_current_stroke(&mut self) {
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

    pub(crate) fn shape_tool(&self) -> Option<ShapeTool> {
        match self.tool {
            Tool::Line => Some(ShapeTool::Line),
            Tool::Rectangle => Some(ShapeTool::Rectangle),
            Tool::Circle => Some(ShapeTool::Circle),
            Tool::Heart => Some(ShapeTool::Heart),
            Tool::Star => Some(ShapeTool::Star),
            Tool::Pencil | Tool::Eraser | Tool::Pan => None,
        }
    }

    pub(crate) fn add_stroke(&mut self, stroke: DrawStroke) {
        if stroke.points.len() >= 2 {
            self.strokes.push(stroke);
            self.redo_stack.clear();
            self.invalidate_generation("Object added. Press Generate to refresh output.");
        }
    }

    pub(crate) fn erase_at(&mut self, point: Point) {
        let radius = 14.0 / self.zoom;
        let before = self.strokes.len();
        self.strokes
            .retain(|stroke| distance_to_polyline(point, &stroke.points) > radius);

        if self.strokes.len() != before {
            self.redo_stack.clear();
            self.invalidate_generation("Erased stroke. Press Generate to refresh output.");
        }
    }

    pub(crate) fn undo(&mut self) {
        if let Some(stroke) = self.strokes.pop() {
            self.redo_stack.push(stroke);
            self.invalidate_generation("Undo.");
        }
    }

    pub(crate) fn redo(&mut self) {
        if let Some(stroke) = self.redo_stack.pop() {
            self.strokes.push(stroke);
            self.invalidate_generation("Redo.");
        }
    }

    pub(crate) fn clear(&mut self) {
        if self.strokes.is_empty() {
            return;
        }

        self.strokes.clear();
        self.redo_stack.clear();
        self.drag_shape_start = None;
        self.generated_preview.clear();
        self.generated_text.clear();
        self.status = "Canvas cleared.".to_owned();
        self.dirty = true;
    }

    pub(crate) fn invalidate_generation(&mut self, status: impl Into<String>) {
        self.generated_preview.clear();
        self.generated_text.clear();
        self.status = status.into();
        self.dirty = true;
    }

    pub(crate) fn generate(&mut self) {
        let result = generation::generate(
            &self.strokes,
            GenerationSettings {
                quality: self.quality,
                output_mode: self.output_mode,
                simplify_tolerance: self.simplify_tolerance,
                polynomial_degree: self.polynomial_degree,
            },
        );

        self.generated_preview = result.scene.preview_paths;
        self.generated_text = result.export_text;
        self.status = result.status;
    }

    pub(crate) fn save_project(&mut self) {
        match persistence::save_strokes(&self.strokes) {
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

    pub(crate) fn load_project(&mut self) {
        match persistence::load_project() {
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

    pub(crate) fn copy_desmos(&mut self, ctx: &Context) {
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
}

impl eframe::App for DrawafuncApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.apply_shortcuts(ctx);
        self.render(ctx);
    }
}
