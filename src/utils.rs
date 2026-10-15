use crate::theme::FG;
use eframe::egui::{self, Pos2, Rect, Shape, Stroke, Ui, vec2};

/// Секунды в `мм:сс` или `чч:мм:сс`.
pub fn format_secs(seconds: f32) -> String {
    let seconds = seconds.max(0.0) as u64;
    let (h, m, s) = (seconds / 3600, seconds % 3600 / 60, seconds % 60);

    if h > 0 {
        format!("{h:02}:{m:02}:{s:02}")
    } else {
        format!("{m:02}:{s:02}")
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
pub fn paint_play_pause(ui: &Ui, rect: Rect, morph: f32, disabled: bool, hovered: bool) {
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
