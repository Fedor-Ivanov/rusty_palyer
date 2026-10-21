//! Навигация: что открыто в центральной области. Одно значение `Route` вместо отдельных
//! полей «вид», «исполнитель», «альбом» — недопустимые сочетания (альбом в Songs) не выразить.
//! Переходы — только через `PlayerApp::navigate`: там же всё, что меняется вместе с ними.

use super::app::PlayerApp;
use crate::{
    core::{library::groups::AlbumGroup, playlists::PlaylistId},
    ui::i18n,
};

/// Раздел — верхний уровень навигации: пункт меню и заголовок
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Section {
    NowPlaying,
    Artists,
    Albums,
    Songs,
    Playlists,
}

impl Section {
    /// Разделы медиатеки — пункты меню слева, по порядку
    pub(crate) const LIBRARY: [Section; 4] = [
        Section::Artists,
        Section::Albums,
        Section::Songs,
        Section::Playlists,
    ];

    /// Название на языке интерфейса
    pub(crate) fn title(self) -> &'static str {
        let t = i18n::t();
        match self {
            Self::NowPlaying => t.now_playing,
            Self::Artists => t.artists,
            Self::Albums => t.albums,
            Self::Songs => t.songs,
            Self::Playlists => t.playlists,
        }
    }
}

/// Что открыто в центральной области
#[derive(Clone, PartialEq)]
pub(crate) enum Route {
    NowPlaying,
    Artists,
    /// Профиль исполнителя: его альбомы
    Artist(String),
    /// Альбом, открытый из профиля исполнителя `artist`
    ArtistAlbum {
        artist: String,
        album: AlbumKey,
    },
    Albums,
    /// Альбом, открытый из всех альбомов
    Album(AlbumKey),
    Songs,
    Playlists,
    /// Сохранённый плейлист
    Playlist(PlaylistId),
}

impl Route {
    /// Раздел, внутри которого открыта страница
    pub(crate) fn section(&self) -> Section {
        match self {
            Self::NowPlaying => Section::NowPlaying,
            Self::Artists | Self::Artist(_) | Self::ArtistAlbum { .. } => Section::Artists,
            Self::Albums | Self::Album(_) => Section::Albums,
            Self::Songs => Section::Songs,
            Self::Playlists | Self::Playlist(_) => Section::Playlists,
        }
    }

    /// Открытый альбом — откуда бы его ни открыли
    pub(crate) fn album(&self) -> Option<&AlbumKey> {
        match self {
            Self::Album(album) | Self::ArtistAlbum { album, .. } => Some(album),
            _ => None,
        }
    }

    /// Уровень выше: из альбома — туда, откуда его открыли, из профиля — к исполнителям
    pub(crate) fn parent(&self) -> Route {
        match self {
            Self::ArtistAlbum { artist, .. } => Self::Artist(artist.clone()),
            Self::Artist(_) => Self::Artists,
            Self::Album(_) => Self::Albums,
            Self::Playlist(_) => Self::Playlists,
            route => route.clone(),
        }
    }
}

impl From<Section> for Route {
    /// Раздел целиком — верхний уровень, без открытых исполнителя и альбома
    fn from(section: Section) -> Self {
        match section {
            Section::NowPlaying => Self::NowPlaying,
            Section::Artists => Self::Artists,
            Section::Albums => Self::Albums,
            Section::Songs => Self::Songs,
            Section::Playlists => Self::Playlists,
        }
    }
}

/// Какой альбом открыт. Альбом — пара «название + исполнитель», как при группировке;
/// хранятся имена, а не номер: номера альбомов меняются вместе с библиотекой
#[derive(Clone, PartialEq)]
pub(crate) struct AlbumKey {
    pub(crate) title: String,
    pub(crate) artist: String,
}

impl AlbumKey {
    pub(crate) fn of(album: &AlbumGroup) -> Self {
        Self {
            title: album.title.clone(),
            artist: album.artist.clone(),
        }
    }

    /// Сравнение без учёта регистра — как при группировке
    pub(crate) fn matches(&self, album: &AlbumGroup) -> bool {
        album.title.to_lowercase() == self.title.to_lowercase()
            && album.artist.to_lowercase() == self.artist.to_lowercase()
    }
}

impl PlayerApp {
    /// Открывает `route`. Здесь же всё, что меняется вместе с переходом:
    /// - поиск относится к разделу — при смене раздела очищается;
    /// - выделение треков альбома относится к открытому альбому — при смене альбома сбрасывается
    pub(crate) fn navigate(&mut self, route: Route) {
        if route == self.route {
            return;
        }
        if route.section() != self.route.section() && !self.search.is_empty() {
            self.search = Default::default();
            // Строки Songs во время поиска — найденные треки: выделение относилось к ним
            self.library_view.clear_selection();
        }
        if route.album() != self.route.album() {
            self.album_tracks_view.clear_selection();
        }
        // Выделение треков плейлиста относится к открытому плейлисту
        if !matches!((&route, &self.route), (Route::Playlist(a), Route::Playlist(b)) if a == b) {
            self.playlist_tracks_view.clear_selection();
        }
        self.route = route;
    }
}

#[cfg(test)]
mod tests {
    use super::{AlbumKey, Route, Section};

    fn album() -> AlbumKey {
        AlbumKey {
            title: "Brol".into(),
            artist: "Angèle".into(),
        }
    }

    #[test]
    fn album_belongs_to_the_section_it_was_opened_from() {
        let from_profile = Route::ArtistAlbum {
            artist: "Angèle".into(),
            album: album(),
        };
        assert!(from_profile.section() == Section::Artists);
        assert!(Route::Album(album()).section() == Section::Albums);
        assert!(from_profile.album() == Some(&album()));
        assert!(Route::Artist("Angèle".into()).album().is_none());
    }

    #[test]
    fn parent_goes_one_level_up() {
        let from_profile = Route::ArtistAlbum {
            artist: "Angèle".into(),
            album: album(),
        };
        assert!(from_profile.parent() == Route::Artist("Angèle".into()));
        assert!(Route::Artist("Angèle".into()).parent() == Route::Artists);
        assert!(Route::Album(album()).parent() == Route::Albums);
        assert!(Route::Songs.parent() == Route::Songs);
    }
}
