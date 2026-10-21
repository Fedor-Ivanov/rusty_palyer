//! Сетка плиток, как CSS `grid-template-columns: repeat(auto-fill, minmax(…, 1fr))`:
//! число колонок подстраивается под ширину, плитки растягиваются на всю строку.
//! Плитка — квадратная картинка и несколько строк текста под ней. Виртуализирована,
//! как и списки: рисуются только видимые строки сетки. Выделение — как в списке треков:
//! ⌘/Shift+клик, рамкой мыши, клавиши ⌘A, Esc, Delete.

use eframe::egui::{
    self, Align, Color32, CursorIcon, FontId, Key, Rect, Response, RichText, Sense, Stroke,
    StrokeKind, Ui, pos2, vec2,
};
use std::collections::BTreeSet;

use super::selection::{Marquee, Selection};
use crate::ui::style::theme::{
    HIGHLIGHT_STRONG, HOVER_FADE_SECS, MARQUEE_FILL, MARQUEE_STROKE, SELECTION_STROKE_WIDTH, TEXT,
    TEXT_MUTED,
};

/// Плитка не уже этого: колонок столько, сколько таких плиток помещается
const TILE_MIN_WIDTH: f32 = 140.0;
const TILE_GAP: f32 = 18.0;
/// Отступ сверху: рамка выделения выходит за плитку и у первой строки иначе обрезалась бы
/// краем прокрутки
const TOP_MARGIN: f32 = 8.0;
const SIDE_MARGIN: f32 = 16.0;
/// Зазор между картинкой и текстом и между строками текста
const TEXT_TOP_GAP: f32 = 8.0;
const LINE_GAP: f32 = 2.0;
const PRIMARY_SIZE: f32 = 14.0;
const SECONDARY_SIZE: f32 = 12.0;
/// Рамка выделенной плитки: отступ от плитки (чтобы не ложилась на край обложки)
/// и скругление
const HIGHLIGHT_PADDING: f32 = 6.0;
const HIGHLIGHT_RADIUS: u8 = 14;
/// Скорость автопрокрутки при выделении областью — как у списка треков
const MARQUEE_AUTOSCROLL_SPEED: f32 = 8.0;

/// Плитка, которую сейчас рисуют
pub struct Tile {
    /// Плавное наведение 0.0..=1.0 — по положению курсора над плиткой: остаётся и тогда,
    /// когда курсор над кнопкой поверх плитки
    pub hover: f32,
    /// Отклик всей плитки (картинка с подписью) — для контекстного меню
    pub response: Response,
    /// Сколько плиток выделено — меню по правому клику действует на них все
    pub selected: usize,
}

/// Строка текста под картинкой плитки
pub struct TileLine {
    pub text: String,
    /// Первая строка (название) — крупнее и ярче остальных
    pub primary: bool,
}

/// Что произошло с сеткой за кадр — решает вызывающий
pub struct TileGridResponse {
    /// Обычный клик по плитке (без ⌘/Shift) — открыть её
    pub opened: Option<usize>,
    /// Нажали Delete или Backspace, пока сетка активна и что-то выделено
    pub delete: bool,
    /// С сеткой работали мышью — она становится активной для клавиатуры
    pub interacted: bool,
}

/// Состояние одной сетки между кадрами: выделение и выделение областью
pub struct TileGridView {
    /// Различает сетки: у каждой свои id прокрутки и фона
    id: &'static str,
    selection: Selection,
    marquee: Option<Marquee>,
    /// К какому содержимому относится выделение (см. `set_scope`)
    scope: String,
}

impl TileGridView {
    pub fn new(id: &'static str) -> Self {
        Self {
            id,
            selection: Selection::default(),
            marquee: None,
            scope: String::new(),
        }
    }

    /// Выделение — номера плиток. Если содержимое сетки сменилось (другой исполнитель,
    /// библиотека изменилась), те же номера значат другие плитки — выделение сбрасывается.
    /// `scope` описывает содержимое; вызывать каждый кадр до `show`
    pub fn set_scope(&mut self, scope: &str) {
        if self.scope != scope {
            self.selection.clear();
            self.marquee = None;
            self.scope = scope.to_owned();
        }
    }

    pub fn selection(&self) -> &BTreeSet<usize> {
        self.selection.indices()
    }

    /// Рисует сетку из `count` плиток с `line_count` строками текста в каждой. `text_align` —
    /// выравнивание текста под картинкой: `Align::Min` — по левому краю, `Align::Center` —
    /// по центру. `keyboard` — активна ли сетка для клавиш (⌘A, Esc, Delete).
    /// `tile` рисует картинку плитки `index` в квадрате `image_rect` (может добавить поверх
    /// неё свои кнопки и меню через [`Tile`]) и возвращает строки текста под ней.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        count: usize,
        line_count: usize,
        text_align: Align,
        keyboard: bool,
        mut tile: impl FnMut(&mut Ui, usize, Rect, &Tile) -> Vec<TileLine>,
    ) -> TileGridResponse {
        let mut response = TileGridResponse {
            opened: None,
            delete: false,
            interacted: false,
        };
        if count == 0 {
            return response;
        }

        // Пока набирают текст (например, в поиске), Backspace и ⌘A — для текста, а не для списка
        if keyboard && !ui.ctx().text_edit_focused() {
            let (delete, select_all, deselect) = ui.input(|i| {
                (
                    i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace),
                    i.modifiers.command && i.key_pressed(Key::A),
                    i.key_pressed(Key::Escape),
                )
            });
            response.delete = delete && !self.selection.is_empty();
            if select_all {
                self.selection.select_all(count);
            }
            if deselect {
                self.selection.clear();
            }
        }

        let width = (ui.available_width() - 2.0 * SIDE_MARGIN).max(TILE_MIN_WIDTH);
        let columns = (((width + TILE_GAP) / (TILE_MIN_WIDTH + TILE_GAP)).floor() as usize).max(1);
        let tile_width = (width - TILE_GAP * (columns - 1) as f32) / columns as f32;
        // Высота строки сетки фиксированная (картинка + все строки текста) — без этого сетку
        // нельзя виртуализировать, а номер плитки под курсором — вычислить
        let row_height = tile_width + TEXT_TOP_GAP + text_height(ui, line_count);
        let rows = count.div_ceil(columns);
        let modifiers = ui.input(|i| i.modifiers);

        ui.spacing_mut().item_spacing.y = TILE_GAP;
        egui::ScrollArea::vertical()
            .id_salt(self.id)
            .content_margin(egui::Margin {
                left: SIDE_MARGIN as i8,
                right: SIDE_MARGIN as i8,
                top: TOP_MARGIN as i8,
                bottom: SIDE_MARGIN as i8,
            })
            .show_rows(ui, row_height, rows, |ui, visible_rows| {
                // Верх и левый край всего содержимого сетки на экране: show_rows отдаёт
                // только видимый кусок, начиная со строки visible_rows.start. Плитки под рамкой
                // считаются по размерам сетки, а не по нарисованным виджетам — плиток
                // за пределами экрана нет, а выделять их рамкой нужно
                let row_stride = row_height + TILE_GAP;
                let content_top = ui.max_rect().top() - visible_rows.start as f32 * row_stride;
                let content_left = ui.max_rect().left();
                let tile_rect = |row: usize, column: usize| {
                    Rect::from_min_size(
                        pos2(
                            content_left + column as f32 * (tile_width + TILE_GAP),
                            content_top + row as f32 * row_stride,
                        ),
                        vec2(tile_width, row_height),
                    )
                };

                // Фон под плитками — для выделения областью и клика по пустому месту.
                // Добавляется до плиток: плитки оказываются сверху и получают клики,
                // а перетаскивание достаётся фону (плитки его не ловят)
                let visible = ui.clip_rect();
                let background = ui.interact(
                    visible,
                    egui::Id::unique(self.id).with("marquee"),
                    Sense::click_and_drag(),
                );
                if background.clicked() {
                    self.selection.clear();
                    response.interacted = true;
                }
                if background.drag_started()
                    && let Some(start) = background.interact_pointer_pos()
                {
                    self.marquee = Some(Marquee {
                        start: pos2(start.x, start.y - content_top),
                        // С ⌘/Ctrl область добавляется к уже выделенному
                        base: if modifiers.command {
                            self.selection.indices().clone()
                        } else {
                            BTreeSet::new()
                        },
                    });
                    response.interacted = true;
                }

                for row in visible_rows {
                    ui.allocate_exact_size(vec2(ui.available_width(), row_height), Sense::hover());
                    for column in 0..columns {
                        let index = row * columns + column;
                        if index >= count {
                            break;
                        }
                        let rect = tile_rect(row, column);
                        let image_rect =
                            Rect::from_min_size(rect.min, vec2(tile_width, tile_width));

                        // Плитка целиком — картинка вместе с подписью. Добавляется до
                        // содержимого: кнопки, которые плитка рисует поверх картинки,
                        // получают клики первыми
                        let tile_id = ui.scope_id().with((self.id, index));
                        let tile_response = ui
                            .interact(rect, tile_id, Sense::click())
                            .on_hover_cursor(CursorIcon::PointingHand);

                        // Клик без модификаторов открывает плитку; ⌘/Shift+клик выделяет,
                        // как в списке. Правый клик по невыделенной плитке выделяет её —
                        // меню действует на выделение, как в Finder
                        if tile_response.clicked() {
                            if modifiers.command || modifiers.shift {
                                self.selection.click(index, modifiers);
                            } else {
                                self.selection.clear();
                                response.opened = Some(index);
                            }
                            response.interacted = true;
                        }
                        if tile_response.secondary_clicked() {
                            if !self.selection.contains(index) {
                                self.selection.click(index, egui::Modifiers::NONE);
                            }
                            response.interacted = true;
                        }

                        let hover = ui.ctx().animate_bool_with_time_and_easing(
                            tile_id.with("hover"),
                            tile_response.contains_pointer(),
                            HOVER_FADE_SECS,
                            egui::emath::easing::cubic_out,
                        );
                        // Выделенная плитка и плитка с открытым меню обведены рамкой,
                        // как выделенный трек в списке
                        let highlighted = ui.ctx().animate_bool_with_time_and_easing(
                            tile_id.with("highlighted"),
                            self.selection.contains(index) || tile_response.context_menu_opened(),
                            HOVER_FADE_SECS,
                            egui::emath::easing::cubic_out,
                        );

                        let lines = tile(
                            ui,
                            index,
                            image_rect,
                            &Tile {
                                hover,
                                response: tile_response,
                                selected: self.selection.len().max(1),
                            },
                        );
                        paint_lines(
                            ui,
                            &lines,
                            pos2(rect.left(), image_rect.bottom() + TEXT_TOP_GAP),
                            tile_width,
                            text_align,
                        );
                        if highlighted > 0.0 {
                            ui.painter().rect_stroke(
                                rect.expand(HIGHLIGHT_PADDING),
                                HIGHLIGHT_RADIUS,
                                Stroke::new(
                                    SELECTION_STROKE_WIDTH,
                                    HIGHLIGHT_STRONG.gamma_multiply(highlighted),
                                ),
                                StrokeKind::Inside,
                            );
                        }
                    }
                }

                // Рамка — после плиток, чтобы лежала поверх них
                if let Some(marquee) = &self.marquee
                    && background.dragged()
                    && let Some(pointer) = background.interact_pointer_pos()
                {
                    let start = pos2(marquee.start.x, content_top + marquee.start.y);
                    let area = Rect::from_two_pos(start, pointer);

                    // Плитки, которых касается рамка: строки — по высоте сетки, в каждой —
                    // колонки, чей прямоугольник пересекается с рамкой
                    let row_at = |y: f32| {
                        ((((y - content_top) / row_stride).floor()).max(0.0) as usize).min(rows - 1)
                    };
                    let touched: Vec<usize> = (row_at(area.top())..=row_at(area.bottom()))
                        .flat_map(|row| (0..columns).map(move |column| (row, column)))
                        .filter(|&(row, column)| row * columns + column < count)
                        .filter(|&(row, column)| tile_rect(row, column).intersects(area))
                        .map(|(row, column)| row * columns + column)
                        .collect();
                    let anchor = touched.first().copied().unwrap_or(0);
                    self.selection.select_many(&marquee.base, touched, anchor);

                    ui.painter().rect(
                        area,
                        4,
                        MARQUEE_FILL,
                        Stroke::new(SELECTION_STROKE_WIDTH, MARQUEE_STROKE),
                        StrokeKind::Inside,
                    );

                    // Курсор ушёл за верх или низ сетки — прокручиваем туда, тем быстрее,
                    // чем дальше курсор. Положительная дельта сдвигает содержимое вниз
                    let overshoot = if pointer.y < visible.top() {
                        visible.top() - pointer.y
                    } else if pointer.y > visible.bottom() {
                        visible.bottom() - pointer.y
                    } else {
                        0.0
                    };
                    if overshoot != 0.0 {
                        let dt = ui.input(|i| i.stable_dt);
                        ui.scroll_with_delta(vec2(0.0, overshoot * MARQUEE_AUTOSCROLL_SPEED * dt));
                        ui.ctx().request_repaint();
                    }
                }
                if background.drag_stopped() {
                    self.marquee = None;
                }
            });

        response
    }
}

/// Высота текста под картинкой: первая строка крупнее, остальные мельче
fn text_height(ui: &Ui, line_count: usize) -> f32 {
    let line_height = |size: f32| {
        ui.painter()
            .layout_no_wrap("Ag".into(), FontId::proportional(size), TEXT)
            .size()
            .y
    };
    (0..line_count)
        .map(|line| {
            line_height(if line == 0 {
                PRIMARY_SIZE
            } else {
                SECONDARY_SIZE
            })
        })
        .sum::<f32>()
        + LINE_GAP * line_count.saturating_sub(1) as f32
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
