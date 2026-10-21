//! Горизонтальный ползунок: перемотка в нижней панели, прозрачность стекла в настройках.

use eframe::egui::{CornerRadius, CursorIcon, Rect, Sense, Ui, pos2, vec2};

use crate::ui::style::theme::{SLIDER_TRACK, TEXT};

/// Ползунок значения 0.0..=1.0 шириной `width`. Возвращает новое значение, пока его тянут
/// или по клику
pub fn slider(
    ui: &mut Ui,
    width: f32,
    value: f32,
    track_height: f32,
    thumb_radius: f32,
) -> Option<f32> {
    let (rect, response) =
        ui.allocate_exact_size(vec2(width, thumb_radius * 2.0), Sense::click_and_drag());
    let response = response.on_hover_cursor(CursorIcon::PointingHand);
    // Центр бегунка ходит от края до края с отступом на радиус — как он нарисован
    let left = rect.left() + thumb_radius;
    let span = (rect.width() - 2.0 * thumb_radius).max(1.0);
    let changed = if response.dragged() || response.clicked() {
        response
            .interact_pointer_pos()
            .map(|pos| ((pos.x - left) / span).clamp(0.0, 1.0))
    } else {
        None
    };
    paint_slider(
        ui,
        rect,
        changed.unwrap_or(value),
        track_height,
        thumb_radius,
    );
    changed
}

/// Ползунок в `rect`: дорожка высотой `track_height` во всю ширину, заполненная до
/// `fraction` (0.0..=1.0), и круглый бегунок радиуса `thumb_radius`
pub fn paint_slider(ui: &Ui, rect: Rect, fraction: f32, track_height: f32, thumb_radius: f32) {
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
