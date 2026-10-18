//! Заголовки блоков и линии на границах панелей.

use eframe::egui::{
    self, Align, Color32, CursorIcon, FontId, Galley, Rangef, Rect, Response, RichText, Sense,
    Stroke, Ui, pos2, vec2,
};
use std::sync::Arc;

use crate::ui::theme::{HOVER_FADE_SECS, RESIZE_LINE, TEXT, TEXT_FAINT, TEXT_MUTED};

const BLOCK_TITLE_SIZE: f32 = 16.0;
/// Пояснение рядом с заголовком
const NOTE_SIZE: f32 = 13.0;
const RESIZE_LINE_WIDTH: f32 = 1.0;

/// Заголовок блока (панели) с выравниванием `align`: `Align::Min` — слева,
/// `Align::Center` — по центру, `Align::Max` — справа.
/// Возвращает прямоугольник строки во всю ширину — по нему можно поставить кнопки
/// рядом с заголовком.
pub fn block_title(ui: &mut Ui, text: &str, align: Align, color: Color32) -> Rect {
    let row = title_row(ui);
    let galley = title_galley(ui, text, color, row.width());
    let x = row.left() + (row.width() - galley.size().x).max(0.0) * align.to_factor();
    paint_in_row(ui, row, x, galley, color);
    row
}

/// Заголовок блока слева и рядом с ним `note` — мелким приглушённым текстом на той же линии
/// (например, сколько треков в списке). Строка та же, что у `block_title`
pub fn block_title_with_note(ui: &mut Ui, text: &str, note: &str) -> Rect {
    // Примерно ширина пробела: «Playlist · 12 tracks» читается одной фразой
    const GAP: f32 = 5.0;
    let row = title_row(ui);
    let title = title_galley(ui, text, TEXT, row.width());
    let title_width = title.size().x;
    paint_in_row(ui, row, row.left(), title, TEXT);

    let note = egui::WidgetText::from(RichText::new(note).size(NOTE_SIZE).color(TEXT_FAINT))
        .into_galley(
            ui,
            Some(egui::TextWrapMode::Truncate),
            (row.width() - title_width - GAP).max(0.0),
            FontId::proportional(NOTE_SIZE),
        );
    paint_in_row(ui, row, row.left() + title_width + GAP, note, TEXT_FAINT);
    row
}

/// Линия изменения ширины боковой панели — на всю высоту границы панелей, видна всегда
pub fn paint_resize_line(ui: &Ui, edge_x: f32, y_range: Rangef) {
    ui.painter()
        .vline(edge_x, y_range, Stroke::new(RESIZE_LINE_WIDTH, RESIZE_LINE));
}

/// Заголовок-«хлебные крошки»: `parents › … › current`. Каждый из `parents` — ссылка
/// на свой уровень, при наведении плавно светлеет. Возвращает номер ссылки, по которой
/// кликнули. Строка той же высоты и текст на той же линии, что у `block_title`:
/// при переходе между обычным заголовком и крошками ничего не сдвигается
pub fn breadcrumb_title(ui: &mut Ui, parents: &[&str], current: &str) -> Option<usize> {
    const GAP: f32 = 6.0;
    let row = title_row(ui);
    let mut clicked = None;
    let mut x = row.left();

    for (index, parent) in parents.iter().enumerate() {
        // Ссылка: место под неё выделяется до отрисовки — цвет зависит от наведения
        let width = title_galley(ui, parent, TEXT, f32::INFINITY).size().x;
        let rect = Rect::from_min_size(pos2(x, row.top()), vec2(width, row.height()));
        let response = ui
            .interact(
                rect,
                ui.scope_id().with(("breadcrumb", index)),
                Sense::click(),
            )
            .on_hover_cursor(CursorIcon::PointingHand);
        if response.clicked() {
            clicked = Some(index);
        }
        let hover = ui.ctx().animate_bool_with_time_and_easing(
            response.id.with("hover"),
            response.hovered(),
            HOVER_FADE_SECS,
            egui::emath::easing::cubic_out,
        );
        let color = TEXT_MUTED.lerp_to_gamma(TEXT, hover);
        paint_in_row(
            ui,
            row,
            x,
            title_galley(ui, parent, color, f32::INFINITY),
            color,
        );
        x += width + GAP;

        let separator = title_galley(ui, "›", TEXT_FAINT, f32::INFINITY);
        let separator_width = separator.size().x;
        paint_in_row(ui, row, x, separator, TEXT_FAINT);
        x += separator_width + GAP;
    }

    // Текущий уровень — обрезается с «…», если не помещается
    let current = title_galley(ui, current, TEXT, (row.right() - x).max(0.0));
    paint_in_row(ui, row, x, current, TEXT);

    clicked
}

/// Строка заголовка во всю ширину. Высота — по высоте строки шрифта заголовка, одна для
/// всех видов заголовков
fn title_row(ui: &mut Ui) -> Rect {
    let height = title_galley(ui, "Ag", TEXT, f32::INFINITY).size().y;
    ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover())
        .0
}

/// Текст заголовка, обрезанный с «…» по ширине `max_width`
fn title_galley(ui: &Ui, text: &str, color: Color32, max_width: f32) -> Arc<Galley> {
    egui::WidgetText::from(RichText::new(text).size(BLOCK_TITLE_SIZE).color(color)).into_galley(
        ui,
        Some(egui::TextWrapMode::Truncate),
        max_width,
        FontId::proportional(BLOCK_TITLE_SIZE),
    )
}

/// Текст в строке заголовка с левого края `x`, по центру строки по высоте
fn paint_in_row(ui: &Ui, row: Rect, x: f32, galley: Arc<Galley>, color: Color32) {
    let y = row.center().y - galley.size().y / 2.0;
    ui.painter().galley(pos2(x, y), galley, color);
}

/// Текст-ссылка: в покое цвета `color`, при наведении плавно светлеет, курсор — рука.
/// Не шире доступного места: длинный текст обрезается с «…»
pub fn text_link(ui: &mut Ui, text: &str, size: f32, color: Color32) -> Response {
    let galley = |ui: &Ui, color: Color32, max_width: f32| {
        egui::WidgetText::from(RichText::new(text).size(size).color(color)).into_galley(
            ui,
            Some(egui::TextWrapMode::Truncate),
            max_width,
            FontId::proportional(size),
        )
    };
    // Место выделяется до отрисовки — цвет зависит от наведения
    let link_size = galley(ui, color, ui.available_width()).size();
    let (rect, response) = ui.allocate_exact_size(link_size, Sense::click());
    let response = response.on_hover_cursor(CursorIcon::PointingHand);
    let hover = ui.ctx().animate_bool_with_time_and_easing(
        response.id.with("hover"),
        response.hovered(),
        HOVER_FADE_SECS,
        egui::emath::easing::cubic_out,
    );
    let color = color.lerp_to_gamma(TEXT, hover);
    ui.painter()
        .galley(rect.min, galley(ui, color, rect.width()), color);
    response
}
