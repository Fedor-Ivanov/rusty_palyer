//! Диалог «удалить из медиатеки?»: что удаляется и подтверждение.

use eframe::egui::{self};
use std::{collections::HashSet, path::PathBuf};

use crate::{
    core::library::Track,
    ui::{
        action::Action,
        app::PlayerApp,
        components::{self},
        i18n::{fill, t},
    },
};

/// Что удаляется из библиотеки: альбом, исполнитель или отдельные треки — от этого
/// зависят тексты диалога удаления
#[derive(Clone, Copy)]
pub(crate) enum GroupKind {
    Album,
    Artist,
    /// Треки, выделенные в списке (Songs или страница альбома)
    Songs,
}

/// Альбомы, исполнители или треки, удаление которых из библиотеки ждёт подтверждения
pub(crate) struct LibraryRemoval {
    pub(crate) kind: GroupKind,
    /// Названия альбомов, имена исполнителей или название трека — для текста диалога
    pub(crate) names: Vec<String>,
    /// Файлы их треков
    pub(crate) paths: HashSet<PathBuf>,
}

impl PlayerApp {
    /// Просит подтвердить удаление треков библиотеки с номерами `indices` (сами файлы
    /// не трогаются). Удаляются они после ответа в диалоге — см. `render_removal_dialog`
    pub(crate) fn request_songs_removal(&mut self, indices: &[usize]) {
        let tracks: Vec<&Track> = indices
            .iter()
            .filter_map(|&index| self.state.library.tracks().get(index))
            .collect();
        let names = tracks.iter().map(|track| track.title.clone()).collect();
        let paths: HashSet<PathBuf> = tracks.iter().map(|track| track.path.clone()).collect();
        if !paths.is_empty() {
            self.pending_removal = Some(LibraryRemoval {
                kind: GroupKind::Songs,
                names,
                paths,
            });
        }
    }

    /// Просит подтвердить удаление альбомов или исполнителей `names` — треков медиатеки
    /// с номерами `tracks`. Запоминаются пути, а не номера: пока диалог открыт, медиатека
    /// может измениться
    pub(crate) fn request_group_removal(
        &mut self,
        kind: GroupKind,
        names: Vec<String>,
        tracks: &[usize],
    ) {
        let paths = self.state.track_paths(tracks);
        self.pending_removal = Some(LibraryRemoval { kind, names, paths });
    }

    /// Диалог «точно удалить из библиотеки?» — пока ждём ответа
    pub(crate) fn render_removal_dialog(&mut self, ctx: &egui::Context) {
        let Some(removal) = &self.pending_removal else {
            return;
        };
        let t = t();
        let count = |word| t.count(removal.names.len(), word);
        let tracks = t.count(removal.paths.len(), &t.track_count);
        let (title, message) = match (removal.kind, removal.names.as_slice()) {
            (GroupKind::Album, [album]) => (
                t.remove_album_title.to_owned(),
                fill(t.remove_album_message, &[("album", album)]),
            ),
            (GroupKind::Album, _) => {
                let albums = count(&t.album_count);
                (
                    fill(t.remove_albums_title, &[("albums", &albums)]),
                    fill(
                        t.remove_albums_message,
                        &[("albums", &albums), ("tracks", &tracks)],
                    ),
                )
            }
            (GroupKind::Artist, [artist]) => (
                t.remove_artist_title.to_owned(),
                fill(t.remove_artist_message, &[("artist", artist)]),
            ),
            (GroupKind::Artist, _) => {
                let artists = count(&t.artist_count);
                (
                    fill(t.remove_artists_title, &[("artists", &artists)]),
                    fill(
                        t.remove_artists_message,
                        &[("artists", &artists), ("tracks", &tracks)],
                    ),
                )
            }
            (GroupKind::Songs, [song]) => (
                t.remove_song_title.to_owned(),
                fill(t.remove_song_message, &[("song", song)]),
            ),
            (GroupKind::Songs, _) => {
                let songs = count(&t.song_count);
                (
                    fill(t.remove_songs_title, &[("songs", &songs)]),
                    fill(t.remove_songs_message, &[("songs", &songs)]),
                )
            }
        };
        let answer =
            components::confirm_dialog(ctx, "remove_from_library", &title, &message, t.remove);
        match answer {
            components::Confirm::Pending => {}
            components::Confirm::Cancelled => self.pending_removal = None,
            components::Confirm::Confirmed => {
                if let Some(removal) = self.pending_removal.take() {
                    self.dispatch(Action::RemoveFromLibrary(removal.paths));
                }
            }
        }
    }
}
