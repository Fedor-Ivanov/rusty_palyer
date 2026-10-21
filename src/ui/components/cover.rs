//! Обложка альбома: рисование для всего интерфейса — плитки сетки и обложки текущего трека
//! с плавной сменой (`CoverFade`), фон из размытой обложки. Сами обложки берутся из кэша
//! (`ui::covers`).

use eframe::egui::{
    self, Color32, CornerRadius, Mesh, Painter, Rect, Shape, Stroke, TextureHandle, Ui,
    epaint::Vertex, pos2, vec2,
};

use crate::ui::{
    covers::Cover,
    style::theme::{
        COVER_CROSSFADE_SECS, HIGHLIGHT, TEXT_FAINT, TRACK_BACKDROP_OPACITY,
        TRACK_BACKDROP_SHADE_BOTTOM, TRACK_BACKDROP_SHADE_TOP,
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

/// Что видно на месте обложки
#[derive(Clone)]
enum Shown {
    Image(TextureHandle),
    /// В тегах обложки нет — заглушка (а у фона — просто стекло)
    Missing,
}

impl Shown {
    fn same(&self, other: &Shown) -> bool {
        match (self, other) {
            (Shown::Image(a), Shown::Image(b)) => a.id() == b.id(),
            (Shown::Missing, Shown::Missing) => true,
            _ => false,
        }
    }

    fn texture(&self) -> Option<&TextureHandle> {
        match self {
            Shown::Image(texture) => Some(texture),
            Shown::Missing => None,
        }
    }
}

/// Плавная смена обложки — для текущего трека, где картинка меняется вместе с треком.
/// Пока новая обложка читается, видна прежняя: при переключении трека картинка не пропадает
/// ни на кадр. Готова — проявляется поверх прежней за `COVER_CROSSFADE_SECS`; у треков
/// одного альбома обложка та же, и переход не виден вовсе. Держит текстуры у себя: прежняя
/// не пропадёт, даже если кэш обложек её уже вытеснил
#[derive(Default)]
pub struct CoverFade {
    current: Option<Shown>,
    previous: Option<Shown>,
    /// Когда началась смена — время egui в секундах
    changed_at: f64,
}

impl CoverFade {
    /// Учитывает ответ кэша обложек за этот кадр и возвращает, насколько новая обложка
    /// уже проявилась: 0.0..=1.0. «Ещё читается» ничего не меняет
    pub fn update(&mut self, ui: &Ui, cover: Cover<'_>) -> f32 {
        let now = ui.input(|input| input.time);
        let next = match cover {
            Cover::Ready(texture) => Some(Shown::Image(texture.clone())),
            Cover::Missing => Some(Shown::Missing),
            Cover::Loading => None,
        };
        if let Some(next) = next
            && !self
                .current
                .as_ref()
                .is_some_and(|current| current.same(&next))
        {
            self.previous = self.current.replace(next);
            self.changed_at = now;
        }

        let t = ((now - self.changed_at) as f32 / COVER_CROSSFADE_SECS).clamp(0.0, 1.0);
        if t < 1.0 {
            ui.ctx().request_repaint();
        } else {
            self.previous = None;
        }
        // Плавно в начале и в конце
        t * t * (3.0 - 2.0 * t)
    }

    /// Обложка в квадрате `rect`: прежняя, поверх неё проявляется новая (`progress` — из
    /// `update`). Пока ещё ничего не прочитано — пустой квадрат, как у `paint_cover`
    pub fn paint(&self, ui: &Ui, rect: Rect, progress: f32, corner_radius: u8) {
        match &self.previous {
            Some(previous) => paint_shown(ui, rect, previous, 1.0, corner_radius),
            None => {
                ui.painter()
                    .rect_filled(rect, CornerRadius::same(corner_radius), HIGHLIGHT);
            }
        }
        if let Some(current) = &self.current {
            paint_shown(ui, rect, current, progress, corner_radius);
        }
    }

    /// Фон из размытой обложки (`CoverSize::Backdrop`) во весь `rect`: обрезана по центру
    /// под пропорции `rect`, полупрозрачная — под ней видно стекло окна, — и затемнена
    /// градиентом, чтобы светлый текст поверх читался на любой обложке. От `fade_from` до
    /// низа `rect` плавно растворяется в стекло. При смене обложки прежний фон растворяется,
    /// новый проявляется (`progress` — из `update`). Возвращает фигуру, а не рисует: фон
    /// кладут под уже нарисованное. Обложки нет — `None`, видно просто стекло
    pub fn backdrop(&self, rect: Rect, fade_from: f32, progress: f32) -> Option<Shape> {
        let rows = backdrop_rows(rect, fade_from);
        let previous = self.previous.as_ref().and_then(Shown::texture);
        let current = self.current.as_ref().and_then(Shown::texture);
        // Фон полупрозрачный: если просто гасить прежний и проявлять новый, в середине
        // перехода два полупрозрачных слоя пропускают больше стекла, и фон «моргает».
        // Поэтому новый слой проявляется ровно настолько, чтобы вместе с гаснущим
        // прежним плотность фона не менялась — меняется только цвет
        let previous_visibility = 1.0 - progress;
        let current_visibility = if previous.is_some() {
            let below = TRACK_BACKDROP_OPACITY * previous_visibility;
            (1.0 - (1.0 - TRACK_BACKDROP_OPACITY) / (1.0 - below)) / TRACK_BACKDROP_OPACITY
        } else {
            progress
        };
        let layers = [
            (previous, previous_visibility),
            (current, current_visibility),
        ];
        let mut shapes = Vec::new();
        // Затемнение — одно на оба слоя: видно настолько, насколько виден хоть какой-то фон
        let mut shade = 0.0;
        for (texture, visibility) in layers {
            if let Some(texture) = texture {
                shapes.push(backdrop_image(rect, &rows, texture, visibility).into());
                shade += visibility;
            }
        }
        if shapes.is_empty() {
            return None;
        }
        shapes.push(backdrop_shade(rect, fade_from, &rows, shade.min(1.0)).into());
        Some(Shape::Vec(shapes))
    }
}

/// Обложка с прозрачностью `opacity`: картинка или заглушка с пластинкой
fn paint_shown(ui: &Ui, rect: Rect, shown: &Shown, opacity: f32, corner_radius: u8) {
    match shown {
        Shown::Image(texture) => {
            egui::Image::new((texture.id(), rect.size()))
                .uv(cover_uv(texture, rect))
                .corner_radius(corner_radius)
                .tint(Color32::WHITE.gamma_multiply(opacity))
                .paint_at(ui, rect);
        }
        Shown::Missing => {
            let mut painter = ui.painter().clone();
            painter.multiply_opacity(opacity);
            paint_placeholder(&painter, rect, corner_radius);
        }
    }
}

/// Ряды вершин фона сверху вниз: высота и доля полной видимости. До `fade_from` фон виден
/// целиком, дальше до низа `rect` растворяется по плавной кривой: прозрачность между
/// рядами меняется линейно, рядов достаточно, чтобы кривая выглядела гладкой
fn backdrop_rows(rect: Rect, fade_from: f32) -> Vec<(f32, f32)> {
    const FADE_STEPS: usize = 12;
    let fade_from = fade_from.clamp(rect.top(), rect.bottom());
    std::iter::once((rect.top(), 1.0))
        .chain((0..=FADE_STEPS).map(|step| {
            let t = step as f32 / FADE_STEPS as f32;
            let smooth = t * t * (3.0 - 2.0 * t);
            (egui::lerp(fade_from..=rect.bottom(), t), 1.0 - smooth)
        }))
        .collect()
}

/// Размытая обложка фона с видимостью `visibility`
fn backdrop_image(
    rect: Rect,
    rows: &[(f32, f32)],
    texture: &TextureHandle,
    visibility: f32,
) -> Mesh {
    let uv = cover_uv(texture, rect);
    let mut mesh = Mesh::with_texture(texture.id());
    for &(y, alpha) in rows {
        let v = egui::lerp(uv.y_range(), (y - rect.top()) / rect.height());
        let tint = Color32::WHITE.gamma_multiply(TRACK_BACKDROP_OPACITY * visibility * alpha);
        for (x, u) in [(rect.left(), uv.left()), (rect.right(), uv.right())] {
            mesh.vertices.push(Vertex {
                pos: pos2(x, y),
                uv: pos2(u, v),
                color: tint,
            });
        }
    }
    connect_rows(&mut mesh);
    mesh
}

/// Затемнение фона ради читаемости текста: от верха до `fade_from` — градиентом от
/// сильного к слабому, дальше растворяется вместе с фоном
fn backdrop_shade(rect: Rect, fade_from: f32, rows: &[(f32, f32)], visibility: f32) -> Mesh {
    let mut mesh = Mesh::default();
    for &(y, alpha) in rows {
        let t = ((y - rect.top()) / (fade_from - rect.top()).max(1.0)).min(1.0);
        let color = TRACK_BACKDROP_SHADE_TOP
            .lerp_to_gamma(TRACK_BACKDROP_SHADE_BOTTOM, t)
            .gamma_multiply(visibility * alpha);
        mesh.colored_vertex(pos2(rect.left(), y), color);
        mesh.colored_vertex(pos2(rect.right(), y), color);
    }
    connect_rows(&mut mesh);
    mesh
}

/// Вершины идут парами (левая, правая) по рядам — между соседними рядами по два треугольника
fn connect_rows(mesh: &mut Mesh) {
    let rows = mesh.vertices.len() as u32 / 2;
    for row in 0..rows.saturating_sub(1) {
        let i = row * 2;
        mesh.add_triangle(i, i + 1, i + 3);
        mesh.add_triangle(i, i + 3, i + 2);
    }
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
