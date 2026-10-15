use eframe::egui::{
    self, Align2, Color32, CornerRadius, CursorIcon, FontId, Pos2, Rect, RichText, Sense, Shape,
    Stroke, Ui, pos2, vec2,
};
use std::time::Duration;

use crate::{
    metadata::SUPPORTED_EXTENSIONS,
    player::{PlayerController, PlayerState},
    track::Track,
    utils::format_secs,
};

const BG: Color32 = Color32::from_rgb(201, 242, 224); // mint
const FG: Color32 = Color32::from_rgb(16, 53, 38); // ink
const TRACK: Color32 = Color32::from_rgb(183, 223, 205); // mint + 10% ink
const RED: Color32 = Color32::from_rgb(239, 68, 68);

const PLAY_BUTTON_SIZE: f32 = 64.0;
const PLAY_MORPH_SECS: f32 = 0.25;
const SLIDER_TRACK_HEIGHT: f32 = 5.0;
const SLIDER_THUMB_RADIUS: f32 = 8.0;

pub struct PlayerApp {
    controller: PlayerController,
    error: Option<String>,
    /// Позиция ползунка (в секундах), пока пользователь его тянет
    drag_secs: Option<f32>,
}

impl PlayerApp {
    pub fn new() -> Self {
        Self {
            controller: PlayerController::new(),
            error: None,
            drag_secs: None,
        }
    }

    fn set_result(&mut self, result: Result<(), String>) {
        self.error = result.err();
    }

    fn open_file(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Audio", SUPPORTED_EXTENSIONS)
            .pick_file()
        else {
            // Пользователь закрыл диалог: оставляем текущий трек
            return;
        };

        let result = Track::from_path(path).and_then(|track| self.controller.set_track(track));
        self.set_result(result);
    }

    fn toggle_play(&mut self) {
        let result = if self.controller.player().state == PlayerState::Playing {
            self.controller.pause()
        } else {
            self.controller.play()
        };
        self.set_result(result);
    }

    fn header(&mut self, ui: &mut Ui) {
        ui.vertical_centered(|ui| {
            let open =
                egui::Button::new(RichText::new("Open file").size(18.0).color(FG)).frame(false);
            if ui
                .add(open)
                .on_hover_cursor(CursorIcon::PointingHand)
                .clicked()
            {
                self.open_file();
            }

            if let Some(track) = &self.controller.player().current_track {
                ui.add_space(8.0);
                ui.label(RichText::new(&track.title).size(18.0).strong().color(FG));
                for line in [&track.artist, &track.album] {
                    if !line.is_empty() {
                        ui.label(RichText::new(line).color(FG));
                    }
                }
            }

            ui.add_space(8.0);

            // Временно показываем состояние плеера, чтобы видеть, что он реагирует на кнопки
            // let state = self.controller.player().state;
            // ui.label(RichText::new(format!("{state:?}")).size(18.0).color(RED));

            if let Some(error) = &self.error {
                ui.label(RichText::new(error).color(RED));
            }
        });
    }

    fn controls(&mut self, ui: &mut Ui) {
        let player = self.controller.player();
        let has_track = player.current_track.is_some();
        let playing = player.state == PlayerState::Playing;
        let duration = player
            .current_track
            .as_ref()
            .map_or(0.0, |t| t.duration_secs as f32);

        let position = self
            .drag_secs
            .unwrap_or(self.controller.position_ms() as f32 / 1000.0)
            .min(duration);

        // Ряд: время слева, кнопка по центру, длительность справа
        let (row, _) =
            ui.allocate_exact_size(vec2(ui.available_width(), PLAY_BUTTON_SIZE), Sense::hover());

        if has_track && duration > 0.0 {
            let font = FontId::proportional(16.0);
            let painter = ui.painter();
            painter.text(
                row.left_center(),
                Align2::LEFT_CENTER,
                format_secs(position),
                font.clone(),
                FG,
            );
            painter.text(
                row.right_center(),
                Align2::RIGHT_CENTER,
                format_secs(duration),
                font,
                FG,
            );
        }

        let button_rect =
            Rect::from_center_size(row.center(), vec2(PLAY_BUTTON_SIZE, PLAY_BUTTON_SIZE));
        let sense = if has_track {
            Sense::click()
        } else {
            Sense::hover()
        };
        let mut button = ui.interact(button_rect, ui.scope_id().with("play_pause"), sense);
        if has_track {
            button = button.on_hover_cursor(CursorIcon::PointingHand);
        }
        // 0.0 — треугольник play, 1.0 — полоски pause
        let morph = ui.ctx().animate_bool_with_time_and_easing(
            button.id,
            playing,
            PLAY_MORPH_SECS,
            egui::emath::easing::cubic_in_out,
        );
        paint_play_pause(ui, button_rect, morph, !has_track, button.hovered());

        if button.clicked() {
            self.toggle_play();
        }

        ui.add_space(1.0);
        self.progress_bar(ui, position, duration, has_track && duration > 0.0);
    }

    fn progress_bar(&mut self, ui: &mut Ui, position: f32, duration: f32, enabled: bool) {
        let sense = if enabled {
            Sense::click_and_drag()
        } else {
            Sense::hover()
        };
        let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), sense);

        let value_at =
            |pos: Pos2| ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0) * duration;

        if enabled {
            let response = response.on_hover_cursor(CursorIcon::PointingHand);

            if response.dragged()
                && let Some(pos) = response.interact_pointer_pos()
            {
                self.drag_secs = Some(value_at(pos));
            }

            if response.drag_stopped() || response.clicked() {
                // При отпускании курсор может быть уже недоступен — берём последнюю позицию драга
                let secs = response
                    .interact_pointer_pos()
                    .map(value_at)
                    .or(self.drag_secs);
                self.drag_secs = None;

                if let Some(secs) = secs {
                    let result = self.controller.seek((secs * 1000.0) as u64);
                    self.set_result(result);
                }
            }
        }

        let fraction = if duration > 0.0 {
            (self.drag_secs.unwrap_or(position) / duration).clamp(0.0, 1.0)
        } else {
            0.0
        };

        let painter = ui.painter();
        let track = Rect::from_center_size(rect.center(), vec2(rect.width(), SLIDER_TRACK_HEIGHT));
        painter.rect_filled(track, CornerRadius::same(255), TRACK);

        let thumb_x = track.left() + track.width() * fraction;
        let filled = Rect::from_min_max(track.min, pos2(thumb_x, track.max.y));
        painter.rect_filled(filled, CornerRadius::same(255), FG);

        // Ползунок не вылезает за края окна
        let thumb_x = thumb_x.clamp(
            rect.left() + SLIDER_THUMB_RADIUS,
            rect.right() - SLIDER_THUMB_RADIUS,
        );
        painter.circle_filled(pos2(thumb_x, rect.center().y), SLIDER_THUMB_RADIUS, FG);
    }
}

impl eframe::App for PlayerApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        self.controller.poll_events();

        // Пока трек играет, обновляем время и ползунок
        if self.controller.player().state == PlayerState::Playing {
            ui.ctx().request_repaint_after(Duration::from_millis(100));
        }

        let frame = egui::Frame::central_panel(ui.style())
            .fill(BG)
            .inner_margin(16);

        egui::CentralPanel::default().frame(frame).show(ui, |ui| {
            self.header(ui);
            ui.add_space(16.0);
            self.controls(ui);
        });
    }
}

// Иконки в координатах 24×24. Каждая фигура задана внутренним контуром и радиусом:
// итоговая форма — контур, раздутый на радиус, так что углы получаются скруглёнными.
// Треугольник play разрезан по вертикали на две половины, каждая — четырёхугольник,
// вершины которого попарно переходят в вершины полоски pause.
const PLAY_TRIANGLE: [(f32, f32); 3] = [(8.0, 6.5), (17.5, 12.0), (8.0, 17.5)];
const PLAY_HALVES: [[(f32, f32); 4]; 2] = [
    [(8.0, 6.5), (12.75, 9.25), (12.75, 14.75), (8.0, 17.5)],
    [(12.75, 9.25), (17.5, 12.0), (17.5, 12.0), (12.75, 14.75)],
];
const PAUSE_BARS: [[(f32, f32); 4]; 2] = [
    [(7.0, 6.0), (9.0, 6.0), (9.0, 18.0), (7.0, 18.0)],
    [(15.0, 6.0), (17.0, 6.0), (17.0, 18.0), (15.0, 18.0)],
];
const PLAY_CORNER_RADIUS: f32 = 1.0;
const PAUSE_CORNER_RADIUS: f32 = 1.0;

/// `morph`: 0.0 — play, 1.0 — pause, между ними — промежуточная форма
fn paint_play_pause(ui: &Ui, rect: Rect, morph: f32, disabled: bool, hovered: bool) {
    let alpha = if disabled { 0.4 } else { 1.0 };
    let icon_color = FG.gamma_multiply(alpha);
    let bg_opacity = if hovered && !disabled { 0.25 } else { 0.18 };

    let painter = ui.painter();
    painter.circle_filled(
        rect.center(),
        rect.width() / 2.0,
        FG.gamma_multiply(bg_opacity * alpha),
    );

    let scale = rect.width() / 24.0;
    let p = |(x, y): (f32, f32)| rect.min + vec2(x, y) * scale;
    let fill = |points: Vec<Pos2>, radius: f32| {
        let outline = rounded_convex(&points, radius * scale);
        painter.add(Shape::convex_polygon(outline, icon_color, Stroke::NONE));
    };

    if morph <= 0.0 {
        // Целиком, а не половинками: иначе на полупрозрачной иконке видно их наложение
        fill(PLAY_TRIANGLE.map(p).to_vec(), PLAY_CORNER_RADIUS);
    } else {
        let radius = egui::lerp(PLAY_CORNER_RADIUS..=PAUSE_CORNER_RADIUS, morph);
        for (from, to) in PLAY_HALVES.iter().zip(&PAUSE_BARS) {
            let points = from
                .iter()
                .zip(to)
                .map(|(&a, &b)| p(a).lerp(p(b), morph))
                .collect();
            fill(points, radius);
        }
    }
}

/// Контур выпуклого многоугольника, раздутого на `radius`: выпуклая оболочка
/// кругов вокруг вершин. Совпадающие вершины допустимы.
fn rounded_convex(points: &[Pos2], radius: f32) -> Vec<Pos2> {
    const ARC_SEGMENTS: usize = 16;

    let mut samples: Vec<Pos2> = points
        .iter()
        .flat_map(|&center| {
            (0..ARC_SEGMENTS).map(move |i| {
                let angle = i as f32 / ARC_SEGMENTS as f32 * std::f32::consts::TAU;
                center + radius * egui::Vec2::angled(angle)
            })
        })
        .collect();
    samples.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));

    // Выпуклая оболочка (монотонная цепочка Эндрю): нижняя цепочка, затем верхняя
    let cross = |o: Pos2, a: Pos2, b: Pos2| (a - o).x * (b - o).y - (a - o).y * (b - o).x;
    let mut hull: Vec<Pos2> = Vec::with_capacity(samples.len() + 1);
    for point in samples.iter().copied() {
        while hull.len() >= 2 && cross(hull[hull.len() - 2], hull[hull.len() - 1], point) <= 0.0 {
            hull.pop();
        }
        hull.push(point);
    }
    let lower_len = hull.len() + 1;
    for point in samples.iter().rev().skip(1).copied() {
        while hull.len() >= lower_len
            && cross(hull[hull.len() - 2], hull[hull.len() - 1], point) <= 0.0
        {
            hull.pop();
        }
        hull.push(point);
    }
    hull.pop();
    hull
}
