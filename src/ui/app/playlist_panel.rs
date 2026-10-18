//! Правая панель: плейлист — что слушаем сейчас. Треки сюда попадают из библиотеки или
//! кнопкой «+» мимо библиотеки. Внизу — shuffle и повтор.

use eframe::egui::{self, Align, Align2, FontId, Layout, Sense, Ui, vec2};

use super::{ADD_BUTTON_SIZE, ListFocus, PlayerApp, loading::Destination};
use crate::{
    library::Track,
    playback::RepeatMode,
    ui::{
        format::plural,
        icons::{paint_repeat, paint_shuffle},
        theme::{TEXT, TEXT_FAINT},
        widgets,
    },
};

// Зазор между кнопками повтора в нижнем блоке
const ICON_BUTTON_GAP: f32 = 4.0;

impl PlayerApp {
    pub(super) fn render_playlist(&mut self, ui: &mut Ui) {
        // Вся область панели — запоминаем до заголовка, чтобы заглушка встала по её центру
        let panel_rect = ui.max_rect();
        egui::Frame::new().inner_margin(16.0).show(ui, |ui| {
            // Рядом с заголовком — сколько треков в плейлисте; пустой — без числа:
            // там и так «No tracks»
            let count = self.controller.player().track_list.tracks.len();
            let row = if count == 0 {
                widgets::block_title(ui, "Playlist", Align::LEFT, TEXT)
            } else {
                let note = format!("· {}", plural(count, "track", "tracks"));
                widgets::block_title_with_note(ui, "Playlist", &note)
            };
            self.render_add_button(ui, row, Destination::Playlist);
        });

        if self.controller.player().track_list.tracks.is_empty() {
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

        // Пока плейлист заимствован списком, менять плеер нельзя — запоминаем, что сделать
        let mut remove = false;
        let list = &self.controller.player().track_list;
        let current = list.current_index;
        let response = self.playlist_view.show(
            ui,
            &list.tracks,
            |index| widgets::RowState {
                current: current == Some(index),
                missing: false,
            },
            self.focus == ListFocus::Playlist,
            |ui, selected| {
                let label = match selected {
                    0 | 1 => "Remove from playlist".to_owned(),
                    count => format!("Remove {count} tracks from playlist"),
                };
                if widgets::menu_item(ui, &label).clicked() {
                    remove = true;
                }
            },
        );

        if response.interacted {
            self.focus = ListFocus::Playlist;
        }
        if remove || response.delete {
            let result = self
                .controller
                .remove_tracks(self.playlist_view.selection());
            self.set_result(result);
            self.playlist_view.clear_selection();
        }
        if let Some(index) = response.activated {
            let result = self
                .controller
                .select(index)
                .and_then(|()| self.controller.play());
            self.set_result(result);
        }
    }

    /// Добавляет треки в конец плейлиста. Если ничего ещё не выбрано, текущим становится
    /// первый из добавленных; иначе текущий трек не трогаем
    pub(super) fn add_to_playlist(&mut self, tracks: Vec<Track>) {
        if tracks.is_empty() {
            return;
        }
        let first_new = self.controller.player().track_list.tracks.len();
        let nothing_selected = self.controller.player().current_track.is_none();
        self.controller.append_tracks(tracks);
        if nothing_selected {
            let result = self.controller.select(first_new);
            self.set_result(result);
        }
    }

    /// Нижний блок плейлиста: слева shuffle, справа две кнопки повтора
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
}
