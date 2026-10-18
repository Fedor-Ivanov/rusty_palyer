//! Иконки кнопок плеера, нарисованные фигурами: play/pause, prev/next, shuffle, repeat, «+»,
//! громкость.

use super::theme::{HIGHLIGHT_STRONG, ICON, ICON_DISABLED, ICON_OFF, ICON_SOFT};
use eframe::egui::{self, Color32, Pos2, Rect, Shape, Stroke, Ui, pos2, vec2};

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
/// Непрозрачность тёмного круга кнопки play поверх обложки: в покое и при наведении
const OVERLAY_PLAY_ALPHA: u8 = 150;
const OVERLAY_PLAY_HOVER_ALPHA: u8 = 200;
const PAUSE_CORNER_RADIUS: f32 = 1.0;
/// Непрозрачность белого круга под иконкой play: в покое и при наведении
const PLAY_BG_OPACITY: f32 = 0.15;
const PLAY_BG_HOVER_OPACITY: f32 = 0.25;

// Иконка «следующий трек»: два треугольника и тонкая полоска. «Предыдущий» — её зеркальное
// отражение. Между фигурами есть зазор, чтобы на полупрозрачной иконке не было видно наложения.
const SKIP_TRIANGLES: [[(f32, f32); 3]; 2] = [
    [(3.5, 7.0), (9.5, 12.0), (3.5, 17.0)],
    [(12.0, 7.0), (18.0, 12.0), (12.0, 17.0)],
];
// Полоска — отрезок, раздутый на радиус скругления, поэтому её толщина — 2 × радиус
const SKIP_BAR: [(f32, f32); 2] = [(20.5, 7.0), (20.5, 17.0)];
const SKIP_CORNER_RADIUS: f32 = 1.0;
/// Запас вокруг иконки prev/next, в котором она ещё реагирует на курсор, в точках экрана
const SKIP_HIT_PADDING: f32 = 4.0;

// Иконка «+»: два отрезка, раздутых на радиус скругления
const ADD_BARS: [[(f32, f32); 2]; 2] = [[(8.0, 12.0), (16.0, 12.0)], [(12.0, 8.0), (12.0, 16.0)]];
const ADD_CORNER_RADIUS: f32 = 1.0;

// Иконки-переключатели (shuffle, repeat) нарисованы линиями толщиной ICON_STROKE
// со стрелками-треугольниками на концах
const ICON_STROKE: f32 = 2.0;
const ARROW_CORNER_RADIUS: f32 = 0.5;
// Shuffle: две перекрещенные линии, обе со стрелкой вправо
const SHUFFLE_LINES: [[(f32, f32); 4]; 2] = [
    [(4.0, 8.0), (8.0, 8.0), (15.0, 16.0), (17.0, 16.0)],
    [(4.0, 16.0), (8.0, 16.0), (15.0, 8.0), (17.0, 8.0)],
];
const SHUFFLE_ARROWS: [[(f32, f32); 3]; 2] = [
    [(17.0, 13.0), (20.0, 16.0), (17.0, 19.0)],
    [(17.0, 5.0), (20.0, 8.0), (17.0, 11.0)],
];
// Repeat: петля из двух скоб — верхняя со стрелкой вправо, нижняя со стрелкой влево
const REPEAT_LINES: [[(f32, f32); 3]; 2] = [
    [(5.0, 12.0), (5.0, 8.0), (16.5, 8.0)],
    [(19.0, 12.0), (19.0, 16.0), (7.5, 16.0)],
];
const REPEAT_ARROWS: [[(f32, f32); 3]; 2] = [
    [(16.5, 5.0), (20.0, 8.0), (16.5, 11.0)],
    [(7.5, 13.0), (4.0, 16.0), (7.5, 19.0)],
];
// «1» внутри петли для повтора одного трека — тоньше, чтобы поместиться
const REPEAT_ONE_MARK: [(f32, f32); 3] = [(10.75, 11.25), (12.0, 10.0), (12.0, 14.0)];
const REPEAT_ONE_STROKE: f32 = 1.5;

// Иконка громкости: динамик из двух выпуклых фигур (корпус и раструб), справа от него —
// волны-дуги с центром у раструба или крестик, если звук выключен
const SPEAKER_SHAPES: [&[(f32, f32)]; 2] = [
    &[(3.5, 10.0), (6.5, 10.0), (6.5, 14.0), (3.5, 14.0)],
    &[(6.0, 10.0), (10.5, 6.0), (10.5, 18.0), (6.0, 14.0)],
];
const SPEAKER_CORNER_RADIUS: f32 = 1.0;
const VOLUME_WAVE_CENTER: (f32, f32) = (11.0, 12.0);
const VOLUME_WAVE_RADII: [f32; 2] = [3.5, 7.0];
/// Половина угла дуги волны, в радианах (±45°)
const VOLUME_WAVE_HALF_ANGLE: f32 = std::f32::consts::FRAC_PI_4;
const VOLUME_WAVE_STROKE: f32 = 1.8;
/// Громкость, начиная с которой видна вторая волна
const VOLUME_LOUD: f32 = 0.5;
const MUTE_CROSS: [[(f32, f32); 2]; 2] = [[(14.5, 9.5), (19.5, 14.5)], [(14.5, 14.5), (19.5, 9.5)]];

/// Круг-подложка круглой кнопки. Возвращает цвет, которым рисовать иконку.
fn paint_button_bg(ui: &Ui, rect: Rect, disabled: bool, hover: f32) -> Color32 {
    let alpha = if disabled { 0.4 } else { 1.0 };
    let bg_opacity = egui::lerp(PLAY_BG_OPACITY..=PLAY_BG_HOVER_OPACITY, hover);
    ui.painter().circle_filled(
        rect.center(),
        rect.width() / 2.0,
        Color32::WHITE.gamma_multiply(bg_opacity * alpha),
    );
    if disabled { ICON_DISABLED } else { ICON }
}

/// Заливает фигуру, заданную в координатах иконки 24×24, со скруглением `radius`
fn fill_icon_shape(ui: &Ui, rect: Rect, points: &[Pos2], radius: f32, color: Color32) {
    let scale = rect.width() / 24.0;
    let points: Vec<Pos2> = points
        .iter()
        .map(|p| rect.min + p.to_vec2() * scale)
        .collect();
    let outline = rounded_convex(&points, radius * scale);
    ui.painter()
        .add(Shape::convex_polygon(outline, color, Stroke::NONE));
}

/// `morph`: 0.0 — play, 1.0 — pause, между ними — промежуточная форма.
/// `hover`: 0.0 — без наведения, 1.0 — наведено, между ними — переход.
pub fn paint_play_pause(ui: &Ui, rect: Rect, morph: f32, disabled: bool, hover: f32) {
    let icon_color = paint_button_bg(ui, rect, disabled, hover);

    if morph <= 0.0 {
        // Целиком, а не половинками: иначе на полупрозрачной иконке видно их наложение
        let triangle = PLAY_TRIANGLE.map(|(x, y)| pos2(x, y));
        fill_icon_shape(ui, rect, &triangle, PLAY_CORNER_RADIUS, icon_color);
    } else {
        let radius = egui::lerp(PLAY_CORNER_RADIUS..=PAUSE_CORNER_RADIUS, morph);
        for (from, to) in PLAY_HALVES.iter().zip(&PAUSE_BARS) {
            let points: Vec<Pos2> = from
                .iter()
                .zip(to)
                .map(|(&(ax, ay), &(bx, by))| pos2(ax, ay).lerp(pos2(bx, by), morph))
                .collect();
            fill_icon_shape(ui, rect, &points, radius, icon_color);
        }
    }
}

/// Кнопка «следующий трек» (`forward`) или «предыдущий». Без круга-подложки:
/// при наведении иконка плавно светлеет.
/// `hover`: 0.0 — без наведения, 1.0 — наведено, между ними — переход.
pub fn paint_skip(ui: &Ui, rect: Rect, forward: bool, disabled: bool, hover: f32) {
    let icon_color = if disabled {
        ICON_DISABLED
    } else {
        ICON_SOFT.lerp_to_gamma(ICON, hover)
    };

    // «Назад» — зеркальное отражение «вперёд» по горизонтали
    let point = |(x, y): (f32, f32)| {
        if forward {
            pos2(x, y)
        } else {
            pos2(24.0 - x, y)
        }
    };
    for triangle in SKIP_TRIANGLES {
        fill_icon_shape(
            ui,
            rect,
            &triangle.map(point),
            SKIP_CORNER_RADIUS,
            icon_color,
        );
    }
    fill_icon_shape(
        ui,
        rect,
        &SKIP_BAR.map(point),
        SKIP_CORNER_RADIUS,
        icon_color,
    );
}

/// Область, в которой кнопка prev/next реагирует на курсор: контур иконки
/// с небольшим запасом. Иконка симметрична, поэтому область одна для обеих кнопок.
pub fn skip_hit_rect(rect: Rect) -> Rect {
    let scale = rect.width() / 24.0;
    let points = SKIP_TRIANGLES
        .iter()
        .flatten()
        .chain(&SKIP_BAR)
        .map(|&(x, y)| rect.min + vec2(x, y) * scale);
    Rect::from_points(&points.collect::<Vec<_>>())
        .expand(SKIP_CORNER_RADIUS * scale + SKIP_HIT_PADDING)
}

/// Светлый круг под иконкой в панели: плавно проявляется при наведении
fn paint_hover_circle(ui: &Ui, rect: Rect, hover: f32) {
    ui.painter().circle_filled(
        rect.center(),
        rect.width() / 2.0,
        HIGHLIGHT_STRONG.gamma_multiply(hover),
    );
}

/// Ломаная в координатах иконки 24×24 с круглыми концами
fn stroke_icon_path(ui: &Ui, rect: Rect, points: &[(f32, f32)], width: f32, color: Color32) {
    let scale = rect.width() / 24.0;
    let points: Vec<Pos2> = points
        .iter()
        .map(|&(x, y)| rect.min + vec2(x, y) * scale)
        .collect();
    let stroke = Stroke::new(width * scale, color);
    // egui рисует линии с прямыми торцами — скругляем их кругами
    for end in [points[0], points[points.len() - 1]] {
        ui.painter().circle_filled(end, stroke.width / 2.0, color);
    }
    ui.painter().add(Shape::line(points, stroke));
}

/// Цвет иконки-переключателя: выключен — приглушённый, включён — светлый
fn toggle_color(active: f32) -> Color32 {
    ICON_OFF.lerp_to_gamma(ICON, active)
}

/// Кнопка «+» в заголовке библиотеки: светлый плюс, при наведении под ним плавно
/// проявляется светлый круг.
/// `hover`: 0.0 — без наведения, 1.0 — наведено, между ними — переход.
pub fn paint_add(ui: &Ui, rect: Rect, hover: f32) {
    paint_hover_circle(ui, rect, hover);
    for bar in ADD_BARS {
        let points = bar.map(|(x, y)| pos2(x, y));
        fill_icon_shape(ui, rect, &points, ADD_CORNER_RADIUS, ICON);
    }
}

/// Переключатель случайного порядка.
/// `active` и `hover`: 0.0..=1.0, плавные переходы включения и наведения.
pub fn paint_shuffle(ui: &Ui, rect: Rect, active: f32, hover: f32) {
    paint_hover_circle(ui, rect, hover);
    let color = toggle_color(active);
    for line in SHUFFLE_LINES {
        stroke_icon_path(ui, rect, &line, ICON_STROKE, color);
    }
    for arrow in SHUFFLE_ARROWS {
        let points = arrow.map(|(x, y)| pos2(x, y));
        fill_icon_shape(ui, rect, &points, ARROW_CORNER_RADIUS, color);
    }
}

/// Переключатель повтора: всего списка или, с `one`, одного трека (с «1» внутри петли).
/// `active` и `hover`: 0.0..=1.0, плавные переходы включения и наведения.
pub fn paint_repeat(ui: &Ui, rect: Rect, one: bool, active: f32, hover: f32) {
    paint_hover_circle(ui, rect, hover);
    let color = toggle_color(active);
    for line in REPEAT_LINES {
        stroke_icon_path(ui, rect, &line, ICON_STROKE, color);
    }
    for arrow in REPEAT_ARROWS {
        let points = arrow.map(|(x, y)| pos2(x, y));
        fill_icon_shape(ui, rect, &points, ARROW_CORNER_RADIUS, color);
    }
    if one {
        stroke_icon_path(ui, rect, &REPEAT_ONE_MARK, REPEAT_ONE_STROKE, color);
    }
}

/// Кнопка play поверх обложки: тёмный полупрозрачный круг с белым треугольником — видна
/// на любой обложке. `visibility` — проявление вместе с наведением на плитку (0.0..=1.0),
/// `hover` — наведение на саму кнопку: круг темнеет
pub fn paint_overlay_play(ui: &Ui, rect: Rect, visibility: f32, hover: f32) {
    let circle = Color32::from_black_alpha(OVERLAY_PLAY_ALPHA)
        .lerp_to_gamma(Color32::from_black_alpha(OVERLAY_PLAY_HOVER_ALPHA), hover);
    ui.painter().circle_filled(
        rect.center(),
        rect.width() / 2.0,
        circle.gamma_multiply(visibility),
    );
    let triangle = PLAY_TRIANGLE.map(|(x, y)| pos2(x, y));
    fill_icon_shape(
        ui,
        rect,
        &triangle,
        PLAY_CORNER_RADIUS,
        ICON.gamma_multiply(visibility),
    );
}

/// Лупа в поле поиска: кружок и ручка вниз вправо. Тонкие линии — иконка рядом с текстом
pub fn paint_search(ui: &Ui, rect: Rect, color: Color32) {
    let stroke = Stroke::new(1.4, color);
    let radius = rect.width() * 0.3;
    let center = rect.center() - vec2(rect.width() * 0.08, rect.height() * 0.08);
    let painter = ui.painter();
    painter.circle_stroke(center, radius, stroke);
    let direction = vec2(1.0, 1.0).normalized();
    painter.line_segment(
        [
            center + direction * radius,
            center + direction * rect.width() * 0.48,
        ],
        stroke,
    );
}

/// Кнопка очистки поля поиска: крестик в круге, круг светлеет при наведении
pub fn paint_clear(ui: &Ui, rect: Rect, hover: f32) {
    let painter = ui.painter();
    painter.circle_filled(
        rect.center(),
        rect.width() / 2.0,
        ICON_DISABLED.lerp_to_gamma(ICON_OFF, hover),
    );
    let arm = rect.width() * 0.18;
    let stroke = Stroke::new(1.4, Color32::from_rgb(0x1C, 0x1C, 0x1E));
    for (from, to) in [
        (vec2(-arm, -arm), vec2(arm, arm)),
        (vec2(-arm, arm), vec2(arm, -arm)),
    ] {
        painter.line_segment([rect.center() + from, rect.center() + to], stroke);
    }
}

/// Кнопка громкости. `volume` — слышимая громкость 0.0..=1.0: в нуле (или с выключенным
/// звуком) — крестик, тише `VOLUME_LOUD` — одна волна, громче — две.
/// `hover`: 0.0..=1.0, плавный переход наведения
pub fn paint_volume(ui: &Ui, rect: Rect, volume: f32, hover: f32) {
    paint_hover_circle(ui, rect, hover);
    for shape in SPEAKER_SHAPES {
        let points: Vec<Pos2> = shape.iter().map(|&(x, y)| pos2(x, y)).collect();
        fill_icon_shape(ui, rect, &points, SPEAKER_CORNER_RADIUS, ICON);
    }

    if volume == 0.0 {
        for line in MUTE_CROSS {
            stroke_icon_path(ui, rect, &line, VOLUME_WAVE_STROKE, ICON);
        }
        return;
    }
    let waves = if volume < VOLUME_LOUD { 1 } else { 2 };
    for radius in &VOLUME_WAVE_RADII[..waves] {
        const SEGMENTS: usize = 12;
        let (cx, cy) = VOLUME_WAVE_CENTER;
        let arc: Vec<(f32, f32)> = (0..=SEGMENTS)
            .map(|i| {
                let t = i as f32 / SEGMENTS as f32;
                let angle = -VOLUME_WAVE_HALF_ANGLE + 2.0 * VOLUME_WAVE_HALF_ANGLE * t;
                (cx + radius * angle.cos(), cy + radius * angle.sin())
            })
            .collect();
        stroke_icon_path(ui, rect, &arc, VOLUME_WAVE_STROKE, ICON);
    }
}
