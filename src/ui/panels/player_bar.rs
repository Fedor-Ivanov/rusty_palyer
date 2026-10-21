//! Нижняя панель: громкость, prev / play-pause / next, время и ползунок перемотки.

use eframe::egui::{
    self, Align2, CornerRadius, CursorIcon, FontId, Id, Order, Pos2, Rangef, Rect, Sense, Stroke,
    StrokeKind, Ui, Vec2, pos2, vec2,
};

use crate::{
    core::playback::PlayerState,
    ui::{
        action::Action,
        app::PlayerApp,
        components,
        components::format_secs,
        style::icons::{paint_play_pause, paint_skip, paint_volume, skip_hit_rect},
        style::theme::{
            BANNER_BORDER, BANNER_FILL, HOVER_FADE_SECS, SLIDER_TRACK, TEXT, TEXT_MUTED,
        },
    },
};

const PLAY_BUTTON_SIZE: f32 = 60.0;
const PLAY_MORPH_SECS: f32 = 0.25;
const SKIP_BUTTON_SIZE: f32 = 40.0;
// Зазор между кнопкой play и кнопками prev/next
const SKIP_BUTTON_GAP: f32 = 4.0;
const SLIDER_TRACK_HEIGHT: f32 = 5.0;
const SLIDER_THUMB_RADIUS: f32 = 8.0;
const VOLUME_BUTTON_SIZE: f32 = 24.0;
/// Вертикальный блок с ползунком громкости над кнопкой: размер, зазор до кнопки,
/// скругление, отступы сверху и снизу
const VOLUME_POPUP_SIZE: Vec2 = vec2(36.0, 140.0);
const VOLUME_POPUP_GAP: f32 = 6.0;
const VOLUME_POPUP_RADIUS: u8 = 12;
const VOLUME_POPUP_PADDING: f32 = 14.0;
const VOLUME_TRACK_WIDTH: f32 = 4.0;
const VOLUME_THUMB_RADIUS: f32 = 6.0;
/// Прокрутка колесом над громкостью: сколько точек прокрутки на всю шкалу
const VOLUME_SCROLL_RANGE: f32 = 400.0;

impl PlayerApp {
    pub(crate) fn render_controls(&mut self, ui: &mut Ui) {
        self.render_volume(ui);

        let player = self.state.controller.player();
        let has_track = player.current_track.is_some();
        let playing = player.state == PlayerState::Playing;
        let duration = player
            .current_track
            .as_ref()
            .map_or(0.0, |t| t.duration_secs as f32);

        let position = self
            .drag_secs
            .unwrap_or(self.state.controller.position_ms() as f32 / 1000.0)
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
        let prev = components::icon_button(
            ui,
            skip_hit_rect(prev_rect),
            components::HitShape::Rect,
            "prev",
            has_track,
        );
        paint_skip(ui, prev_rect, false, !has_track, prev.hover);

        // У play чёткий круг, поэтому и наведение считаем только внутри него
        let play = components::icon_button(
            ui,
            play_rect,
            components::HitShape::Circle,
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

        let next = components::icon_button(
            ui,
            skip_hit_rect(next_rect),
            components::HitShape::Rect,
            "next",
            has_track,
        );
        paint_skip(ui, next_rect, true, !has_track, next.hover);

        if prev.clicked {
            self.dispatch(Action::Previous);
        }
        if play.clicked {
            self.dispatch(Action::TogglePlay);
        }
        if next.clicked {
            self.dispatch(Action::Next);
        }

        ui.add_space(1.0);
        self.render_progress_bar(ui, position, duration, has_track && duration > 0.0);
    }

    pub(crate) fn render_progress_bar(
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
                    self.dispatch(Action::Seek(secs));
                }
            }
        }

        let fraction = if duration > 0.0 {
            (self.drag_secs.unwrap_or(position) / duration).clamp(0.0, 1.0)
        } else {
            0.0
        };

        paint_slider(ui, rect, fraction, SLIDER_TRACK_HEIGHT, SLIDER_THUMB_RADIUS);
    }

    /// Строка громкости: кнопка у правого края, над оставшимся временем. Клик — mute.
    /// При наведении над кнопкой появляется блок с ползунком; он не прячется, пока курсор
    /// на кнопке, на блоке или между ними и пока ползунок тянут
    fn render_volume(&mut self, ui: &mut Ui) {
        let (row, _) = ui.allocate_exact_size(
            vec2(ui.available_width(), VOLUME_BUTTON_SIZE),
            Sense::hover(),
        );
        let button_rect = Rect::from_min_size(
            pos2(row.right() - VOLUME_BUTTON_SIZE, row.top()),
            vec2(VOLUME_BUTTON_SIZE, VOLUME_BUTTON_SIZE),
        );
        let button = components::icon_button(
            ui,
            button_rect,
            components::HitShape::Circle,
            "volume",
            true,
        );
        let button_hovered = button.response.hovered();
        if button.clicked {
            self.dispatch(Action::ToggleMute);
        }
        paint_volume(
            ui,
            button_rect,
            self.state.controller.volume().audible(),
            button.hover,
        );

        // Блок по центру над кнопкой, но в пределах окна
        let window = ui.ctx().content_rect();
        let mut popup = Rect::from_center_size(
            pos2(
                button_rect.center().x,
                button_rect.top() - VOLUME_POPUP_GAP - VOLUME_POPUP_SIZE.y / 2.0,
            ),
            VOLUME_POPUP_SIZE,
        );
        popup = popup.translate(vec2((window.right() - popup.right()).min(0.0), 0.0));

        let popup_id = Id::unique("volume_popup");
        let slider_id = popup_id.with("slider");
        let was_open = ui.data(|data| {
            data.get_temp::<bool>(popup_id.with("open"))
                .unwrap_or(false)
        });
        // Прямоугольник, охватывающий кнопку и блок: курсор идёт от кнопки к блоку через зазор
        let in_zone = ui
            .ctx()
            .pointer_hover_pos()
            .is_some_and(|pos| button_rect.union(popup).expand(4.0).contains(pos));
        let open = button_hovered || (was_open && in_zone) || ui.ctx().is_being_dragged(slider_id);
        ui.data_mut(|data| data.insert_temp(popup_id.with("open"), open));
        let visibility = ui.ctx().animate_bool_with_time_and_easing(
            popup_id.with("fade"),
            open,
            HOVER_FADE_SECS,
            egui::emath::easing::cubic_out,
        );
        if visibility == 0.0 {
            return;
        }

        // Колесо мыши над кнопкой или блоком — тоже громкость
        if open && in_zone {
            let scroll = ui.input(|input| input.smooth_scroll_delta.y);
            if scroll != 0.0 {
                let level = self.state.controller.volume().audible() + scroll / VOLUME_SCROLL_RANGE;
                self.dispatch(Action::SetVolume(level));
            }
        }

        // Блок — поверх центральной области: он выше нижней панели
        let mut level = None;
        let audible = self.state.controller.volume().audible();
        egui::Area::new(popup_id)
            .order(Order::Foreground)
            .fixed_pos(popup.min)
            .constrain(false)
            .show(ui.ctx(), |ui| {
                ui.multiply_opacity(visibility);
                let (rect, _) = ui.allocate_exact_size(popup.size(), Sense::hover());
                ui.painter().rect(
                    rect,
                    VOLUME_POPUP_RADIUS,
                    BANNER_FILL,
                    Stroke::new(1.0, BANNER_BORDER),
                    StrokeKind::Inside,
                );

                // Тянуть можно за всю ширину блока, а не только за тонкую дорожку
                let slider = rect.shrink2(vec2(0.0, VOLUME_POPUP_PADDING));
                let response = ui
                    .interact(slider, slider_id, Sense::click_and_drag())
                    .on_hover_cursor(CursorIcon::PointingHand);
                let scale = vertical_scale(slider, VOLUME_THUMB_RADIUS);
                if (response.dragged() || response.clicked())
                    && let Some(pos) = response.interact_pointer_pos()
                {
                    // Снизу вверх: внизу тишина, вверху максимум
                    level = Some(((scale.max - pos.y) / scale.span()).clamp(0.0, 1.0));
                }
                paint_vertical_slider(ui, slider, level.unwrap_or(audible));
            });
        if let Some(level) = level {
            self.dispatch(Action::SetVolume(level));
        }
    }
}

/// Ползунок в `rect`: дорожка высотой `track_height` во всю ширину, заполненная до
/// `fraction` (0.0..=1.0), и круглый бегунок радиуса `thumb_radius`
fn paint_slider(ui: &Ui, rect: Rect, fraction: f32, track_height: f32, thumb_radius: f32) {
    let painter = ui.painter();
    let track = Rect::from_center_size(rect.center(), vec2(rect.width(), track_height));
    painter.rect_filled(track, CornerRadius::same(255), SLIDER_TRACK);

    let thumb_x = track.left() + track.width() * fraction;
    let filled = Rect::from_min_max(track.min, pos2(thumb_x, track.max.y));
    painter.rect_filled(filled, CornerRadius::same(255), TEXT);

    // Бегунок не вылезает за края. max/min вместо clamp: на очень узком
    // окне левая граница может оказаться правее правой, и clamp паникует
    let thumb_x = thumb_x
        .min(rect.right() - thumb_radius)
        .max(rect.left() + thumb_radius);
    painter.circle_filled(pos2(thumb_x, rect.center().y), thumb_radius, TEXT);
}

/// Где по высоте ходит центр бегунка вертикального ползунка в `rect`: от низа (0.0) до
/// верха (1.0), с отступом на радиус бегунка — он не вылезает за края
fn vertical_scale(rect: Rect, thumb_radius: f32) -> Rangef {
    Rangef::new(rect.top() + thumb_radius, rect.bottom() - thumb_radius)
}

/// Вертикальный ползунок громкости в `rect`: дорожка по центру, заполненная снизу до
/// `fraction` (0.0..=1.0), и круглый бегунок
fn paint_vertical_slider(ui: &Ui, rect: Rect, fraction: f32) {
    let painter = ui.painter();
    let track = Rect::from_center_size(rect.center(), vec2(VOLUME_TRACK_WIDTH, rect.height()));
    painter.rect_filled(track, CornerRadius::same(255), SLIDER_TRACK);

    let scale = vertical_scale(rect, VOLUME_THUMB_RADIUS);
    let thumb_y = scale.max - scale.span() * fraction;
    let filled = Rect::from_min_max(pos2(track.left(), thumb_y), track.max);
    painter.rect_filled(filled, CornerRadius::same(255), TEXT);
    painter.circle_filled(pos2(rect.center().x, thumb_y), VOLUME_THUMB_RADIUS, TEXT);
}
