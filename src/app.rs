use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, CursorIcon, FontId, Label, Layout, Pos2, Rect,
    RichText, Sense, Ui, pos2, vec2,
};
use std::time::Duration;

use crate::theme::{BG, FG, RED, TRACK};
use crate::{
    metadata::SUPPORTED_EXTENSIONS,
    player::{PlayerController, PlayerState},
    track::Track,
    ui,
    utils::{format_secs, paint_play_pause},
};

const PLAY_BUTTON_SIZE: f32 = 64.0;
const PLAY_MORPH_SECS: f32 = 0.25;
const SLIDER_TRACK_HEIGHT: f32 = 5.0;
const SLIDER_THUMB_RADIUS: f32 = 8.0;

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

        // В очередь идут все треки, первый из них становится текущим
        if !tracks.is_empty() {
            self.controller.set_list(tracks);
            if let Err(e) = self.controller.select(0) {
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

    fn render_header(&mut self, ui: &mut Ui) {
        ui.vertical_centered(|ui| {
            let open =
                egui::Button::new(RichText::new("Open file").size(18.0).color(FG)).frame(false);
            if ui
                .add(open)
                .on_hover_cursor(CursorIcon::PointingHand)
                .clicked()
            {
                self.add_tracks();
            }

            // Временно показываем состояние плеера, чтобы видеть, что он реагирует на кнопки
            // let state = self.controller.player().state;
            // ui.label(RichText::new(format!("{state:?}")).size(18.0).color(RED));

            if let Some(error) = &self.error {
                ui.label(RichText::new(error).color(RED));
            }
        });
    }

    fn render_current_track_info(&mut self, ui: &mut Ui) {
        ui.vertical(|ui| {
            if let Some(track) = &self.controller.player().current_track {
                ui.add_space(8.0);
                for line in [&track.title, &track.artist, &track.album] {
                    if !line.is_empty() {
                        ui.label(RichText::new(line).size(16.0).color(FG));
                    }
                }
                ui.add_space(8.0);

                ui::render_album_cover(ui, track.album_cover.as_deref());
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
                FG,
            );
            painter.text(
                row.right_center(),
                Align2::RIGHT_CENTER,
                format_secs(duration),
                font,
                FG,
            );
        }

        let button_rect =
            Rect::from_center_size(row.center(), vec2(PLAY_BUTTON_SIZE, PLAY_BUTTON_SIZE));
        let sense = if has_track {
            Sense::click()
        } else {
            Sense::hover()
        };
        let mut button = ui.interact(button_rect, ui.scope_id().with("play_pause"), sense);
        if has_track {
            button = button.on_hover_cursor(CursorIcon::PointingHand);
        }
        // 0.0 — треугольник play, 1.0 — полоски pause
        let morph = ui.ctx().animate_bool_with_time_and_easing(
            button.id,
            playing,
            PLAY_MORPH_SECS,
            egui::emath::easing::cubic_in_out,
        );
        paint_play_pause(ui, button_rect, morph, !has_track, button.hovered());

        if button.clicked() {
            self.toggle_play();
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
        painter.rect_filled(track, CornerRadius::same(255), TRACK);

        let thumb_x = track.left() + track.width() * fraction;
        let filled = Rect::from_min_max(track.min, pos2(thumb_x, track.max.y));
        painter.rect_filled(filled, CornerRadius::same(255), FG);

        // Ползунок не вылезает за края окна
        let thumb_x = thumb_x.clamp(
            rect.left() + SLIDER_THUMB_RADIUS,
            rect.right() - SLIDER_THUMB_RADIUS,
        );
        painter.circle_filled(pos2(thumb_x, rect.center().y), SLIDER_THUMB_RADIUS, FG);
    }

    fn render_list(&mut self, ui: &mut Ui) {
        // Пока `list` заимствован, менять плеер нельзя — запоминаем клик и обрабатываем после цикла
        let mut clicked = None;
        let current = self.controller.player().current_index;

        ui.vertical(|ui| {
            ui.label(RichText::new("List"));

            for (index, track) in self.controller.player().list.iter().enumerate() {
                // Текущий трек подсвечиваем фоном
                let fill = if current == Some(index) {
                    BG
                } else {
                    Color32::TRANSPARENT
                };
                let row = egui::Frame::new()
                    .fill(fill)
                    .corner_radius(6)
                    .inner_margin(4.0)
                    .show(ui, |ui| {
                        // Аналог justify-content: space-between: время прижато вправо,
                        // название занимает оставшееся место слева и обрезается с «…»
                        ui.horizontal(|ui| {
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                let duration = format_secs(track.duration_secs as f32);
                                let duration = RichText::new(duration).size(14.0).color(FG);
                                ui.add(Label::new(duration).selectable(false));

                                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                                    let title = if track.artist.is_empty() {
                                        track.title.clone()
                                    } else {
                                        format!("{} — {}", track.title, track.artist)
                                    };
                                    let title = RichText::new(title).size(14.0).color(FG);
                                    ui.add(Label::new(title).selectable(false).truncate());
                                });
                            });
                        });
                    })
                    .response
                    .interact(Sense::click())
                    .on_hover_cursor(CursorIcon::PointingHand);

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

        let list_frame = egui::Frame::new().fill(TRACK).inner_margin(16.0);
        egui::Panel::left("list")
            .frame(list_frame)
            .default_size(200.0)
            .size_range(150.0..=400.0)
            .resizable(true)
            .show_separator_line(false)
            .show(ui, |ui| {
                // Panel подгоняет ширину под содержимое, поэтому без этого она сжимается
                // обратно до минимума сразу после перетаскивания края
                ui.set_min_width(ui.available_width());
                self.render_list(ui);
            });

        let frame = egui::Frame::central_panel(ui.style())
            .fill(BG)
            .inner_margin(16);

        egui::CentralPanel::default().frame(frame).show(ui, |ui| {
            ui.vertical_centered_justified(|ui| {
                self.render_header(ui);
                ui.add_space(8.0);
                self.render_current_track_info(ui);
                ui.add_space(16.0);
                self.render_controls(ui);
            })
        });
    }
}
