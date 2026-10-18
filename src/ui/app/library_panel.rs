//! Левая панель: библиотека — треки, сохранённые между запусками. Двойной клик добавляет
//! трек в плейлист и запускает его; из меню — добавить в плейлист или убрать из библиотеки.

use eframe::egui::{self, Align, Align2, FontId, Ui};

use super::{ListFocus, PlayerApp, loading::Destination};
use crate::ui::{
    theme::{TEXT, TEXT_FAINT},
    widgets,
};

impl PlayerApp {
    pub(super) fn render_library(&mut self, ui: &mut Ui) {
        // Вся область панели — запоминаем до заголовка, чтобы заглушка встала по её центру
        let panel_rect = ui.max_rect();
        egui::Frame::new().inner_margin(16.0).show(ui, |ui| {
            let row = widgets::block_title(ui, "Library", Align::LEFT, TEXT);
            self.render_add_button(ui, row, Destination::Library);
        });

        if self.library.tracks().is_empty() {
            ui.painter().text(
                panel_rect.center(),
                Align2::CENTER_CENTER,
                "Library is empty",
                FontId::proportional(14.0),
                TEXT_FAINT,
            );
            return;
        }

        // Пока библиотека заимствована списком, менять ничего нельзя — запоминаем, что сделать
        let mut add_to_playlist = false;
        let mut remove = false;
        let library = &self.library;
        let response = self.library_view.show(
            ui,
            library.tracks(),
            |index| widgets::RowState {
                current: false,
                missing: library.is_missing(index),
            },
            self.focus == ListFocus::Library,
            |ui, selected| {
                let (add_label, remove_label) = match selected {
                    0 | 1 => (
                        "Add to playlist".to_owned(),
                        "Remove from library".to_owned(),
                    ),
                    count => (
                        format!("Add {count} tracks to playlist"),
                        format!("Remove {count} tracks from library"),
                    ),
                };
                if widgets::menu_item(ui, &add_label).clicked() {
                    add_to_playlist = true;
                }
                if widgets::menu_item(ui, &remove_label).clicked() {
                    remove = true;
                }
            },
        );

        if response.interacted {
            self.focus = ListFocus::Library;
        }
        if add_to_playlist {
            let tracks = self.selected_library_tracks();
            self.add_to_playlist(tracks);
        }
        if remove || response.delete {
            let result = self.library.remove(self.library_view.selection());
            self.set_result(result);
            self.library_view.clear_selection();
        }
        if let Some(index) = response.activated {
            self.play_from_library(index);
        }
    }

    fn selected_library_tracks(&self) -> Vec<crate::library::Track> {
        let tracks = self.library.tracks();
        self.library_view
            .selection()
            .iter()
            .filter_map(|&index| tracks.get(index).cloned())
            .collect()
    }

    /// Двойной клик в библиотеке: трек сразу играет. Если он уже есть в плейлисте, играет
    /// оттуда, иначе встаёт в конец — повторный запуск не плодит копии в плейлисте
    fn play_from_library(&mut self, index: usize) {
        let Some(track) = self.library.tracks().get(index).cloned() else {
            return;
        };
        let playlist = &self.controller.player().track_list.tracks;
        let playlist_index = match playlist.iter().position(|t| t.path == track.path) {
            Some(existing) => existing,
            None => {
                self.controller.append_tracks([track]);
                self.controller.player().track_list.tracks.len() - 1
            }
        };
        let result = self
            .controller
            .select(playlist_index)
            .and_then(|()| self.controller.play());
        self.set_result(result);
    }
}
