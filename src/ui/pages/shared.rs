//! Общее для страниц медиатеки: контекстные меню альбомов, исполнителей и треков,
//! выборки треков исполнителя.

use eframe::egui;

use crate::{
    core::library::groups::AlbumGroup,
    ui::{
        action::Action,
        app::PlayerApp,
        components::{self},
        dialogs::remove::GroupKind,
        i18n::{fill, t},
    },
};

/// Что сделать с альбомом или исполнителем — из контекстного меню или кнопкой на обложке
#[derive(Clone, Copy)]
pub(crate) enum GroupAction {
    /// Заменить плейлист их треками и играть
    Play,
    AddToPlaylist,
    /// Убрать их треки из библиотеки (файлы не трогает)
    Remove,
}

/// К каким плиткам относится действие
pub(crate) enum Target {
    /// Одна плитка: кнопка play на обложке
    One(usize),
    /// Выделенные: контекстное меню, клавиша Delete
    Selection,
}

/// Контекстное меню альбома или исполнителя. Действует на все выделенные плитки —
/// в подписях их число. Возвращает выбранное действие
pub(crate) fn group_context_menu(tile: &components::Tile, kind: GroupKind) -> Option<GroupAction> {
    let mut action = None;
    tile.response.context_menu(|ui| {
        components::menu_style(ui);
        let t = t();
        let items = match (tile.selected, kind) {
            (1, _) => [
                (t.play.to_owned(), GroupAction::Play),
                (t.add_to_playlist.to_owned(), GroupAction::AddToPlaylist),
                (t.remove_from_library.to_owned(), GroupAction::Remove),
            ],
            (count, kind) => {
                let word = match kind {
                    GroupKind::Album => &t.album_count,
                    GroupKind::Artist => &t.artist_count,
                    GroupKind::Songs => &t.track_count,
                };
                let items = t.count(count, word);
                let items = [("items", items.as_str())];
                [
                    (fill(t.play_items, &items), GroupAction::Play),
                    (
                        fill(t.add_items_to_playlist, &items),
                        GroupAction::AddToPlaylist,
                    ),
                    (
                        fill(t.remove_items_from_library, &items),
                        GroupAction::Remove,
                    ),
                ]
            }
        };
        for (label, menu_action) in items {
            if components::menu_item(ui, &label).clicked() {
                action = Some(menu_action);
            }
        }
    });
    action
}

/// Подписи меню списка треков медиатеки: добавить в плейлист и удалить из медиатеки.
/// Для нескольких выделенных — с их числом
pub(crate) fn track_menu_labels(selected: usize) -> (String, String) {
    let t = t();
    match selected {
        0 | 1 => (
            t.add_to_playlist.to_owned(),
            t.remove_from_library.to_owned(),
        ),
        count => {
            let items = t.count(count, &t.track_count);
            let items = [("items", items.as_str())];
            (
                fill(t.add_items_to_playlist, &items),
                fill(t.remove_items_from_library, &items),
            )
        }
    }
}

impl PlayerApp {
    /// Альбомы исполнителя `artist` (имя без учёта регистра, как при группировке),
    /// в порядке сетки
    pub(crate) fn artist_albums(&self, artist: &str) -> impl Iterator<Item = &AlbumGroup> {
        let artist = artist.to_lowercase();
        self.core
            .groups
            .albums
            .iter()
            .filter(move |album| album.artist.to_lowercase() == artist)
    }

    /// Номера всех треков исполнителя в библиотеке: альбом за альбомом в порядке сетки
    pub(crate) fn artist_track_indices(&self, artist: &str) -> Vec<usize> {
        self.artist_albums(artist)
            .flat_map(|album| album.tracks.iter().copied())
            .collect()
    }
}

/// Пункт меню альбомов или исполнителей `names` — в действие над их треками медиатеки
/// `tracks`, по порядку
pub(crate) fn group_action(
    action: GroupAction,
    kind: GroupKind,
    names: Vec<String>,
    tracks: Vec<usize>,
) -> Action {
    match action {
        GroupAction::Play => Action::PlayLibraryTracks(tracks),
        GroupAction::AddToPlaylist => Action::AddToPlaylist(tracks),
        // Сразу не удаляем — сначала спрашиваем
        GroupAction::Remove => Action::RequestGroupRemoval {
            kind,
            names,
            tracks,
        },
    }
}

/// Пункт меню списка треков медиатеки
pub(crate) enum TrackMenuItem {
    AddToPlaylist,
    RemoveFromLibrary,
}

/// Меню списка треков медиатеки: добавить в плейлист и удалить из медиатеки. Для
/// нескольких выделенных — с их числом. Возвращает выбранный пункт
pub(crate) fn track_menu(ui: &mut egui::Ui, selected: usize) -> Option<TrackMenuItem> {
    let (add_label, remove_label) = track_menu_labels(selected);
    if components::menu_item(ui, &add_label).clicked() {
        Some(TrackMenuItem::AddToPlaylist)
    } else if components::menu_item(ui, &remove_label).clicked() {
        Some(TrackMenuItem::RemoveFromLibrary)
    } else {
        None
    }
}
