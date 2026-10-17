use eframe::egui::{
    self, Align, Color32, CornerRadius, CursorIcon, Label, Layout, Pos2, Rect, Response, RichText,
    Sense, Stroke, TextureHandle, Ui, vec2,
};
use std::hash::{DefaultHasher, Hash, Hasher};

use crate::theme::{GRAY_1, GRAY_3, GRAY_4};

const COVER_MAX_SIZE: f32 = 180.0;
pub const HOVER_FADE_SECS: f32 = 0.15;
pub const TRACK_ACTIVE_FADE_SECS: f32 = 0.2;
const BLOCK_TITLE_SIZE: f32 = 16.0;

pub fn render_album_cover(ui: &mut Ui, cover_data: Option<&[u8]>) {
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
        ui.image((texture.id(), size * scale));
    } else {
        // Временная заглушка: квадрат с виниловой пластинкой
        let side = ui.available_width().min(COVER_MAX_SIZE);
        let (rect, _) = ui.allocate_exact_size(vec2(side, side), Sense::hover());
        let painter = ui.painter();
        painter.rect_filled(rect, CornerRadius::same(8), GRAY_1);

        // Размеры — доли стороны квадрата
        let center = rect.center();
        let r = |fraction: f32| side * fraction;
        let groove = Stroke::new(1.0, GRAY_3.gamma_multiply(0.15));
        painter.circle_filled(center, r(0.42), GRAY_4);
        painter.circle_stroke(center, r(0.42), groove);
        for fraction in [0.37, 0.32, 0.27, 0.22] {
            painter.circle_stroke(center, r(fraction), groove);
        }
        painter.circle_filled(center, r(0.14), GRAY_3.gamma_multiply(0.15));
        painter.circle_filled(center, r(0.025).max(2.0), GRAY_4);
    }
}

/// Форма области, в которой кнопка реагирует на курсор
pub enum HitShape {
    Rect,
    /// Круг, вписанный в прямоугольник кнопки
    Circle,
}

pub struct IconButton {
    pub response: Response,
    pub clicked: bool,
    /// Плавный hover для отрисовки: 0.0 — без наведения, 1.0 — наведено
    pub hover: f32,
}

/// Кнопка-иконка: клик, курсор-рука и плавный hover — только внутри `shape`
pub fn icon_button(
    ui: &mut Ui,
    rect: Rect,
    shape: HitShape,
    id_salt: &str,
    enabled: bool,
) -> IconButton {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let response = ui.interact(rect, ui.scope_id().with(id_salt), sense);

    // egui проверяет попадание по прямоугольнику, круг уточняем сами
    let inside = |pos: Pos2| match shape {
        HitShape::Rect => true,
        HitShape::Circle => pos.distance(rect.center()) <= rect.width() / 2.0,
    };
    let hovered = enabled && response.hover_pos().is_some_and(inside);
    let clicked = response.clicked() && response.interact_pointer_pos().is_some_and(inside);

    if hovered {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let hover = ui.ctx().animate_bool_with_time_and_easing(
        response.id.with("hover"),
        hovered,
        HOVER_FADE_SECS,
        egui::emath::easing::cubic_out,
    );

    IconButton {
        response,
        clicked,
        hover,
    }
}

/// Кнопка-переключатель в панели списка. Возвращает клик и плавные (0.0..=1.0)
/// значения включения и наведения для отрисовки иконки.
pub fn toggle_button(
    ui: &mut Ui,
    rect: Rect,
    id_salt: &str,
    on: bool,
    tooltip: &str,
) -> (bool, f32, f32) {
    let button = icon_button(ui, rect, HitShape::Circle, id_salt, true);
    let active = ui.ctx().animate_bool_with_time_and_easing(
        button.response.id.with("active"),
        on,
        TRACK_ACTIVE_FADE_SECS,
        egui::emath::easing::cubic_in_out,
    );
    let clicked = button.clicked;
    button.response.on_hover_text(tooltip);
    (clicked, active, button.hover)
}

/// Заголовок блока (панели) с выравниванием `align`: `Align::Min` — слева,
/// `Align::Center` — по центру, `Align::Max` — справа.
/// Возвращает прямоугольник строки во всю ширину — по нему можно поставить кнопки
/// рядом с заголовком.
pub fn block_title(ui: &mut Ui, text: &str, align: Align, color: Color32) -> Rect {
    ui.with_layout(Layout::top_down(align), |ui| {
        // Строка во всю ширину при любом выравнивании, иначе прямоугольник
        // был бы размером с текст
        ui.set_min_width(ui.available_width());
        let title = RichText::new(text).size(BLOCK_TITLE_SIZE).color(color);
        ui.add(Label::new(title).selectable(false));
    })
    .response
    .rect
}
