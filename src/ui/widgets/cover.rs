//! Обложка альбома или заглушка с пластинкой.

use eframe::egui::{self, Color32, CornerRadius, Sense, Stroke, TextureHandle, Ui, vec2};
use std::hash::{DefaultHasher, Hash, Hasher};

use crate::ui::theme::{HIGHLIGHT, TEXT_FAINT};

const COVER_MAX_SIZE: f32 = 250.0;
const COVER_RADIUS: u8 = 12;

pub fn album_cover(ui: &mut Ui, cover_data: Option<&[u8]>) {
    if let Some(cover) = cover_data {
        let cache_id = egui::Id::unique("album_cover");
        let cover_hash = {
            let mut hasher = DefaultHasher::new();
            cover.hash(&mut hasher);
            hasher.finish()
        };
        let cached = ui
            .ctx()
            .data(|data| data.get_temp::<(u64, TextureHandle)>(cache_id))
            .filter(|(hash, _)| *hash == cover_hash)
            .map(|(_, texture)| texture);

        let texture = match cached {
            Some(texture) => texture,
            None => {
                let Ok(image) = image::load_from_memory(cover) else {
                    return;
                };
                let image = image.to_rgba8();
                let size = [image.width() as usize, image.height() as usize];
                let texture = ui.ctx().load_texture(
                    "album_cover",
                    egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw()),
                    egui::TextureOptions::LINEAR,
                );
                ui.ctx().data_mut(|data| {
                    data.insert_temp(cache_id, (cover_hash, texture.clone()));
                });
                texture
            }
        };

        let size = texture.size_vec2();
        let max_size = vec2(ui.available_width(), COVER_MAX_SIZE);
        let scale = (max_size / size).min_elem().min(1.0);
        ui.add(egui::Image::new((texture.id(), size * scale)).corner_radius(COVER_RADIUS));
    } else {
        // Временная заглушка: квадрат с виниловой пластинкой
        let side = ui.available_width().min(COVER_MAX_SIZE);
        let (rect, _) = ui.allocate_exact_size(vec2(side, side), Sense::hover());
        let painter = ui.painter();
        painter.rect_filled(rect, CornerRadius::same(COVER_RADIUS), HIGHLIGHT);

        // Размеры — доли стороны квадрата
        let center = rect.center();
        let r = |fraction: f32| side * fraction;
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
}
