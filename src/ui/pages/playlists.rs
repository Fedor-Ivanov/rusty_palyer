//! Сохранённые плейлисты сеткой: обложка (по первым трекам), название и число треков.

use eframe::egui::{Align, Ui};
use std::path::PathBuf;

use super::{
    TILE_RADIUS, placeholder,
    search::{contains_query, normalized_query},
    shared::overlay_play_button,
};
use crate::{
    core::playlists::PlaylistId,
    ui::{
        action::Action,
        app::{ListFocus, PlayerApp},
        components::{self, TileLine},
        covers::CoverSize,
        i18n::{fill, t},
        navigation::Route,
    },
};

/// Обложку плейлиста ищем среди стольких первых его треков
pub(crate) const PLAYLIST_COVER_TRACKS: usize = 8;

/// Пункт контекстного меню плейлиста
enum PlaylistMenuItem {
    Play,
    Rename,
    /// Удалить выделенные плейлисты — один или несколько
    Delete,
}

impl PlayerApp {
    /// Все сохранённые плейлисты сеткой. Клик открывает плейлист, кнопка на обложке играет
    /// его; из меню — сыграть, переименовать, удалить. Если выделено несколько — меню только
    /// удаляет их все
    pub(crate) fn render_playlists(&mut self, ui: &mut Ui) {
        let all = self.core.playlists.all();
        if all.is_empty() {
            placeholder(ui, t().no_playlists);
            return;
        }
        // Поиск — по названию
        let query = normalized_query(&self.search.query);
        let shown: Vec<usize> = (0..all.len())
            .filter(|&index| contains_query(&all[index].name, &query))
            .collect();
        if shown.is_empty() {
            placeholder(ui, t().no_results);
            return;
        }
        // Номера плиток относятся к плейлистам и запросу — при их изменении выделение
        // сбрасывается
        self.playlists_grid
            .set_scope(&format!("{}/{query}", self.core.playlists.revision()));

        let covers = &mut self.covers;
        let mut menu: Option<(PlaylistMenuItem, PlaylistId)> = None;
        let response = self.playlists_grid.show(
            ui,
            shown.len(),
            2,
            Align::Min,
            self.focus == ListFocus::Grid,
            |ui, index, image_rect, tile| {
                let playlist = &all[shown[index]];
                let candidates = cover_candidates(&playlist.tracks);
                let cover = covers.get(&candidates, CoverSize::Thumbnail);
                components::paint_cover(ui, image_rect, cover, TILE_RADIUS);
                // Кнопка play на обложке — как у альбомов; у пустого плейлиста играть нечего
                if !playlist.tracks.is_empty()
                    && overlay_play_button(ui, image_rect, tile, &format!("play_playlist {index}"))
                {
                    menu = Some((PlaylistMenuItem::Play, playlist.id));
                }
                if let Some(item) = playlist_context_menu(tile) {
                    menu = Some((item, playlist.id));
                }
                vec![
                    TileLine {
                        text: playlist.name.clone(),
                        primary: true,
                    },
                    TileLine {
                        text: t().count(playlist.tracks.len(), &t().track_count),
                        primary: false,
                    },
                ]
            },
        );
        if response.interacted {
            self.focus = ListFocus::Grid;
        }

        let id_of = |index: usize| all[shown[index]].id;
        // Меню по правому клику и Delete действуют на выделение: правый клик выделяет
        // плитку под курсором, если она не была выделена
        let selected = || {
            self.playlists_grid
                .selection()
                .iter()
                .map(|&index| id_of(index))
                .collect::<Vec<_>>()
        };
        let action = if let Some(index) = response.opened {
            Some(Action::Navigate(Route::Playlist(id_of(index))))
        } else if let Some((item, id)) = menu {
            Some(match item {
                PlaylistMenuItem::Play => Action::PlayPlaylist(id),
                PlaylistMenuItem::Rename => Action::RequestRenamePlaylist(id),
                PlaylistMenuItem::Delete => Action::RequestPlaylistDeletion(selected()),
            })
        } else if response.delete {
            Some(Action::RequestPlaylistDeletion(selected()))
        } else {
            None
        };
        if let Some(action) = action {
            self.dispatch(action);
        }
    }
}

/// Где искать обложку плейлиста: его первые треки
pub(crate) fn cover_candidates(tracks: &[crate::core::library::Track]) -> Vec<PathBuf> {
    tracks
        .iter()
        .take(PLAYLIST_COVER_TRACKS)
        .map(|track| track.path.clone())
        .collect()
}

/// Контекстное меню плейлиста. Выделен один — сыграть, переименовать, удалить его;
/// несколько — только удалить их все: сыграть или переименовать сразу несколько нельзя
fn playlist_context_menu(tile: &components::Tile) -> Option<PlaylistMenuItem> {
    let mut chosen = None;
    tile.response.context_menu(|ui| {
        components::menu_style(ui);
        if tile.selected > 1 {
            let items = t().count(tile.selected, &t().playlist_count);
            let label = fill(t().delete_items, &[("items", &items)]);
            if components::menu_item(ui, &label).clicked() {
                chosen = Some(PlaylistMenuItem::Delete);
            }
            return;
        }
        for (label, item) in [
            (t().play, PlaylistMenuItem::Play),
            (t().rename, PlaylistMenuItem::Rename),
            (t().delete, PlaylistMenuItem::Delete),
        ] {
            if components::menu_item(ui, label).clicked() {
                chosen = Some(item);
            }
        }
    });
    chosen
}
