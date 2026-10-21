//! Обложка альбома: одна функция рисования для всего интерфейса — плитки сетки
//! и обложки текущего трека. Сами обложки берутся из кэша (`ui::covers`).

use eframe::egui::{
    self, Color32, CornerRadius, Mesh, Painter, Rect, Shape, Stroke, TextureHandle, Ui,
    epaint::Vertex, pos2, vec2,
};

use crate::ui::{
    covers::Cover,
    theme::{
        HIGHLIGHT, TEXT_FAINT, TRACK_BACKDROP_OPACITY, TRACK_BACKDROP_SHADE_BOTTOM,
        TRACK_BACKDROP_SHADE_TOP,
    },
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
    egui::Image::new((texture.id(), rect.size()))
        .uv(cover_uv(texture, rect))
        .corner_radius(corner_radius)
        .paint_at(ui, rect);
}

/// Фон из размытой обложки (`CoverSize::Backdrop`) во весь `rect`: обрезана по центру под
/// пропорции `rect`, полупрозрачная — под ней видно стекло окна, — и затемнена градиентом,
/// чтобы светлый текст поверх читался на любой обложке. От `fade_from` до низа `rect`
/// плавно растворяется в стекло — без резкой границы. `visibility` — проявление 0.0..=1.0.
/// Возвращает фигуру, а не рисует: фон кладут под уже нарисованное. Пока обложка читается
/// или её нет — `None`
pub fn backdrop_shape(
    rect: Rect,
    fade_from: f32,
    cover: Cover<'_>,
    visibility: f32,
) -> Option<Shape> {
    /// Рядов вершин в растворении: прозрачность между рядами меняется линейно,
    /// по рядам — по плавной кривой
    const FADE_STEPS: usize = 12;
    let Cover::Ready(texture) = cover else {
        return None;
    };
    let uv = cover_uv(texture, rect);
    let fade_from = fade_from.clamp(rect.top(), rect.bottom());

    // Ряды вершин сверху вниз: высота и доля полной видимости
    let rows = std::iter::once((rect.top(), 1.0)).chain((0..=FADE_STEPS).map(|step| {
        let t = step as f32 / FADE_STEPS as f32;
        let smooth = t * t * (3.0 - 2.0 * t);
        (egui::lerp(fade_from..=rect.bottom(), t), 1.0 - smooth)
    }));

    let mut image = Mesh::with_texture(texture.id());
    let mut shade = Mesh::default();
    for (y, alpha) in rows {
        let v = egui::lerp(uv.y_range(), (y - rect.top()) / rect.height());
        let tint = Color32::WHITE.gamma_multiply(TRACK_BACKDROP_OPACITY * visibility * alpha);
        for (x, u) in [(rect.left(), uv.left()), (rect.right(), uv.right())] {
            image.vertices.push(Vertex {
                pos: pos2(x, y),
                uv: pos2(u, v),
                color: tint,
            });
        }
        // Затемнение — от верха до начала растворения, дальше растворяется вместе с фоном
        let shade_t = ((y - rect.top()) / (fade_from - rect.top()).max(1.0)).min(1.0);
        let shade_color = TRACK_BACKDROP_SHADE_TOP
            .lerp_to_gamma(TRACK_BACKDROP_SHADE_BOTTOM, shade_t)
            .gamma_multiply(visibility * alpha);
        shade.colored_vertex(pos2(rect.left(), y), shade_color);
        shade.colored_vertex(pos2(rect.right(), y), shade_color);
    }
    // Между соседними рядами — по два треугольника
    for mesh in [&mut image, &mut shade] {
        let rows = mesh.vertices.len() as u32 / 2;
        for row in 0..rows - 1 {
            let i = row * 2;
            mesh.add_triangle(i, i + 1, i + 3);
            mesh.add_triangle(i, i + 3, i + 2);
        }
    }
    Some(Shape::Vec(vec![image.into(), shade.into()]))
}

/// Часть текстуры, которая заполняет `rect` без искажений: обрезка по центру под пропорции
/// `rect` (как CSS `object-fit: cover`)
fn cover_uv(texture: &TextureHandle, rect: Rect) -> Rect {
    // Во сколько раз картинка шире области (по пропорциям)
    let ratio = texture.aspect_ratio() / (rect.width() / rect.height().max(1.0));
    let uv_size = if ratio > 1.0 {
        vec2(1.0 / ratio, 1.0)
    } else {
        vec2(1.0, ratio)
    };
    Rect::from_center_size(pos2(0.5, 0.5), uv_size)
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
