//! Правая панель: виртуализированный список треков, выделение, контекстное меню,
//! нижний блок с shuffle и повтором.

use eframe::egui::{
    self, Align, Align2, FontId, Key, Label, Layout, Rect, RichText, Sense, Ui, pos2, vec2,
};
use std::collections::BTreeSet;

use super::{ADD_BUTTON_SIZE, Marquee, PlayerApp};
use crate::{
    library::Track,
    playback::RepeatMode,
    ui::{
        format::format_secs,
        icons::{paint_repeat, paint_shuffle},
        theme::{
            HIGHLIGHT, HIGHLIGHT_STRONG, HOVER_FADE_SECS, MARQUEE_FILL, MARQUEE_STROKE, TEXT,
            TEXT_FAINT, TEXT_MUTED, TRACK_ACTIVE_FADE_SECS,
        },
        widgets,
    },
};

// Зазор между кнопками повтора в нижнем блоке списка
const ICON_BUTTON_GAP: f32 = 4.0;
// Строки списка треков: высота фиксированная — без неё список нельзя виртуализировать
// (рисовать только видимые строки), так как позицию строки не вычислить без измерения
const TRACK_ROW_HEIGHT: f32 = 30.0;
const TRACK_ROW_PADDING: f32 = 8.0;
const TRACK_ROW_GAP: f32 = 6.0;
// Толщина рамки выделенной строки
const SELECTION_STROKE_WIDTH: f32 = 1.5;
// Скорость автопрокрутки при выделении областью: точек в секунду на каждую точку,
// на которую курсор ушёл за край списка
const MARQUEE_AUTOSCROLL_SPEED: f32 = 8.0;

impl PlayerApp {
    /// Нижний блок списка: слева shuffle, справа две кнопки повтора
    pub(super) fn render_playback_modes(&mut self, ui: &mut Ui) {
        let list = &self.controller.player().track_list;
        let (shuffle, repeat) = (list.shuffle, list.repeat);
        let size = vec2(ADD_BUTTON_SIZE, ADD_BUTTON_SIZE);

        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
            let (clicked, active, hover) =
                widgets::toggle_button(ui, rect, "shuffle", shuffle, "Shuffle");
            paint_shuffle(ui, rect, active, hover);
            if clicked {
                self.controller.toggle_shuffle();
            }

            // Повторы прижаты вправо. Включена может быть только одна из кнопок —
            // это обеспечивает сам RepeatMode
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = ICON_BUTTON_GAP;
                // В right_to_left первой добавляется самая правая кнопка
                for (mode, id_salt, tooltip) in [
                    (RepeatMode::All, "repeat_all", "Repeat all"),
                    (RepeatMode::One, "repeat_one", "Repeat one"),
                ] {
                    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
                    let (clicked, active, hover) =
                        widgets::toggle_button(ui, rect, id_salt, repeat == mode, tooltip);
                    paint_repeat(ui, rect, mode == RepeatMode::One, active, hover);
                    if clicked {
                        self.controller.toggle_repeat(mode);
                    }
                }
            });
        });
    }

    pub(super) fn render_track_list(&mut self, ui: &mut Ui) {
        let current = self.controller.player().track_list.current_index;
        let track_count = self.controller.player().track_list.tracks.len();
        // Вся область панели — запоминаем до заголовка, чтобы «No tracks» встал по её центру
        let panel_rect = ui.max_rect();
        // Отступы задаёт Frame: у результата vertical_centered/ScrollArea::show их нет
        egui::Frame::new().inner_margin(16.0).show(ui, |ui| {
            widgets::block_title(ui, "Playlist", Align::LEFT, TEXT);
        });

        // Заглушка по центру всей панели, а не места под заголовком — иначе она
        // съезжает вниз на его высоту
        if track_count == 0 {
            if self.loaders.is_empty() {
                ui.painter().text(
                    panel_rect.center(),
                    Align2::CENTER_CENTER,
                    "No tracks",
                    FontId::proportional(14.0),
                    TEXT_FAINT,
                );
            }
            return;
        }

        // Клавиши списка: удалить выделенное, выделить всё, снять выделение
        let (delete, select_all, deselect) = ui.input(|i| {
            (
                i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace),
                i.modifiers.command && i.key_pressed(Key::A),
                i.key_pressed(Key::Escape),
            )
        });
        if select_all {
            self.selection.select_all(track_count);
        }
        if deselect {
            self.selection.clear();
        }

        // Пока `track_list` заимствован, менять плеер нельзя — запоминаем, что сделать, и делаем после
        let mut play = None;
        let mut remove = delete;
        let modifiers = ui.input(|i| i.modifiers);
        // show_rows берёт зазор между строками из Ui, в котором вызван
        ui.spacing_mut().item_spacing.y = TRACK_ROW_GAP;
        egui::ScrollArea::vertical()
            // Отступ задаётся самой ScrollArea: полоса прокрутки остаётся у края панели,
            // а отступ прокручивается вместе со списком. Frame внутри show_rows не годится —
            // его отступ сбил бы расчёт позиций строк
            .content_margin(egui::Margin {
                left: 16,
                right: 16,
                top: 0,
                bottom: 16,
            })
            // Рисуются только видимые строки: тысячи треков не тормозят прокрутку и анимации
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
                    egui::Id::unique("track_list_marquee"),
                    Sense::click_and_drag(),
                );
                if background.clicked() {
                    self.selection.clear();
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
                }

                let tracks = &self.controller.player().track_list.tracks;
                for index in rows {
                    let row = track_row(
                        ui,
                        index,
                        &tracks[index],
                        current == Some(index),
                        self.selection.contains(index),
                    );

                    if row.clicked() {
                        self.selection.click(index, modifiers);
                    }
                    if row.double_clicked() {
                        play = Some(index);
                    }
                    // Правый клик по невыделенному треку выделяет его — меню действует
                    // на выделение, как в Finder
                    if row.secondary_clicked() && !self.selection.contains(index) {
                        self.selection.click(index, egui::Modifiers::NONE);
                    }
                    row.context_menu(|ui| {
                        widgets::menu_style(ui);
                        let label = match self.selection.len() {
                            0 | 1 => "Remove from playlist".to_owned(),
                            count => format!("Remove {count} tracks from playlist"),
                        };
                        if widgets::menu_item(ui, &label).clicked() {
                            remove = true;
                        }
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

        if remove {
            self.remove_selected();
        }
        if let Some(index) = play {
            let result = self
                .controller
                .select(index)
                .and_then(|()| self.controller.play());
            self.set_result(result);
        }
    }

    /// Удаляет выделенные треки из списка
    pub(super) fn remove_selected(&mut self) {
        if self.selection.is_empty() {
            return;
        }
        let result = self.controller.remove_tracks(self.selection.indices());
        self.set_result(result);
        // Номера треков после удаления сдвинулись — старое выделение уже ни на что не указывает
        self.selection.clear();
    }
}

/// Строка списка треков фиксированной высоты: название слева (обрезается с «…»),
/// длительность справа. Возвращает `true`, если по строке кликнули.
fn track_row(
    ui: &mut Ui,
    index: usize,
    track: &Track,
    is_current: bool,
    is_selected: bool,
) -> egui::Response {
    let (row_rect, _) =
        ui.allocate_exact_size(vec2(ui.available_width(), TRACK_ROW_HEIGHT), Sense::hover());
    // Стабильный id строки: по нему egui хранит состояние анимаций. Id от позиции
    // не годится — при прокрутке строки меняют место
    let row_id = ui.scope_id().with(("track_row", index));
    // Обычный курсор, а не «рука»: клик выделяет, а не запускает трек — запуск двойным кликом
    let row = ui.interact(row_rect, row_id, Sense::click());

    // Активность, выделение и наведение меняются плавно: фон и цвет текста перетекают
    let active = ui.ctx().animate_bool_with_time_and_easing(
        row_id.with("active"),
        is_current,
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
        row.hovered() && !is_current,
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

    let text_color = TEXT_MUTED.lerp_to_gamma(TEXT, active);
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
