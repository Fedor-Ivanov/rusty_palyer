//! Песни: все треки медиатеки списком (или найденные поиском).

use eframe::egui::{self, Ui};

use super::{
    placeholder,
    shared::{TrackMenuItem, track_menu},
};
use crate::ui::{
    action::Action,
    app::{ListFocus, PlayerApp},
    components::{self, ButtonStyle},
    i18n::t,
};

impl PlayerApp {
    /// Все треки библиотеки списком — или найденные поиском. Двойной клик добавляет трек
    /// в плейлист и запускает его; из меню — добавить в плейлист или убрать из библиотеки
    pub(crate) fn render_songs(&mut self, ui: &mut Ui) {
        if self.state.library.tracks().is_empty() {
            placeholder(ui, t().library_empty);
            return;
        }
        self.refresh_found_songs();
        if self
            .search
            .found_songs
            .as_ref()
            .is_some_and(|found| found.indices.is_empty())
        {
            placeholder(ui, t().no_results);
            return;
        }

        // «Play all» — показанные треки в порядке списка заменяют плейлист и играют
        let mut play_all = false;
        egui::Frame::new()
            .inner_margin(egui::Margin {
                left: 16,
                right: 16,
                top: 0,
                bottom: 16,
            })
            .show(ui, |ui| {
                play_all =
                    components::pill_button(ui, t().play_all, ButtonStyle::Primary).clicked();
            });
        if play_all {
            self.dispatch(Action::PlayLibraryTracks(self.shown_song_indices()));
        }

        // Номера строк списка — номера в `found` (во время поиска) или сразу в медиатеке.
        // Меню знает только, сколько строк выделено: какие — узнаём после списка
        let mut menu = None;
        let library = &self.state.library;
        let found = self.search.found_songs.as_ref();
        let tracks = found.map_or(library.tracks(), |found| &found.tracks);
        let response = self.library_view.show(
            ui,
            tracks,
            |index| components::RowState {
                current: false,
                missing: library.is_missing(found.map_or(index, |found| found.indices[index])),
            },
            self.focus == ListFocus::Library,
            |ui, selected| {
                if let Some(item) = track_menu(ui, selected) {
                    menu = Some(item);
                }
            },
        );

        if response.interacted {
            self.focus = ListFocus::Library;
        }
        let selected: Vec<usize> = self
            .library_view
            .selection()
            .iter()
            .filter_map(|&index| self.song_index(index))
            .collect();
        let action = match menu {
            Some(TrackMenuItem::AddToPlaylist) => Some(Action::AddToPlaylist(selected)),
            Some(TrackMenuItem::RemoveFromLibrary) => Some(Action::RequestSongsRemoval(selected)),
            None if response.delete => Some(Action::RequestSongsRemoval(selected)),
            None => response
                .activated
                .and_then(|index| self.song_index(index))
                .map(Action::PlayFromLibrary),
        };
        if let Some(action) = action {
            self.dispatch(action);
        }
    }

    /// Номер в библиотеке трека из строки `row` списка Songs
    pub(crate) fn song_index(&self, row: usize) -> Option<usize> {
        match &self.search.found_songs {
            Some(found) => found.indices.get(row).copied(),
            None => (row < self.state.library.tracks().len()).then_some(row),
        }
    }

    /// Номера в библиотеке всех треков, показанных в Songs, по порядку
    pub(crate) fn shown_song_indices(&self) -> Vec<usize> {
        match &self.search.found_songs {
            Some(found) => found.indices.clone(),
            None => (0..self.state.library.tracks().len()).collect(),
        }
    }
}
