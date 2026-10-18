use eframe::egui::{
    self, Align, Align2, CornerRadius, CursorIcon, FontId, Key, Label, Layout, Pos2, Rect,
    RichText, Sense, Ui, pos2, vec2,
};
use std::{
    collections::{BTreeSet, VecDeque},
    time::{Duration, Instant},
};

use crate::theme::{
    HIGHLIGHT, HIGHLIGHT_STRONG, MARQUEE_FILL, MARQUEE_STROKE, SLIDER_TRACK, TEXT, TEXT_FAINT,
    TEXT_MUTED, WINDOW_TINT,
};
use crate::{
    loader::{Finished, Loader, Source},
    metadata::SUPPORTED_EXTENSIONS,
    player::{PlayerController, PlayerState, RepeatMode},
    track::Track,
    ui_components,
    utils::{
        format_secs, paint_add, paint_play_pause, paint_repeat, paint_shuffle, paint_skip,
        skip_hit_rect,
    },
};

const PLAY_BUTTON_SIZE: f32 = 60.0;
const PLAY_MORPH_SECS: f32 = 0.25;
const ADD_BUTTON_SIZE: f32 = 24.0;
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
// Фоновое добавление: как часто проверять готовность, через сколько показывать прогресс
// (быстрое добавление обходится без него) и сколько минимум его держать, чтобы не мелькал
const LOADER_POLL_INTERVAL: Duration = Duration::from_millis(50);
const LOADER_SHOW_DELAY: Duration = Duration::from_millis(400);
const LOADER_MIN_VISIBLE: Duration = Duration::from_millis(500);
// Длительность перехода, когда трек становится активным или перестаёт им быть
const SKIP_BUTTON_SIZE: f32 = 40.0;
// Зазор между кнопкой play и кнопками prev/next
const SKIP_BUTTON_GAP: f32 = 4.0;
const SLIDER_TRACK_HEIGHT: f32 = 5.0;
const SLIDER_THUMB_RADIUS: f32 = 8.0;
// Ширина боковых панелей (библиотека и список треков): пределы ресайза и стартовая ширина
const SIDE_PANEL_MIN_WIDTH: f32 = 180.0;
const SIDE_PANEL_MAX_WIDTH: f32 = 420.0;
const SIDEBAR_DEFAULT_WIDTH: f32 = 240.0;
const TRACK_LIST_DEFAULT_WIDTH: f32 = 300.0;
// Минимальная ширина центральной области (CentralPanel): её обеспечивают максимумы боковых панелей
const CONTENT_MIN_WIDTH: f32 = 260.0;
pub struct PlayerApp {
    controller: PlayerController,
    error: Option<String>,
    /// Позиция ползунка (в секундах), пока пользователь его тянет
    drag_secs: Option<f32>,
    /// Фоновые добавления треков. Обрабатываются по очереди: треки второго появятся
    /// в списке после треков первого, даже если он закончит раньше
    loaders: VecDeque<Loader>,
    /// С какого момента виден прогресс текущего добавления; `None` — не виден
    loader_shown_at: Option<Instant>,
    /// Выделенные треки в списке
    selection: Selection,
    /// Выделение областью мыши, пока его тянут
    marquee: Option<Marquee>,
}

impl PlayerApp {
    pub fn new() -> Self {
        Self {
            controller: PlayerController::new(),
            error: None,
            drag_secs: None,
            loaders: VecDeque::new(),
            loader_shown_at: None,
            selection: Selection::default(),
            marquee: None,
        }
    }

    /// Запоминает ошибку для плашки. Успех её не сбрасывает: плашка закрывается только
    /// кнопкой «OK», иначе ошибка исчезала бы сама при следующем удачном действии
    fn set_result(&mut self, result: Result<(), String>) {
        if let Err(e) = result {
            self.error = Some(e);
        }
    }

    /// Выбор отдельных файлов
    fn add_files(&mut self) {
        let Some(paths) = rfd::FileDialog::new()
            .add_filter("Audio", SUPPORTED_EXTENSIONS)
            .pick_files()
        else {
            // Пользователь закрыл диалог: оставляем текущий трек
            return;
        };
        self.loaders.push_back(Loader::start(Source::Files(paths)));
    }

    /// Выбор папки: добавляются все аудиофайлы в ней, включая вложенные папки
    fn add_folder(&mut self) {
        let Some(dir) = rfd::FileDialog::new().pick_folder() else {
            return;
        };
        self.loaders.push_back(Loader::start(Source::Folder(dir)));
    }

    /// Забирает готовые треки у фонового добавления и кладёт их в конец списка.
    /// Вызывается каждый кадр
    fn poll_loaders(&mut self, ctx: &egui::Context) {
        let Some(loader) = self.loaders.front_mut() else {
            return;
        };
        loader.update();

        // Прогресс показываем, только если добавление затянулось: несколько файлов
        // добавляются быстрее, и тогда прогресс не мелькает вовсе
        if self.loader_shown_at.is_none() && loader.elapsed() >= LOADER_SHOW_DELAY {
            self.loader_shown_at = Some(Instant::now());
        }
        // Уже показанный прогресс держим не меньше LOADER_MIN_VISIBLE — иначе он мог бы
        // появиться и сразу исчезнуть, это выглядит хуже, чем не показывать его вовсе
        let can_hide = self
            .loader_shown_at
            .is_none_or(|shown_at| shown_at.elapsed() >= LOADER_MIN_VISIBLE);

        if loader.is_finished() && can_hide {
            let finished = self.loaders.pop_front().map(Loader::finish);
            self.loader_shown_at = None;
            if let Some(finished) = finished {
                self.apply_loaded(finished);
            }
        }

        // Пока идёт добавление, кадры нужны и без действий пользователя:
        // иначе прогресс обновлялся бы только при движении мыши
        if !self.loaders.is_empty() {
            ctx.request_repaint_after(LOADER_POLL_INTERVAL);
        }
    }

    /// Кладёт добавленные треки в конец списка — все разом
    fn apply_loaded(&mut self, finished: Finished) {
        if !finished.tracks.is_empty() {
            // Если ничего ещё не выбрано, текущим становится первый из добавленных;
            // иначе текущий трек не трогаем
            let first_new = self.controller.player().track_list.tracks.len();
            let nothing_selected = self.controller.player().track_list.current_index.is_none();
            self.controller.append_tracks(finished.tracks);
            if nothing_selected && let Err(e) = self.controller.select(first_new) {
                self.error = Some(e);
            }
        }

        if finished.total == Some(0) {
            self.error = Some("No supported audio files found".into());
        } else if !finished.errors.is_empty() {
            self.error = Some(summarize_errors(&finished.errors));
        }
    }

    /// Отменяет текущее добавление: уже прочитанные треки не добавляются
    fn cancel_loading(&mut self) {
        self.loaders.pop_front();
        self.loader_shown_at = None;
    }

    fn toggle_play(&mut self) {
        let result = if self.controller.player().state == PlayerState::Playing {
            self.controller.pause()
        } else {
            self.controller.play()
        };
        self.set_result(result);
    }

    /// Кнопка по центру всего оставшегося места
    fn render_add_tracks_block(&mut self, ui: &mut Ui) {
        let text = RichText::new("Open files").size(18.0).color(TEXT);

        // Layout в egui не центрирует по вертикали, поэтому считаем размер кнопки
        // заранее и ставим её в центр свободной области сами
        let text_size = egui::WidgetText::from(text.clone())
            .into_galley(
                ui,
                Some(egui::TextWrapMode::Extend),
                f32::INFINITY,
                egui::TextStyle::Button,
            )
            .size();
        let button_size = text_size + 2.0 * ui.spacing().button_padding;
        let button_rect =
            Rect::from_center_size(ui.available_rect_before_wrap().center(), button_size);

        let open = egui::Button::new(text).frame(false);
        if ui
            .put(button_rect, open)
            .on_hover_cursor(CursorIcon::PointingHand)
            .on_hover_text("Open files")
            .clicked()
        {
            self.add_files();
        }
    }

    fn render_current_track_info(&mut self, ui: &mut Ui) {
        ui.vertical(|ui| {
            if let Some(track) = &self.controller.player().current_track {
                // Название ярче, остальное — второстепенным цветом
                for (line, color) in [
                    (track.title.as_str(), TEXT),
                    (&track.artist, TEXT_MUTED),
                    (&track.album, TEXT_MUTED),
                    (format_secs(track.duration_secs as f32).as_str(), TEXT_MUTED),
                ] {
                    if !line.is_empty() {
                        ui.label(RichText::new(line).size(16.0).color(color));
                    }
                }
                ui.add_space(8.0);

                ui.vertical_centered(|ui| {
                    ui_components::album_cover(ui, track.album_cover.as_deref());
                });
            }
        });
    }

    fn render_controls(&mut self, ui: &mut Ui) {
        let player = self.controller.player();
        let has_track = player.current_track.is_some();
        let playing = player.state == PlayerState::Playing;
        let duration = player
            .current_track
            .as_ref()
            .map_or(0.0, |t| t.duration_secs as f32);

        let position = self
            .drag_secs
            .unwrap_or(self.controller.position_ms() as f32 / 1000.0)
            .min(duration);

        // Ряд: время слева, кнопка по центру, длительность справа
        let (row, _) =
            ui.allocate_exact_size(vec2(ui.available_width(), PLAY_BUTTON_SIZE), Sense::hover());

        if has_track && duration > 0.0 {
            let font = FontId::proportional(16.0);
            let painter = ui.painter();
            painter.text(
                row.left_center(),
                Align2::LEFT_CENTER,
                format_secs(position),
                font.clone(),
                TEXT_MUTED,
            );
            // Справа — сколько осталось. Округляем вверх: format_secs отбрасывает дробную
            // часть, и иначе прошедшее + оставшееся давало бы на секунду меньше длительности
            let remaining = (duration - position).max(0.0).ceil();
            painter.text(
                row.right_center(),
                Align2::RIGHT_CENTER,
                format_secs(remaining),
                font,
                TEXT_MUTED,
            );
        }

        // prev и next стоят по бокам от play
        let play_rect =
            Rect::from_center_size(row.center(), vec2(PLAY_BUTTON_SIZE, PLAY_BUTTON_SIZE));
        let skip_offset = vec2(
            (PLAY_BUTTON_SIZE + SKIP_BUTTON_SIZE) / 2.0 + SKIP_BUTTON_GAP,
            0.0,
        );
        let skip_size = vec2(SKIP_BUTTON_SIZE, SKIP_BUTTON_SIZE);
        let prev_rect = Rect::from_center_size(row.center() - skip_offset, skip_size);
        let next_rect = Rect::from_center_size(row.center() + skip_offset, skip_size);

        // prev/next реагируют только на контур иконки, а не на весь квадрат вокруг неё
        let prev = ui_components::icon_button(
            ui,
            skip_hit_rect(prev_rect),
            ui_components::HitShape::Rect,
            "prev",
            has_track,
        );
        paint_skip(ui, prev_rect, false, !has_track, prev.hover);

        // У play чёткий круг, поэтому и наведение считаем только внутри него
        let play = ui_components::icon_button(
            ui,
            play_rect,
            ui_components::HitShape::Circle,
            "play_pause",
            has_track,
        );
        // 0.0 — треугольник play, 1.0 — полоски pause
        let morph = ui.ctx().animate_bool_with_time_and_easing(
            play.response.id,
            playing,
            PLAY_MORPH_SECS,
            egui::emath::easing::cubic_in_out,
        );
        paint_play_pause(ui, play_rect, morph, !has_track, play.hover);

        let next = ui_components::icon_button(
            ui,
            skip_hit_rect(next_rect),
            ui_components::HitShape::Rect,
            "next",
            has_track,
        );
        paint_skip(ui, next_rect, true, !has_track, next.hover);

        if prev.clicked {
            let result = self.controller.skip_prev();
            self.set_result(result);
        }
        if play.clicked {
            self.toggle_play();
        }
        if next.clicked {
            let result = self.controller.skip_next();
            self.set_result(result);
        }

        ui.add_space(1.0);
        self.render_progress_bar(ui, position, duration, has_track && duration > 0.0);
    }

    fn render_progress_bar(&mut self, ui: &mut Ui, position: f32, duration: f32, enabled: bool) {
        let sense = if enabled {
            Sense::click_and_drag()
        } else {
            Sense::hover()
        };
        let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), sense);

        let value_at =
            |pos: Pos2| ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0) * duration;

        if enabled {
            let response = response.on_hover_cursor(CursorIcon::PointingHand);

            if response.dragged()
                && let Some(pos) = response.interact_pointer_pos()
            {
                self.drag_secs = Some(value_at(pos));
            }

            if response.drag_stopped() || response.clicked() {
                // При отпускании курсор может быть уже недоступен — берём последнюю позицию драга
                let secs = response
                    .interact_pointer_pos()
                    .map(value_at)
                    .or(self.drag_secs);
                self.drag_secs = None;

                if let Some(secs) = secs {
                    let result = self.controller.seek((secs * 1000.0) as u64);
                    self.set_result(result);
                }
            }
        }

        let fraction = if duration > 0.0 {
            (self.drag_secs.unwrap_or(position) / duration).clamp(0.0, 1.0)
        } else {
            0.0
        };

        let painter = ui.painter();
        let track = Rect::from_center_size(rect.center(), vec2(rect.width(), SLIDER_TRACK_HEIGHT));
        painter.rect_filled(track, CornerRadius::same(255), SLIDER_TRACK);

        let thumb_x = track.left() + track.width() * fraction;
        let filled = Rect::from_min_max(track.min, pos2(thumb_x, track.max.y));
        painter.rect_filled(filled, CornerRadius::same(255), TEXT);

        // Ползунок не вылезает за края окна. max/min вместо clamp: на очень узком
        // окне левая граница может оказаться правее правой, и clamp паникует
        let thumb_x = thumb_x
            .min(rect.right() - SLIDER_THUMB_RADIUS)
            .max(rect.left() + SLIDER_THUMB_RADIUS);
        painter.circle_filled(pos2(thumb_x, rect.center().y), SLIDER_THUMB_RADIUS, TEXT);
    }

    /// Содержимое самой левой боковой панели
    fn render_sidebar(&mut self, ui: &mut Ui) {
        egui::Frame::new().inner_margin(16.0).show(ui, |ui| {
            let row = ui_components::block_title(ui, "Library", Align::LEFT, TEXT);

            // «+» прижат к правому краю строки, заголовок при этом остаётся по центру
            let add_rect = Rect::from_center_size(
                pos2(row.right() - ADD_BUTTON_SIZE / 2.0, row.center().y),
                vec2(ADD_BUTTON_SIZE, ADD_BUTTON_SIZE),
            );
            let add = ui_components::icon_button(
                ui,
                add_rect,
                ui_components::HitShape::Circle,
                "add",
                true,
            );
            paint_add(ui, add_rect, add.hover);

            // Меню под «+», выровнено по его правому краю. Открывается и закрывается
            // по нашему `clicked` — он учитывает только клики внутри круга
            egui::Popup::menu(&add.response)
                .open_memory(add.clicked.then_some(egui::SetOpenCommand::Toggle))
                .align(egui::RectAlign::BOTTOM_END)
                .gap(4.0)
                .show(|ui| {
                    ui_components::menu_style(ui);
                    if ui_components::menu_item(ui, "Add files").clicked() {
                        self.add_files();
                    }
                    if ui_components::menu_item(ui, "Add folder").clicked() {
                        self.add_folder();
                    }
                });
        });
    }

    /// Нижний блок списка: слева shuffle, справа две кнопки повтора
    fn render_playback_modes(&mut self, ui: &mut Ui) {
        let list = &self.controller.player().track_list;
        let (shuffle, repeat) = (list.shuffle, list.repeat);
        let size = vec2(ADD_BUTTON_SIZE, ADD_BUTTON_SIZE);

        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
            let (clicked, active, hover) =
                ui_components::toggle_button(ui, rect, "shuffle", shuffle, "Shuffle");
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
                        ui_components::toggle_button(ui, rect, id_salt, repeat == mode, tooltip);
                    paint_repeat(ui, rect, mode == RepeatMode::One, active, hover);
                    if clicked {
                        self.controller.toggle_repeat(mode);
                    }
                }
            });
        });
    }

    fn render_track_list(&mut self, ui: &mut Ui) {
        let current = self.controller.player().track_list.current_index;
        let track_count = self.controller.player().track_list.tracks.len();
        // Вся область панели — запоминаем до заголовка, чтобы «No tracks» встал по её центру
        let panel_rect = ui.max_rect();
        // Отступы задаёт Frame: у результата vertical_centered/ScrollArea::show их нет
        egui::Frame::new().inner_margin(16.0).show(ui, |ui| {
            ui_components::block_title(ui, "Playlist", Align::LEFT, TEXT);
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
                        ui_components::menu_style(ui);
                        let label = match self.selection.len() {
                            0 | 1 => "Remove from playlist".to_owned(),
                            count => format!("Remove {count} tracks from playlist"),
                        };
                        if ui_components::menu_item(ui, &label).clicked() {
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
    fn remove_selected(&mut self) {
        if self.selection.is_empty() {
            return;
        }
        let result = self.controller.remove_tracks(self.selection.indices());
        self.set_result(result);
        // Номера треков после удаления сдвинулись — старое выделение уже ни на что не указывает
        self.selection.clear();
    }
}

impl eframe::App for PlayerApp {
    /// Окно прозрачное: под ним система рисует размытие, поэтому сами ничего не заливаем
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0; 4]
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        // Трек доиграл — переходим к следующему в очереди
        if self.controller.poll_events()
            && let Err(e) = self.controller.play_next()
        {
            self.error = Some(e);
        }

        self.poll_loaders(ui.ctx());

        // Пока трек играет, обновляем время и ползунок
        if self.controller.player().state == PlayerState::Playing {
            ui.ctx().request_repaint_after(Duration::from_millis(100));
        }

        // Тонировка системного размытия — первой, под всеми панелями. Других заливок у панелей
        // нет: стекло по всему окну одного цвета
        ui.painter().rect_filled(ui.max_rect(), 0, WINDOW_TINT);

        // У CentralPanel нет своей минимальной ширины — он получает остаток после боковых
        // панелей. Поэтому минимум обеспечивают их максимумы: каждая может занять только то,
        // что останется после минимумов всего, что ещё не размещено. При сужении окна боковые
        // панели сжимаются первыми, а центральная не становится уже CONTENT_MIN_WIDTH
        let sidebar_max_width = (ui.available_width() - SIDE_PANEL_MIN_WIDTH - CONTENT_MIN_WIDTH)
            .clamp(SIDE_PANEL_MIN_WIDTH, SIDE_PANEL_MAX_WIDTH);

        // egui подсвечивает край панели белой линией при наведении и перетаскивании —
        // толщину берёт из hovered/active fg_stroke родительского Ui. Вместо неё у нас своя
        // линия в тон интерфейса (paint_resize_line), поэтому на время боковых панелей
        // толщина обнуляется. Меняется только толщина: цвет текста виджетов внутри не страдает
        let widgets = &mut ui.style_mut().visuals.widgets;
        let edge_strokes = (
            widgets.hovered.fg_stroke.width,
            widgets.active.fg_stroke.width,
        );
        widgets.hovered.fg_stroke.width = 0.0;
        widgets.active.fg_stroke.width = 0.0;

        // Библиотека — у левого края окна
        let sidebar = egui::Panel::left("library_sidebar")
            .frame(egui::Frame::new())
            .default_size(SIDEBAR_DEFAULT_WIDTH)
            .size_range(SIDE_PANEL_MIN_WIDTH..=sidebar_max_width)
            .resizable(true)
            .show_separator_line(false)
            .show(ui, |ui| {
                self.render_sidebar(ui);
            });

        // Считается после боковой панели: available_width уже без её ширины
        let track_list_max_width = (ui.available_width() - CONTENT_MIN_WIDTH)
            .clamp(SIDE_PANEL_MIN_WIDTH, SIDE_PANEL_MAX_WIDTH);

        // Список треков — у правого края окна
        let track_list = egui::Panel::right("track_list")
            .frame(egui::Frame::new())
            .default_size(TRACK_LIST_DEFAULT_WIDTH)
            .size_range(SIDE_PANEL_MIN_WIDTH..=track_list_max_width)
            .resizable(true)
            .show_separator_line(false)
            .show(ui, |ui| {
                // Panel подгоняет ширину под содержимое, поэтому без этого она сжимается
                // обратно до минимума сразу после перетаскивания края
                ui.set_min_width(ui.available_width());

                // Блок прижат к низу панели. Добавляется до списка: панель сразу забирает
                // своё место, а ScrollArea получает только оставшееся и под блок не заходит
                egui::Panel::bottom("track_list_bottom")
                    // Без своего frame панель залила бы себя фоном по умолчанию
                    .frame(egui::Frame::new().inner_margin(16.0))
                    .resizable(false)
                    .show_separator_line(false)
                    .show(ui, |ui| {
                        self.render_playback_modes(ui);
                    });

                self.render_track_list(ui);
            });

        let widgets = &mut ui.style_mut().visuals.widgets;
        (
            widgets.hovered.fg_stroke.width,
            widgets.active.fg_stroke.width,
        ) = edge_strokes;

        // Панели нужно добавлять до CentralPanel — он занимает всё оставшееся место.
        // Добавлена после боковых, поэтому тянется только под центральной карточкой
        egui::Panel::bottom("audio_controls")
            .frame(egui::Frame::new())
            .show_separator_line(false)
            .show(ui, |ui| {
                // Без стекла: контролы лежат прямо на фоне окна
                egui::Frame::new().inner_margin(16.0).show(ui, |ui| {
                    self.render_controls(ui);
                });
            });

        // Центральная область без своего фона: под ней тонированное системное размытие
        egui::CentralPanel::default()
            .frame(egui::Frame::new())
            .show(ui, |ui| {
                egui::Frame::new().inner_margin(16.0).show(ui, |ui| {
                    ui.vertical_centered_justified(|ui| {
                        if self.controller.player().track_list.tracks.is_empty() {
                            self.render_add_tracks_block(ui);
                        } else {
                            self.render_current_track_info(ui);
                        }
                    })
                });
            });

        // Плашки поверх всего окна, по центру сверху: прогресс добавления (только когда
        // poll_loaders решил его показать) и под ним ошибка — пока не нажмут «OK»
        let mut banner_top = ui_components::BANNER_TOP;
        if self.loader_shown_at.is_some()
            && let Some(loader) = self.loaders.front()
        {
            let (cancel, bottom) = ui_components::loading_banner(ui.ctx(), loader.progress());
            banner_top = bottom - ui.ctx().content_rect().top() + ui_components::BANNER_GAP;
            if cancel {
                self.cancel_loading();
            }
        }
        if let Some(error) = &self.error
            && ui_components::error_banner(ui.ctx(), error, banner_top)
        {
            self.error = None;
        }

        // Линии ресайза — последними, поверх всего, на границе панелей — там, где egui даёт тянуть край
        let sidebar_rect = sidebar.response.rect;
        ui_components::paint_resize_line(ui, sidebar_rect.right(), sidebar_rect.y_range());
        let track_list_rect = track_list.response.rect;
        ui_components::paint_resize_line(ui, track_list_rect.left(), track_list_rect.y_range());
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
        ui_components::TRACK_ACTIVE_FADE_SECS,
        egui::emath::easing::cubic_in_out,
    );
    let selected = ui.ctx().animate_bool_with_time_and_easing(
        row_id.with("selected"),
        is_selected,
        ui_components::HOVER_FADE_SECS,
        egui::emath::easing::cubic_out,
    );
    let hover = ui.ctx().animate_bool_with_time_and_easing(
        row_id.with("hover"),
        row.hovered() && !is_current,
        ui_components::HOVER_FADE_SECS,
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

/// Выделение областью мыши. Начало хранится в координатах содержимого списка, а не экрана:
/// пока тянут, список может прокручиваться, и экранная точка начала уезжает вместе с ним
struct Marquee {
    start: Pos2,
    /// Что было выделено до начала (с ⌘/Ctrl область добавляется к нему)
    base: BTreeSet<usize>,
}

/// Выделение в списке треков — как в Finder: клик выделяет один трек, ⌘/Ctrl+клик
/// добавляет или убирает трек, Shift+клик выделяет диапазон от последнего клика
#[derive(Default)]
struct Selection {
    indices: BTreeSet<usize>,
    /// Откуда считается диапазон при Shift+клике
    anchor: Option<usize>,
}

impl Selection {
    fn click(&mut self, index: usize, modifiers: egui::Modifiers) {
        if modifiers.shift
            && let Some(anchor) = self.anchor
        {
            // ⌘/Ctrl вместе с Shift добавляет диапазон к уже выделенному
            if !modifiers.command {
                self.indices.clear();
            }
            self.indices.extend(anchor.min(index)..=anchor.max(index));
        } else if modifiers.command {
            if !self.indices.remove(&index) {
                self.indices.insert(index);
            }
            self.anchor = Some(index);
        } else {
            self.indices = BTreeSet::from([index]);
            self.anchor = Some(index);
        }
    }

    /// Выделение областью: `base` плюс строки `range`. `anchor` — строка, с которой начали
    /// тянуть: от неё считается диапазон при следующем Shift+клике
    fn select_range(
        &mut self,
        base: &BTreeSet<usize>,
        range: std::ops::RangeInclusive<usize>,
        anchor: usize,
    ) {
        self.indices = base.iter().copied().chain(range).collect();
        self.anchor = Some(anchor);
    }

    fn select_all(&mut self, len: usize) {
        self.indices = (0..len).collect();
    }

    fn clear(&mut self) {
        self.indices.clear();
        self.anchor = None;
    }

    fn contains(&self, index: usize) -> bool {
        self.indices.contains(&index)
    }

    fn len(&self) -> usize {
        self.indices.len()
    }

    fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    fn indices(&self) -> &BTreeSet<usize> {
        &self.indices
    }
}

/// Текст ошибки добавления: первые несколько файлов и сколько ещё не добавилось —
/// при тысячах файлов полный список не поместился бы на экран
fn summarize_errors(errors: &[String]) -> String {
    const SHOWN: usize = 3;
    let mut text = errors[..errors.len().min(SHOWN)].join("\n");
    if errors.len() > SHOWN {
        text += &format!(
            "\n…and {} more files could not be added",
            errors.len() - SHOWN
        );
    }
    text
}

#[cfg(test)]
mod tests {
    use super::Selection;
    use eframe::egui::Modifiers;

    fn selected(selection: &Selection) -> Vec<usize> {
        selection.indices().iter().copied().collect()
    }

    #[test]
    fn click_selects_only_one_track() {
        let mut selection = Selection::default();
        selection.click(2, Modifiers::NONE);
        selection.click(5, Modifiers::NONE);
        assert_eq!(selected(&selection), [5]);
    }

    #[test]
    fn command_click_toggles_tracks() {
        let mut selection = Selection::default();
        selection.click(2, Modifiers::NONE);
        selection.click(5, Modifiers::COMMAND);
        selection.click(7, Modifiers::COMMAND);
        selection.click(2, Modifiers::COMMAND);
        assert_eq!(selected(&selection), [5, 7]);
    }

    #[test]
    fn shift_click_selects_range_from_anchor() {
        let mut selection = Selection::default();
        selection.click(5, Modifiers::NONE);
        selection.click(2, Modifiers::SHIFT);
        assert_eq!(selected(&selection), [2, 3, 4, 5]);
        // Якорь остаётся на 5: новый Shift+клик заменяет диапазон, а не расширяет его
        selection.click(7, Modifiers::SHIFT);
        assert_eq!(selected(&selection), [5, 6, 7]);
    }

    #[test]
    fn command_shift_click_adds_range() {
        let mut selection = Selection::default();
        selection.click(0, Modifiers::NONE);
        selection.click(5, Modifiers::COMMAND);
        selection.click(7, Modifiers::COMMAND | Modifiers::SHIFT);
        assert_eq!(selected(&selection), [0, 5, 6, 7]);
    }

    #[test]
    fn marquee_range_replaces_or_adds_to_base() {
        let mut selection = Selection::default();
        selection.select_range(&Default::default(), 3..=6, 6);
        assert_eq!(selected(&selection), [3, 4, 5, 6]);

        let base = [0, 9].into_iter().collect();
        selection.select_range(&base, 4..=5, 4);
        assert_eq!(selected(&selection), [0, 4, 5, 9]);
        // Shift+клик после области считает диапазон от строки, с которой её начали тянуть
        selection.click(7, Modifiers::SHIFT);
        assert_eq!(selected(&selection), [4, 5, 6, 7]);
    }
}
