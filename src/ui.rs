use eframe::egui::{
    self, Align2, CentralPanel, Color32, Context, FontId, PointerButton, Rect, RichText, Sense,
    SidePanel, Slider, Stroke as EguiStroke, TopBottomPanel, Ui,
};

use crate::app::{DrawafuncApp, INITIAL_ZOOM, MAX_ZOOM, MIN_ZOOM, ThemeChoice, Tool};
use crate::geometry::nice_grid_step;
use crate::model::Point;
use crate::persistence::project_hash;
use crate::settings::{OutputMode, QualityPreset};
use crate::shapes::{make_shape_stroke, shape_points};

impl DrawafuncApp {
    pub(crate) fn render(&mut self, ctx: &Context) {
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
        ui.radio_value(&mut self.tool, Tool::Line, "Line");
        ui.radio_value(&mut self.tool, Tool::Rectangle, "Rectangle");
        ui.radio_value(&mut self.tool, Tool::Circle, "Circle");
        ui.radio_value(&mut self.tool, Tool::Heart, "Heart");
        ui.radio_value(&mut self.tool, Tool::Star, "Star");
        ui.radio_value(&mut self.tool, Tool::Eraser, "Eraser");
        ui.radio_value(&mut self.tool, Tool::Pan, "Pan");

        ui.add_space(10.0);
        ui.heading("Generation");
        egui::ComboBox::from_id_salt("quality")
            .selected_text(self.quality.label())
            .show_ui(ui, |ui| {
                for quality in QualityPreset::ALL {
                    ui.selectable_value(&mut self.quality, quality, quality.label());
                }
            });
        egui::ComboBox::from_id_salt("output_mode")
            .selected_text(self.output_mode.label())
            .show_ui(ui, |ui| {
                for mode in OutputMode::ALL {
                    ui.selectable_value(&mut self.output_mode, mode, mode.label());
                }
            });
        ui.add(Slider::new(&mut self.polynomial_degree, 1..=3).text("Polynomial max degree"));
        ui.add_space(6.0);
        ui.add(Slider::new(&mut self.stroke_width, 1.0..=8.0).text("Width"));
        ui.add(Slider::new(&mut self.simplify_tolerance, 0.01..=0.6).text("Simplify"));

        ui.add_space(10.0);
        ui.heading("View");
        ui.checkbox(&mut self.show_grid, "Grid");
        ui.checkbox(&mut self.show_original, "Original");
        ui.checkbox(&mut self.show_generated, "Generated");
        if ui.button("Reset view").clicked() {
            self.zoom = INITIAL_ZOOM;
            self.pan = egui::Vec2::ZERO;
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

        if self.shape_tool().is_some() && response.drag_started_by(PointerButton::Primary) {
            if let Some(pos) = pointer_pos {
                self.drag_shape_start = Some(self.screen_to_math(pos, rect));
            }
        }

        if self.shape_tool().is_some() && response.drag_stopped_by(PointerButton::Primary) {
            if let (Some(shape), Some(start), Some(pos)) =
                (self.shape_tool(), self.drag_shape_start.take(), pointer_pos)
            {
                let color = active_draw_color(visuals.dark_mode);
                let end = self.screen_to_math(pos, rect);
                if let Some(stroke) = make_shape_stroke(shape, start, end, color, self.stroke_width)
                {
                    self.add_stroke(stroke);
                }
            }
        }

        if self.tool == Tool::Pencil && response.drag_started_by(PointerButton::Primary) {
            if let Some(pos) = pointer_pos {
                let color = active_draw_color(visuals.dark_mode);
                self.start_stroke(color, self.screen_to_math(pos, rect));
            }
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

        if let (Some(shape), Some(start), Some(pos)) =
            (self.shape_tool(), self.drag_shape_start, pointer_pos)
        {
            if let Some(points) = shape_points(shape, start, self.screen_to_math(pos, rect)) {
                self.paint_polyline(
                    &painter,
                    rect,
                    &points,
                    Color32::from_rgb(85, 170, 255),
                    self.stroke_width,
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

        if self.strokes.is_empty()
            && self.current_stroke.is_none()
            && self.drag_shape_start.is_none()
        {
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
                [
                    egui::Pos2::new(x, rect.top()),
                    egui::Pos2::new(x, rect.bottom()),
                ],
                EguiStroke::new(1.0, grid_color),
            );
            x += screen_step;
        }

        let mut y = origin.y - ((origin.y - rect.top()) / screen_step).ceil() * screen_step;
        while y <= rect.bottom() {
            painter.line_segment(
                [
                    egui::Pos2::new(rect.left(), y),
                    egui::Pos2::new(rect.right(), y),
                ],
                EguiStroke::new(1.0, grid_color),
            );
            y += screen_step;
        }

        painter.line_segment(
            [
                egui::Pos2::new(rect.left(), origin.y),
                egui::Pos2::new(rect.right(), origin.y),
            ],
            EguiStroke::new(1.5, axis_color),
        );
        painter.line_segment(
            [
                egui::Pos2::new(origin.x, rect.top()),
                egui::Pos2::new(origin.x, rect.bottom()),
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

fn active_draw_color(dark_mode: bool) -> Color32 {
    if dark_mode {
        Color32::from_rgb(235, 238, 244)
    } else {
        Color32::from_rgb(23, 26, 31)
    }
}
