//! Общее для страниц медиатеки: контекстные меню альбомов, исполнителей и треков,
//! выборки треков исполнителя.

use eframe::egui;

use crate::{
    core::library::{Track, groups::AlbumGroup},
    core::playlists::PlaylistId,
    ui::style::icons::paint_overlay_play,
    ui::{
        action::Action,
        app::PlayerApp,
        components::{self},
        dialogs::remove::GroupKind,
        i18n::{fill, t},
        navigation::{AlbumKey, Route},
    },
};

/// Кнопка play поверх обложки в сетке: размер и отступ от угла обложки
const OVERLAY_PLAY_SIZE: f32 = 32.0;
const OVERLAY_PLAY_INSET: f32 = 8.0;

/// Кнопка play в левом верхнем углу обложки плитки `tile` — проявляется при наведении
/// на плитку. Возвращает `true`, если по ней кликнули
pub(crate) fn overlay_play_button(
    ui: &mut egui::Ui,
    image_rect: egui::Rect,
    tile: &components::Tile,
    id_salt: &str,
) -> bool {
    if tile.hover == 0.0 {
        return false;
    }
    let play_rect = egui::Rect::from_min_size(
        image_rect.min + egui::vec2(OVERLAY_PLAY_INSET, OVERLAY_PLAY_INSET),
        egui::vec2(OVERLAY_PLAY_SIZE, OVERLAY_PLAY_SIZE),
    );
    let play = components::icon_button(ui, play_rect, components::HitShape::Circle, id_salt, true);
    paint_overlay_play(ui, play_rect, tile.hover, play.hover);
    play.clicked
}

/// Куда добавить треки из подменю «Add to playlist ›»
#[derive(Clone, Copy)]
pub(crate) enum PlaylistChoice {
    /// В новый плейлист — сначала спросим название
    New,
    Existing(PlaylistId),
}

/// Сохранённые плейлисты для подменю «Add to playlist ›»: номер и название
pub(crate) type PlaylistEntries = [(PlaylistId, String)];

/// Подменю «Add to playlist ›»: «New playlist…» и сохранённые плейлисты.
/// Возвращает выбор
pub(crate) fn playlist_submenu(
    ui: &mut egui::Ui,
    playlists: &PlaylistEntries,
) -> Option<PlaylistChoice> {
    let mut chosen = None;
    components::submenu(ui, t().add_to_playlist, |ui| {
        if components::menu_item(ui, t().new_playlist).clicked() {
            chosen = Some(PlaylistChoice::New);
        }
        if !playlists.is_empty() {
            ui.separator();
        }
        for (id, name) in playlists {
            if components::menu_item(ui, name).clicked() {
                chosen = Some(PlaylistChoice::Existing(*id));
            }
        }
    });
    chosen
}

/// Что сделать с альбомом или исполнителем — из контекстного меню или кнопкой на обложке
#[derive(Clone, Copy)]
pub(crate) enum GroupAction {
    /// Заменить очередь их треками и играть
    Play,
    AddToQueue,
    /// Их треки — в сохранённый плейлист
    AddToPlaylist(PlaylistChoice),
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
pub(crate) fn group_context_menu(
    tile: &components::Tile,
    kind: GroupKind,
    playlists: &PlaylistEntries,
) -> Option<GroupAction> {
    let mut action = None;
    tile.response.context_menu(|ui| {
        components::menu_style(ui);
        let t = t();
        let items = match (tile.selected, kind) {
            (1, _) => [
                (t.play.to_owned(), GroupAction::Play),
                (t.add_to_queue.to_owned(), GroupAction::AddToQueue),
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
                    (fill(t.add_items_to_queue, &items), GroupAction::AddToQueue),
                    (
                        fill(t.remove_items_from_library, &items),
                        GroupAction::Remove,
                    ),
                ]
            }
        };
        let [play, add_to_queue, remove] = items;
        for (label, menu_action) in [play, add_to_queue] {
            if components::menu_item(ui, &label).clicked() {
                action = Some(menu_action);
            }
        }
        if let Some(choice) = playlist_submenu(ui, playlists) {
            action = Some(GroupAction::AddToPlaylist(choice));
        }
        if components::menu_item(ui, &remove.0).clicked() {
            action = Some(remove.1);
        }
    });
    action
}

/// Подписи меню списка треков медиатеки: добавить в плейлист и удалить из медиатеки.
/// Для нескольких выделенных — с их числом
pub(crate) fn track_menu_labels(selected: usize) -> (String, String) {
    let t = t();
    match selected {
        0 | 1 => (t.add_to_queue.to_owned(), t.remove_from_library.to_owned()),
        count => {
            let items = t.count(count, &t.track_count);
            let items = [("items", items.as_str())];
            (
                fill(t.add_items_to_queue, &items),
                fill(t.remove_items_from_library, &items),
            )
        }
    }
}

impl PlayerApp {
    /// Куда ведёт «Go to album» для трека: страница его альбома — если трек в медиатеке
    /// и у него известный альбом (см. `Core::album_of`). Иначе пункта в меню нет
    pub(crate) fn album_route(&mut self, track: &Track) -> Option<Route> {
        if !self.core.library.paths().contains(&track.path) {
            return None;
        }
        self.core
            .album_of(track)
            .map(|album| Route::Album(AlbumKey::of(album)))
    }

    /// Сохранённые плейлисты для подменю «Add to playlist ›»
    pub(crate) fn playlist_entries(&self) -> Vec<(PlaylistId, String)> {
        self.core
            .playlists
            .all()
            .iter()
            .map(|playlist| (playlist.id, playlist.name.clone()))
            .collect()
    }

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
        GroupAction::AddToQueue => Action::AddToQueue(tracks),
        GroupAction::AddToPlaylist(PlaylistChoice::New) => Action::RequestNewPlaylist(tracks),
        GroupAction::AddToPlaylist(PlaylistChoice::Existing(id)) => {
            Action::AddToPlaylist { id, tracks }
        }
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
    AddToQueue,
    AddToPlaylist(PlaylistChoice),
    /// Открыть альбом трека
    GoToAlbum(Route),
    RemoveFromLibrary,
}

/// Меню списка треков медиатеки: добавить в очередь, добавить в сохранённый плейлист,
/// перейти к альбому и удалить из медиатеки. Для нескольких выделенных — с их числом.
/// `album` — куда ведёт «Go to album» (см. `PlayerApp::album_route`); `None` — пункта нет:
/// выделено несколько треков, альбом неизвестен или мы уже на его странице.
/// Возвращает выбранный пункт
pub(crate) fn track_menu(
    ui: &mut egui::Ui,
    selected: usize,
    playlists: &PlaylistEntries,
    album: Option<&Route>,
) -> Option<TrackMenuItem> {
    let (add_label, remove_label) = track_menu_labels(selected);
    let mut chosen = None;
    if components::menu_item(ui, &add_label).clicked() {
        chosen = Some(TrackMenuItem::AddToQueue);
    }
    if let Some(choice) = playlist_submenu(ui, playlists) {
        chosen = Some(TrackMenuItem::AddToPlaylist(choice));
    }
    if let Some(route) = album
        && components::menu_item(ui, t().go_to_album).clicked()
    {
        chosen = Some(TrackMenuItem::GoToAlbum(route.clone()));
    }
    if components::menu_item(ui, &remove_label).clicked() {
        chosen = Some(TrackMenuItem::RemoveFromLibrary);
    }
    chosen
}

impl TrackMenuItem {
    /// Пункт меню — в действие над выделенными треками медиатеки `selected`
    pub(crate) fn into_action(self, selected: Vec<usize>) -> Action {
        match self {
            TrackMenuItem::AddToQueue => Action::AddToQueue(selected),
            TrackMenuItem::AddToPlaylist(PlaylistChoice::New) => {
                Action::RequestNewPlaylist(selected)
            }
            TrackMenuItem::AddToPlaylist(PlaylistChoice::Existing(id)) => Action::AddToPlaylist {
                id,
                tracks: selected,
            },
            TrackMenuItem::GoToAlbum(route) => Action::Navigate(route),
            TrackMenuItem::RemoveFromLibrary => Action::RequestSongsRemoval(selected),
        }
    }
}
