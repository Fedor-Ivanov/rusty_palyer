//! Обложка альбома: одна функция рисования для всего интерфейса — плитки сетки
//! и обложки текущего трека. Сами обложки берутся из кэша (`ui::covers`).

use eframe::egui::{
    self, Color32, CornerRadius, Painter, Rect, Stroke, TextureHandle, Ui, pos2, vec2,
};

use crate::ui::{
    covers::Cover,
    theme::{HIGHLIGHT, TEXT_FAINT},
};

/// Обложка в квадрате `rect`: готовая — обрезанная до квадрата по центру (как CSS
/// `object-fit: cover`); пока читается — пустой квадрат (пластинка бы мелькала);
/// если в тегах обложки нет — заглушка с пластинкой
pub fn paint_cover(ui: &Ui, rect: Rect, cover: Cover<'_>, corner_radius: u8) {
    match cover {
        Cover::Ready(texture) => paint_square_image(ui, texture, rect, corner_radius),
        Cover::Missing => paint_placeholder(ui.painter(), rect, corner_radius),
        Cover::Loading => {
            ui.painter()
                .rect_filled(rect, CornerRadius::same(corner_radius), HIGHLIGHT);
        }
    }
}

fn paint_square_image(ui: &Ui, texture: &TextureHandle, rect: Rect, corner_radius: u8) {
    let aspect = texture.aspect_ratio();
    let uv_size = if aspect > 1.0 {
        vec2(1.0 / aspect, 1.0)
    } else {
        vec2(1.0, aspect)
    };
    egui::Image::new((texture.id(), rect.size()))
        .uv(Rect::from_center_size(pos2(0.5, 0.5), uv_size))
        .corner_radius(corner_radius)
        .paint_at(ui, rect);
}

/// Заглушка обложки: квадрат с виниловой пластинкой
fn paint_placeholder(painter: &Painter, rect: Rect, corner_radius: u8) {
    painter.rect_filled(rect, CornerRadius::same(corner_radius), HIGHLIGHT);

    // Размеры — доли стороны квадрата
    let center = rect.center();
    let r = |fraction: f32| rect.width().min(rect.height()) * fraction;
    let disc = Color32::from_rgb(0x12, 0x12, 0x14);
    let groove = Stroke::new(1.0, TEXT_FAINT.gamma_multiply(0.3));
    painter.circle_filled(center, r(0.42), disc);
    painter.circle_stroke(center, r(0.42), groove);
    for fraction in [0.37, 0.32, 0.27, 0.22] {
        painter.circle_stroke(center, r(fraction), groove);
    }
    painter.circle_filled(center, r(0.14), TEXT_FAINT);
    painter.circle_filled(center, r(0.025).max(2.0), disc);
}
