//! Нижняя панель: prev / play-pause / next, время и ползунок перемотки.

use eframe::egui::{
    self, Align2, CornerRadius, CursorIcon, FontId, Pos2, Rect, Sense, Ui, pos2, vec2,
};

use super::PlayerApp;
use crate::{
    playback::PlayerState,
    ui::{
        format::format_secs,
        icons::{paint_play_pause, paint_skip, skip_hit_rect},
        theme::{SLIDER_TRACK, TEXT, TEXT_MUTED},
        widgets,
    },
};

const PLAY_BUTTON_SIZE: f32 = 60.0;
const PLAY_MORPH_SECS: f32 = 0.25;
const SKIP_BUTTON_SIZE: f32 = 40.0;
// Зазор между кнопкой play и кнопками prev/next
const SKIP_BUTTON_GAP: f32 = 4.0;
const SLIDER_TRACK_HEIGHT: f32 = 5.0;
const SLIDER_THUMB_RADIUS: f32 = 8.0;

impl PlayerApp {
    pub(super) fn toggle_play(&mut self) {
        let result = if self.controller.player().state == PlayerState::Playing {
            self.controller.pause()
        } else {
            self.controller.play()
        };
        self.set_result(result);
    }

    pub(super) fn render_controls(&mut self, ui: &mut Ui) {
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
                TEXT_MUTED,
            );
            // Справа — сколько осталось. Округляем вверх: format_secs отбрасывает дробную
            // часть, и иначе прошедшее + оставшееся давало бы на секунду меньше длительности
            let remaining = (duration - position).max(0.0).ceil();
            painter.text(
                row.right_center(),
                Align2::RIGHT_CENTER,
                format_secs(remaining),
                font,
                TEXT_MUTED,
            );
        }

        // prev и next стоят по бокам от play
        let play_rect =
            Rect::from_center_size(row.center(), vec2(PLAY_BUTTON_SIZE, PLAY_BUTTON_SIZE));
        let skip_offset = vec2(
            (PLAY_BUTTON_SIZE + SKIP_BUTTON_SIZE) / 2.0 + SKIP_BUTTON_GAP,
            0.0,
        );
        let skip_size = vec2(SKIP_BUTTON_SIZE, SKIP_BUTTON_SIZE);
        let prev_rect = Rect::from_center_size(row.center() - skip_offset, skip_size);
        let next_rect = Rect::from_center_size(row.center() + skip_offset, skip_size);

        // prev/next реагируют только на контур иконки, а не на весь квадрат вокруг неё
        let prev = widgets::icon_button(
            ui,
            skip_hit_rect(prev_rect),
            widgets::HitShape::Rect,
            "prev",
            has_track,
        );
        paint_skip(ui, prev_rect, false, !has_track, prev.hover);

        // У play чёткий круг, поэтому и наведение считаем только внутри него
        let play = widgets::icon_button(
            ui,
            play_rect,
            widgets::HitShape::Circle,
            "play_pause",
            has_track,
        );
        // 0.0 — треугольник play, 1.0 — полоски pause
        let morph = ui.ctx().animate_bool_with_time_and_easing(
            play.response.id,
            playing,
            PLAY_MORPH_SECS,
            egui::emath::easing::cubic_in_out,
        );
        paint_play_pause(ui, play_rect, morph, !has_track, play.hover);

        let next = widgets::icon_button(
            ui,
            skip_hit_rect(next_rect),
            widgets::HitShape::Rect,
            "next",
            has_track,
        );
        paint_skip(ui, next_rect, true, !has_track, next.hover);

        if prev.clicked {
            let result = self.controller.skip_prev();
            self.set_result(result);
        }
        if play.clicked {
            self.toggle_play();
        }
        if next.clicked {
            let result = self.controller.skip_next();
            self.set_result(result);
        }

        ui.add_space(1.0);
        self.render_progress_bar(ui, position, duration, has_track && duration > 0.0);
    }

    pub(super) fn render_progress_bar(
        &mut self,
        ui: &mut Ui,
        position: f32,
        duration: f32,
        enabled: bool,
    ) {
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
        painter.rect_filled(track, CornerRadius::same(255), SLIDER_TRACK);

        let thumb_x = track.left() + track.width() * fraction;
        let filled = Rect::from_min_max(track.min, pos2(thumb_x, track.max.y));
        painter.rect_filled(filled, CornerRadius::same(255), TEXT);

        // Ползунок не вылезает за края окна. max/min вместо clamp: на очень узком
        // окне левая граница может оказаться правее правой, и clamp паникует
        let thumb_x = thumb_x
            .min(rect.right() - SLIDER_THUMB_RADIUS)
            .max(rect.left() + SLIDER_THUMB_RADIUS);
        painter.circle_filled(pos2(thumb_x, rect.center().y), SLIDER_THUMB_RADIUS, TEXT);
    }
}
