use eframe::egui::{
    self, Align, Align2, CornerRadius, CursorIcon, FontId, Label, Layout, Pos2, Rect, RichText,
    Sense, Ui, pos2, vec2,
};
use std::time::Duration;

use crate::theme::{GRAY_1, GRAY_2, GRAY_3, GRAY_4, GRAY_5, RED};
use crate::{
    metadata::SUPPORTED_EXTENSIONS,
    player::{PlayerController, PlayerState},
    track::Track,
    ui,
    utils::{format_secs, paint_add, paint_play_pause, paint_skip, skip_hit_rect},
};

const PLAY_BUTTON_SIZE: f32 = 60.0;
const PLAY_MORPH_SECS: f32 = 0.25;
const ADD_BUTTON_SIZE: f32 = 24.0;
// Непрозрачность тёмной подложки под неактивным треком при наведении
const TRACK_HOVER_DARKEN: f32 = 0.35;
// Отступ между строками списка треков
const TRACK_ROW_GAP: f32 = 6.0;
// Длительность перехода, когда трек становится активным или перестаёт им быть
const TRACK_ACTIVE_FADE_SECS: f32 = 0.2;
const SKIP_BUTTON_SIZE: f32 = 40.0;
// Зазор между кнопкой play и кнопками prev/next
const SKIP_BUTTON_GAP: f32 = 4.0;
const SLIDER_TRACK_HEIGHT: f32 = 5.0;
const SLIDER_THUMB_RADIUS: f32 = 8.0;
const TRACK_LIST_MIN_WIDTH: f32 = 150.0;
const TRACK_LIST_MAX_WIDTH: f32 = 400.0;
// Сколько места под основной контент панель списка обязана оставить
const CONTENT_MIN_WIDTH: f32 = 260.0;

pub struct PlayerApp {
    controller: PlayerController,
    error: Option<String>,
    /// Позиция ползунка (в секундах), пока пользователь его тянет
    drag_secs: Option<f32>,
}

impl PlayerApp {
    pub fn new() -> Self {
        Self {
            controller: PlayerController::new(),
            error: None,
            drag_secs: None,
        }
    }

    fn set_result(&mut self, result: Result<(), String>) {
        self.error = result.err();
    }

    fn add_tracks(&mut self) {
        let Some(paths) = rfd::FileDialog::new()
            .add_filter("Audio", SUPPORTED_EXTENSIONS)
            .pick_files()
        else {
            // Пользователь закрыл диалог: оставляем текущий трек
            return;
        };

        // Неподходящие файлы пропускаем, остальные всё равно открываем
        let mut tracks = Vec::new();
        let mut errors = Vec::new();
        for path in paths {
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            match Track::from_path(path) {
                Ok(track) => tracks.push(track),
                Err(e) => errors.push(format!("{name}: {e}")),
            }
        }

        // Треки добавляются в конец очереди. Если ничего ещё не выбрано,
        // текущим становится первый из добавленных; иначе текущий трек не трогаем
        if !tracks.is_empty() {
            let first_new = self.controller.player().track_list.len();
            let nothing_selected = self.controller.player().current_index.is_none();
            self.controller.append_tracks(tracks);
            if nothing_selected && let Err(e) = self.controller.select(first_new) {
                errors.push(e);
            }
        }

        self.error = (!errors.is_empty()).then(|| errors.join("\n"));
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
        let text = RichText::new("Add tracks").size(18.0).color(GRAY_4);

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
            .on_hover_text("Add tracks")
            .clicked()
        {
            self.add_tracks();
        }
    }

    fn render_current_track_info(&mut self, ui: &mut Ui) {
        ui.vertical(|ui| {
            if let Some(track) = &self.controller.player().current_track {
                for line in [
                    &track.title,
                    &track.artist,
                    &track.album,
                    format_secs(track.duration_secs as f32).as_str(),
                ] {
                    if !line.is_empty() {
                        ui.label(RichText::new(line).size(16.0).color(GRAY_5));
                    }
                }
                ui.add_space(8.0);

                ui.vertical_centered(|ui| {
                    ui::render_album_cover(ui, track.album_cover.as_deref());
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
                GRAY_5,
            );
            // Справа — сколько осталось. Округляем вверх: format_secs отбрасывает дробную
            // часть, и иначе прошедшее + оставшееся давало бы на секунду меньше длительности
            let remaining = (duration - position).max(0.0).ceil();
            painter.text(
                row.right_center(),
                Align2::RIGHT_CENTER,
                format!("{}", format_secs(remaining)),
                font,
                GRAY_5,
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
        let prev = ui::icon_button(
            ui,
            skip_hit_rect(prev_rect),
            ui::HitShape::Rect,
            "prev",
            has_track,
        );
        paint_skip(ui, prev_rect, false, !has_track, prev.hover);

        // У play чёткий круг, поэтому и наведение считаем только внутри него
        let play = ui::icon_button(ui, play_rect, ui::HitShape::Circle, "play_pause", has_track);
        // 0.0 — треугольник play, 1.0 — полоски pause
        let morph = ui.ctx().animate_bool_with_time_and_easing(
            play.response.id,
            playing,
            PLAY_MORPH_SECS,
            egui::emath::easing::cubic_in_out,
        );
        paint_play_pause(ui, play_rect, morph, !has_track, play.hover);

        let next = ui::icon_button(
            ui,
            skip_hit_rect(next_rect),
            ui::HitShape::Rect,
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
        painter.rect_filled(track, CornerRadius::same(255), GRAY_1);

        let thumb_x = track.left() + track.width() * fraction;
        let filled = Rect::from_min_max(track.min, pos2(thumb_x, track.max.y));
        painter.rect_filled(filled, CornerRadius::same(255), GRAY_4);

        // Ползунок не вылезает за края окна. max/min вместо clamp: на очень узком
        // окне левая граница может оказаться правее правой, и clamp паникует
        let thumb_x = thumb_x
            .min(rect.right() - SLIDER_THUMB_RADIUS)
            .max(rect.left() + SLIDER_THUMB_RADIUS);
        painter.circle_filled(pos2(thumb_x, rect.center().y), SLIDER_THUMB_RADIUS, GRAY_4);
    }

    fn render_track_list(&mut self, ui: &mut Ui) {
        // Пока `track_list` заимствован, менять плеер нельзя — запоминаем клик и обрабатываем после цикла
        let mut clicked = None;
        let current = self.controller.player().current_index;
        // Вся область панели — запоминаем до заголовка, чтобы «No tracks» встал по её центру
        let panel_rect = ui.max_rect();
        // Отступы задаёт Frame: у результата vertical_centered/ScrollArea::show их нет
        egui::Frame::new().inner_margin(16.0).show(ui, |ui| {
            let row = ui
                .vertical_centered(|ui| {
                    ui.label(RichText::new("Playlist").size(16.0).color(GRAY_1));
                })
                .response
                .rect;

            // «+» прижат к правому краю строки, заголовок при этом остаётся по центру
            let add_rect = Rect::from_center_size(
                pos2(row.right() - ADD_BUTTON_SIZE / 2.0, row.center().y),
                vec2(ADD_BUTTON_SIZE, ADD_BUTTON_SIZE),
            );
            let add = ui::icon_button(ui, add_rect, ui::HitShape::Circle, "add_tracks", true);
            paint_add(ui, add_rect, add.hover);
            add.response.on_hover_text("Add tracks");
            if add.clicked {
                self.add_tracks();
            }
        });

        // Сверху без отступа — его уже дал заголовок
        let list_margin = egui::Margin {
            left: 16,
            right: 16,
            top: 0,
            bottom: 16,
        };
        // Заглушка по центру всей панели, а не места под заголовком — иначе она
        // съезжает вниз на его высоту
        if self.controller.player().track_list.is_empty() {
            ui.painter().text(
                panel_rect.center(),
                Align2::CENTER_CENTER,
                "No tracks",
                FontId::proportional(14.0),
                GRAY_3,
            );
            return;
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            // Frame внутри ScrollArea: полоса прокрутки остаётся у края панели,
            // а отступ прокручивается вместе со списком
            egui::Frame::new().inner_margin(list_margin).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = TRACK_ROW_GAP;

                for (index, track) in self.controller.player().track_list.iter().enumerate() {
                    // Стабильный id строки: по нему egui хранит состояние анимаций
                    let row_id = ui.scope_id().with(("track_row", index));
                    let is_current = current == Some(index);

                    // Активность плавно меняется от 0.0 до 1.0: фон и цвет текста
                    // перетекают, а не переключаются мгновенно
                    let active = ui.ctx().animate_bool_with_time_and_easing(
                        row_id.with("active"),
                        is_current,
                        TRACK_ACTIVE_FADE_SECS,
                        egui::emath::easing::cubic_in_out,
                    );
                    let text_color = GRAY_1.lerp_to_gamma(GRAY_5, active);

                    // begin/paint вместо show: фон рисуется после содержимого (под ним),
                    // поэтому его цвет можно выбрать, уже зная, наведён ли курсор на строку
                    let mut row_frame = egui::Frame::new()
                        .corner_radius(6)
                        .inner_margin(6.0)
                        .begin(ui);

                    // Аналог justify-content: space-between: время прижато вправо,
                    // название занимает оставшееся место слева и обрезается с «…»
                    row_frame.content_ui.horizontal(|ui| {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let duration = format_secs(track.duration_secs as f32);
                            let duration = RichText::new(duration).size(14.0).color(text_color);
                            ui.add(Label::new(duration).selectable(false));

                            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                                let title = if track.artist.is_empty() {
                                    track.title.clone()
                                } else {
                                    format!("{} — {}", track.title, track.artist)
                                };
                                let title = RichText::new(title).size(14.0).color(text_color);
                                ui.add(Label::new(title).selectable(false).truncate());
                            });
                        });
                    });

                    let row_rect = row_frame.allocate_space(ui).rect;
                    let row = ui
                        .interact(row_rect, row_id, Sense::click())
                        .on_hover_cursor(CursorIcon::PointingHand);

                    // Неактивный трек под курсором плавно затемняем
                    let hover = ui.ctx().animate_bool_with_time_and_easing(
                        row_id.with("hover"),
                        row.hovered() && !is_current,
                        ui::HOVER_FADE_SECS,
                        egui::emath::easing::cubic_out,
                    );
                    let hover_fill = GRAY_5.gamma_multiply(TRACK_HOVER_DARKEN * hover);
                    // Активный фон проявляется поверх подсветки наведения
                    row_frame.frame.fill = hover_fill.lerp_to_gamma(GRAY_2, active);
                    row_frame.paint(ui);

                    if row.clicked() {
                        clicked = Some(index);
                    }
                }
            });

            if let Some(index) = clicked {
                let result = self
                    .controller
                    .select(index)
                    .and_then(|()| self.controller.play());
                self.set_result(result);
            }
        });
    }
}

impl eframe::App for PlayerApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        // Трек доиграл — переходим к следующему в очереди
        if self.controller.poll_events()
            && let Err(e) = self.controller.play_next()
        {
            self.error = Some(e);
        }

        // Пока трек играет, обновляем время и ползунок
        if self.controller.player().state == PlayerState::Playing {
            ui.ctx().request_repaint_after(Duration::from_millis(100));
        }

        // Максимум зависит от ширины окна: при сужении окна панель сжимается первой,
        // а не выдавливает основной контент
        let actual_track_list_max_width = (ui.available_width() - CONTENT_MIN_WIDTH)
            .clamp(TRACK_LIST_MIN_WIDTH, TRACK_LIST_MAX_WIDTH);

        let track_list_frame = egui::Frame::new().fill(GRAY_4);
        egui::Panel::left("track_list")
            .frame(track_list_frame)
            .default_size(200.0)
            .size_range(TRACK_LIST_MIN_WIDTH..=actual_track_list_max_width)
            .resizable(true)
            .show_separator_line(false)
            .show(ui, |ui| {
                // Panel подгоняет ширину под содержимое, поэтому без этого она сжимается
                // обратно до минимума сразу после перетаскивания края
                ui.set_min_width(ui.available_width());
                self.render_track_list(ui);
            });

        // Панели нужно добавлять до CentralPanel — он занимает всё оставшееся место
        egui::Panel::bottom("footer")
            .frame(egui::Frame::new().fill(GRAY_2).inner_margin(16.0))
            .show_separator_line(false)
            .show(ui, |ui| {
                self.render_controls(ui);
            });

        let main_frame = egui::Frame::central_panel(ui.style())
            .fill(GRAY_2)
            .inner_margin(16.0);

        egui::CentralPanel::default()
            .frame(main_frame)
            .show(ui, |ui| {
                ui.vertical_centered_justified(|ui| {
                    // Ошибка видна всегда, а не только пока список пуст: она бывает
                    // и при воспроизведении. Идёт первой, чтобы кнопка «Add tracks»
                    // центрировалась в месте, которое осталось под ней
                    if let Some(error) = &self.error {
                        ui.label(RichText::new(error).color(RED));
                    }

                    if self.controller.player().track_list.is_empty() {
                        self.render_add_tracks_block(ui);
                    } else {
                        self.render_current_track_info(ui);
                    }
                })
            });
    }
}
