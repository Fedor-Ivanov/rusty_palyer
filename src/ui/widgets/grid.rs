//! Сетка плиток, как CSS `grid-template-columns: repeat(auto-fill, minmax(…, 1fr))`:
//! число колонок подстраивается под ширину, плитки растягиваются на всю строку.
//! Плитка — квадратная картинка и несколько строк текста под ней. Виртуализирована,
//! как и списки: рисуются только видимые строки сетки.

use eframe::egui::{
    self, Align, Color32, CursorIcon, FontId, Rect, Response, RichText, Sense, Ui, pos2, vec2,
};

use crate::ui::theme::{HOVER_FADE_SECS, TEXT, TEXT_MUTED};

/// Плитка не уже этого: колонок столько, сколько таких плиток помещается
const TILE_MIN_WIDTH: f32 = 140.0;
const TILE_GAP: f32 = 18.0;
const TOP_MARGIN: f32 = 8.0;
/// Зазор между картинкой и текстом и между строками текста
const TEXT_TOP_GAP: f32 = 8.0;
const LINE_GAP: f32 = 2.0;
const PRIMARY_SIZE: f32 = 14.0;
const SECONDARY_SIZE: f32 = 12.0;

/// Плитка, которую сейчас рисуют
pub struct Tile {
    /// Плавное наведение 0.0..=1.0 — по положению курсора над плиткой: остаётся и тогда,
    /// когда курсор над кнопкой поверх плитки
    pub hover: f32,
    /// Отклик всей плитки (картинка с подписью) — для контекстного меню
    pub response: Response,
}

/// Строка текста под картинкой плитки
pub struct TileLine {
    pub text: String,
    /// Первая строка (название) — крупнее и ярче остальных
    pub primary: bool,
}

/// Рисует сетку из `count` плиток с `line_count` строками текста в каждой. `text_align` —
/// выравнивание текста под картинкой: `Align::Min` — по левому краю, `Align::Center` — по
/// центру. `clickable` — клик по плитке что-то делает (курсор-рука над ней).
/// `tile` рисует картинку плитки `index` в квадрате `image_rect` (может добавить поверх неё
/// свои кнопки и меню через [`Tile`]) и возвращает строки текста под ней.
/// Возвращает номер плитки, по которой кликнули.
pub fn tile_grid(
    ui: &mut Ui,
    id_salt: &str,
    count: usize,
    line_count: usize,
    text_align: Align,
    clickable: bool,
    mut tile: impl FnMut(&mut Ui, usize, Rect, &Tile) -> Vec<TileLine>,
) -> Option<usize> {
    let mut clicked = None;
    let margin = 16.0;
    let width = (ui.available_width() - 2.0 * margin).max(TILE_MIN_WIDTH);
    let columns = (((width + TILE_GAP) / (TILE_MIN_WIDTH + TILE_GAP)).floor() as usize).max(1);
    let tile_width = (width - TILE_GAP * (columns - 1) as f32) / columns as f32;

    // Высота строки сетки фиксированная (картинка + все строки текста) — без этого сетку
    // нельзя виртуализировать
    let line_height = |size: f32| {
        ui.painter()
            .layout_no_wrap("Ag".into(), FontId::proportional(size), TEXT)
            .size()
            .y
    };
    let text_height = (0..line_count)
        .map(|line| {
            let size = if line == 0 {
                PRIMARY_SIZE
            } else {
                SECONDARY_SIZE
            };
            line_height(size)
        })
        .sum::<f32>()
        + LINE_GAP * line_count.saturating_sub(1) as f32;
    let row_height = tile_width + TEXT_TOP_GAP + text_height;
    let rows = count.div_ceil(columns);

    ui.spacing_mut().item_spacing.y = TILE_GAP;
    egui::ScrollArea::vertical()
        .id_salt(id_salt)
        // Сверху небольшой отступ: рамка вокруг плитки (например, с открытым меню) выходит
        // за плитку и у первой строки иначе обрезалась бы краем прокрутки
        .content_margin(egui::Margin {
            left: margin as i8,
            right: margin as i8,
            top: TOP_MARGIN as i8,
            bottom: margin as i8,
        })
        .show_rows(ui, row_height, rows, |ui, visible_rows| {
            for row in visible_rows {
                let (row_rect, _) =
                    ui.allocate_exact_size(vec2(ui.available_width(), row_height), Sense::hover());
                for column in 0..columns {
                    let index = row * columns + column;
                    if index >= count {
                        break;
                    }
                    let left = row_rect.left() + column as f32 * (tile_width + TILE_GAP);
                    let image_rect = Rect::from_min_size(
                        pos2(left, row_rect.top()),
                        vec2(tile_width, tile_width),
                    );
                    // Плитка целиком — картинка вместе с подписью. Добавляется до содержимого:
                    // кнопки, которые плитка рисует поверх картинки, получают клики первыми
                    let tile_rect =
                        Rect::from_min_size(image_rect.min, vec2(tile_width, row_height));
                    let tile_id = ui.scope_id().with((id_salt, index));
                    let mut response = ui.interact(tile_rect, tile_id, Sense::click());
                    if clickable {
                        response = response.on_hover_cursor(CursorIcon::PointingHand);
                        if response.clicked() {
                            clicked = Some(index);
                        }
                    }
                    let hover = ui.ctx().animate_bool_with_time_and_easing(
                        tile_id.with("hover"),
                        response.contains_pointer(),
                        HOVER_FADE_SECS,
                        egui::emath::easing::cubic_out,
                    );
                    let lines = tile(ui, index, image_rect, &Tile { hover, response });
                    paint_lines(
                        ui,
                        &lines,
                        pos2(left, image_rect.bottom() + TEXT_TOP_GAP),
                        tile_width,
                        text_align,
                    );
                }
            }
        });
    clicked
}

/// Строки под картинкой, обрезанные с «…» по ширине плитки
fn paint_lines(ui: &Ui, lines: &[TileLine], mut pos: egui::Pos2, width: f32, align: Align) {
    for line in lines {
        let (size, color): (f32, Color32) = if line.primary {
            (PRIMARY_SIZE, TEXT)
        } else {
            (SECONDARY_SIZE, TEXT_MUTED)
        };
        let galley = egui::WidgetText::from(RichText::new(&line.text).size(size).color(color))
            .into_galley(
                ui,
                Some(egui::TextWrapMode::Truncate),
                width,
                FontId::proportional(size),
            );
        let size = galley.size();
        // Свободное место по ширине — слева, поровну с обеих сторон или справа
        let x = pos.x + (width - size.x).max(0.0) * align.to_factor();
        ui.painter().galley(pos2(x, pos.y), galley, color);
        pos.y += size.y + LINE_GAP;
    }
}
