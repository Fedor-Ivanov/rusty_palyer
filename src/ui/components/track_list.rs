//! Список треков — общий для библиотеки и плейлиста: виртуализация (рисуются только видимые
//! строки), выделение кликами и областью мыши, клавиши, контекстное меню.

use eframe::egui::{self, Align, Key, Label, Layout, Rect, RichText, Sense, Ui, pos2, vec2};
use std::collections::BTreeSet;

use super::selection::{Marquee, Selection};
use crate::{
    library::Track,
    ui::{
        format::format_secs,
        i18n::t,
        theme::{
            HIGHLIGHT, HIGHLIGHT_STRONG, HOVER_FADE_SECS, MARQUEE_FILL, MARQUEE_STROKE,
            SELECTION_STROKE_WIDTH, TEXT, TEXT_FAINT, TEXT_MUTED, TRACK_ACTIVE_FADE_SECS,
        },
    },
};

// Строки: высота фиксированная — без неё список нельзя виртуализировать,
// так как позицию строки не вычислить без измерения
const TRACK_ROW_HEIGHT: f32 = 30.0;
const TRACK_ROW_PADDING: f32 = 8.0;
const TRACK_ROW_GAP: f32 = 6.0;
// Скорость автопрокрутки при выделении областью: точек в секунду на каждую точку,
// на которую курсор ушёл за край списка
const MARQUEE_AUTOSCROLL_SPEED: f32 = 8.0;

/// Состояние одного списка между кадрами: выделение и выделение областью
pub struct TrackListView {
    /// Различает списки: у каждого свои id прокрутки и фона
    id: &'static str,
    selection: Selection,
    marquee: Option<Marquee>,
}

/// Что произошло со списком за кадр — решает вызывающий
pub struct TrackListResponse {
    /// Двойной клик по треку
    pub activated: Option<usize>,
    /// Нажали Delete или Backspace, пока список активен
    pub delete: bool,
    /// Со списком работали мышью — он становится активным для клавиатуры
    pub interacted: bool,
}

/// Что показать в строке
pub struct RowState {
    /// Играющий трек: подсвечен
    pub current: bool,
    /// Файла нет на диске: текст приглушён, при наведении — подсказка
    pub missing: bool,
}

impl TrackListView {
    pub fn new(id: &'static str) -> Self {
        Self {
            id,
            selection: Selection::default(),
            marquee: None,
        }
    }

    pub fn selection(&self) -> &BTreeSet<usize> {
        self.selection.indices()
    }

    /// Номера треков после удаления сдвигаются — старое выделение ни на что не указывает
    pub fn clear_selection(&mut self) {
        self.selection.clear();
    }

    /// Рисует список. `row_state` — что показать в строке по её номеру; `keyboard` — активен
    /// ли список для клавиш (⌘A, Esc, Delete); `context_menu` рисует пункты меню по правому
    /// клику и получает число выделенных треков.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        tracks: &[Track],
        row_state: impl Fn(usize) -> RowState,
        keyboard: bool,
        mut context_menu: impl FnMut(&mut Ui, usize),
    ) -> TrackListResponse {
        let track_count = tracks.len();
        let mut response = TrackListResponse {
            activated: None,
            delete: false,
            interacted: false,
        };
        if track_count == 0 {
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
                self.selection.select_all(track_count);
            }
            if deselect {
                self.selection.clear();
            }
        }

        let modifiers = ui.input(|i| i.modifiers);
        // show_rows берёт зазор между строками из Ui, в котором вызван
        ui.spacing_mut().item_spacing.y = TRACK_ROW_GAP;
        egui::ScrollArea::vertical()
            .id_salt(self.id)
            // Отступ задаётся самой ScrollArea: полоса прокрутки остаётся у края панели,
            // а отступ прокручивается вместе со списком. Frame внутри show_rows не годится —
            // его отступ сбил бы расчёт позиций строк
            .content_margin(egui::Margin {
                left: 16,
                right: 16,
                top: 0,
                bottom: 16,
            })
            .show_rows(ui, TRACK_ROW_HEIGHT, track_count, |ui, rows| {
                // Верх всего содержимого списка (строки 0) в координатах экрана: show_rows
                // отдаёт только видимый кусок, начиная со строки rows.start. Номер строки
                // под курсором считается по высоте строк, а не по нарисованным виджетам —
                // строк за пределами экрана нет, а выделять их областью нужно
                let row_stride = TRACK_ROW_HEIGHT + TRACK_ROW_GAP;
                let content_top = ui.max_rect().top() - rows.start as f32 * row_stride;
                let row_at = |y: f32| {
                    (((y - content_top) / row_stride).floor().max(0.0) as usize)
                        .min(track_count - 1)
                };

                // Фон под строками — для выделения областью и клика по пустому месту.
                // Добавляется до строк: строки оказываются сверху и получают клики,
                // а перетаскивание достаётся фону (строки его не ловят)
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

                for index in rows {
                    let row = track_row(
                        ui,
                        index,
                        &tracks[index],
                        &row_state(index),
                        self.selection.contains(index),
                    );

                    if row.clicked() {
                        self.selection.click(index, modifiers);
                        response.interacted = true;
                    }
                    if row.double_clicked() {
                        response.activated = Some(index);
                    }
                    // Правый клик по невыделенному треку выделяет его — меню действует
                    // на выделение, как в Finder
                    if row.secondary_clicked() {
                        if !self.selection.contains(index) {
                            self.selection.click(index, egui::Modifiers::NONE);
                        }
                        response.interacted = true;
                    }
                    let selected = self.selection.len();
                    row.context_menu(|ui| {
                        super::menu_style(ui);
                        context_menu(ui, selected);
                    });
                }

                // Рамка — после строк, чтобы лежала поверх них
                if let Some(marquee) = &self.marquee
                    && background.dragged()
                    && let Some(pointer) = background.interact_pointer_pos()
                {
                    let start = pos2(marquee.start.x, content_top + marquee.start.y);
                    let (from, to) = (row_at(start.y), row_at(pointer.y));
                    self.selection
                        .select_range(&marquee.base, from.min(to)..=from.max(to), from);

                    ui.painter().rect(
                        Rect::from_two_pos(start, pointer),
                        4,
                        MARQUEE_FILL,
                        egui::Stroke::new(SELECTION_STROKE_WIDTH, MARQUEE_STROKE),
                        egui::StrokeKind::Inside,
                    );

                    // Курсор ушёл за верх или низ списка — прокручиваем туда, тем быстрее,
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

/// Строка списка фиксированной высоты: название слева (обрезается с «…»), длительность справа
fn track_row(
    ui: &mut Ui,
    index: usize,
    track: &Track,
    state: &RowState,
    is_selected: bool,
) -> egui::Response {
    let (row_rect, _) =
        ui.allocate_exact_size(vec2(ui.available_width(), TRACK_ROW_HEIGHT), Sense::hover());
    // Стабильный id строки: по нему egui хранит состояние анимаций. Id от позиции
    // не годится — при прокрутке строки меняют место
    let row_id = ui.scope_id().with(("track_row", index));
    // Обычный курсор, а не «рука»: клик выделяет, а не запускает трек — запуск двойным кликом
    let mut row = ui.interact(row_rect, row_id, Sense::click());
    if state.missing {
        row = row.on_hover_text(t().file_missing);
    }

    // Активность, выделение и наведение меняются плавно: фон и цвет текста перетекают
    let active = ui.ctx().animate_bool_with_time_and_easing(
        row_id.with("active"),
        state.current,
        TRACK_ACTIVE_FADE_SECS,
        egui::emath::easing::cubic_in_out,
    );
    let selected = ui.ctx().animate_bool_with_time_and_easing(
        row_id.with("selected"),
        is_selected,
        HOVER_FADE_SECS,
        egui::emath::easing::cubic_out,
    );
    let hover = ui.ctx().animate_bool_with_time_and_easing(
        row_id.with("hover"),
        row.hovered() && !state.current,
        HOVER_FADE_SECS,
        egui::emath::easing::cubic_out,
    );
    // Фон рисуется до текста, поэтому оказывается под ним
    let fill = HIGHLIGHT
        .gamma_multiply(hover)
        .lerp_to_gamma(HIGHLIGHT_STRONG, active);
    ui.painter().rect_filled(row_rect, 8, fill);
    // Выделение — только рамка того же цвета, что подсветка играющего трека: фон остаётся
    // свободным, и выделенный играющий трек по-прежнему видно как играющий
    if selected > 0.0 {
        ui.painter().rect_stroke(
            row_rect,
            8,
            egui::Stroke::new(
                SELECTION_STROKE_WIDTH,
                HIGHLIGHT_STRONG.gamma_multiply(selected),
            ),
            egui::StrokeKind::Inside,
        );
    }

    let text_color = if state.missing {
        TEXT_FAINT
    } else {
        TEXT_MUTED.lerp_to_gamma(TEXT, active)
    };
    let content = row_rect.shrink2(vec2(TRACK_ROW_PADDING, 0.0));
    // Аналог justify-content: space-between: время прижато вправо,
    // название занимает оставшееся место слева
    let mut content_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(content)
            .layout(Layout::right_to_left(Align::Center)),
    );
    let duration = RichText::new(format_secs(track.duration_secs as f32))
        .size(14.0)
        .color(text_color);
    content_ui.add(Label::new(duration).selectable(false));
    content_ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
        let title = if track.artist.is_empty() {
            track.title.clone()
        } else {
            format!("{} — {}", track.title, track.artist)
        };
        let title = RichText::new(title).size(14.0).color(text_color);
        ui.add(Label::new(title).selectable(false).truncate());
    });

    row
}
