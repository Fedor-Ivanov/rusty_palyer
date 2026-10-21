//! Данные приложения целиком: медиатека, плеер с плейлистом и настройки — и их сохранение
//! на диск. Здесь всё, что меняет данные; интерфейс только вызывает эти методы
//! (см. `ui::action`) и читает поля. Без egui — проверяется тестами без окна.

use std::{collections::HashSet, path::PathBuf};

use super::{
    library::{
        Library, Track,
        groups::{self, AlbumGroup, ArtistGroup},
    },
    playback::{PlayerController, PlaylistStore},
    settings::{Settings, SettingsStore},
};

pub struct AppState {
    /// Треки, сохранённые между запусками
    pub library: Library,
    /// Плеер и плейлист
    pub controller: PlayerController,
    /// Настройки. Громкость живёт в плеере, в настройки она копируется перед сохранением
    pub settings: Settings,
    /// Исполнители и альбомы медиатеки — пересчитываются (`refresh_groups`), только когда
    /// медиатека изменилась
    pub groups: LibraryGroups,
    playlist_store: PlaylistStore,
    /// Ревизия плейлиста, которая уже сохранена на диск
    saved_playlist_revision: u64,
    settings_store: SettingsStore,
}

#[derive(Default)]
pub struct LibraryGroups {
    /// Ревизия медиатеки, по которой посчитаны группы; `None` — ещё не считались
    pub revision: Option<u64>,
    pub artists: Vec<ArtistGroup>,
    pub albums: Vec<AlbumGroup>,
}

impl AppState {
    /// Загружает сохранённое в папке данных `dir`: медиатеку, настройки и плейлист прошлого
    /// запуска. `None` — папка недоступна: всё живёт только до выхода. `wake` вызывается
    /// из аудиопотока, когда трек доиграл (см. `PlayerController::new`).
    /// Вторым значением — первая из ошибок загрузки, для показа пользователю
    pub fn load(dir: Option<PathBuf>, wake: impl Fn() + Send + 'static) -> (Self, Option<String>) {
        let (library, mut error) = Library::load(dir.clone());
        let (settings_store, settings, settings_error) = SettingsStore::load(dir.clone());
        if let Some(e) = settings_error {
            error.get_or_insert(e);
        }
        let playlist_store = PlaylistStore::new(dir);
        let saved_playlist = playlist_store.load().unwrap_or_else(|e| {
            error.get_or_insert(e);
            None
        });

        let mut controller = PlayerController::new(wake);
        if let Some(list) = saved_playlist {
            controller.restore_track_list(list);
        }
        controller.set_volume(settings.volume());
        let state = Self {
            library,
            // Восстановленный плейлист и так лежит на диске
            saved_playlist_revision: controller.playlist_revision(),
            controller,
            settings,
            groups: LibraryGroups::default(),
            playlist_store,
            settings_store,
        };
        (state, error)
    }

    /// Пересчитывает исполнителей и альбомы, если медиатека изменилась
    pub fn refresh_groups(&mut self) {
        let revision = self.library.revision();
        if self.groups.revision != Some(revision) {
            let (artists, albums) = groups::group(self.library.tracks());
            self.groups = LibraryGroups {
                revision: Some(revision),
                artists,
                albums,
            };
        }
    }

    /// Треки медиатеки с номерами `indices` для воспроизведения. Треки, чьих файлов нет
    /// на диске, пропускаются: сыграть их всё равно не выйдет
    pub fn playable_tracks(&self, indices: &[usize]) -> Vec<Track> {
        indices
            .iter()
            .filter(|&&index| !self.library.is_missing(index))
            .filter_map(|&index| self.library.tracks().get(index).cloned())
            .collect()
    }

    /// Заменяет плейлист треками медиатеки `indices` и играет с первого
    pub fn play_library_tracks(&mut self, indices: &[usize]) -> Result<(), String> {
        let tracks = self.playable_tracks(indices);
        self.controller.play_all(tracks)
    }

    /// Трек медиатеки сразу играет. Если он уже есть в плейлисте, играет оттуда, иначе
    /// встаёт в конец — повторный запуск не плодит копии в плейлисте
    pub fn play_from_library(&mut self, index: usize) -> Result<(), String> {
        let Some(track) = self.library.tracks().get(index).cloned() else {
            return Ok(());
        };
        let playlist = &self.controller.player().track_list.tracks;
        let playlist_index = match playlist.iter().position(|t| t.path == track.path) {
            Some(existing) => existing,
            None => {
                self.controller.append_tracks([track]);
                self.controller.player().track_list.tracks.len() - 1
            }
        };
        self.controller.select(playlist_index)?;
        self.controller.play()
    }

    /// Треки медиатеки `indices` — в конец плейлиста
    pub fn add_library_tracks_to_playlist(&mut self, indices: &[usize]) -> Result<(), String> {
        let tracks = self.playable_tracks(indices);
        self.add_to_playlist(tracks)
    }

    /// Добавляет треки в конец плейлиста. Если ничего ещё не выбрано, текущим становится
    /// первый из добавленных; иначе текущий трек не трогаем
    pub fn add_to_playlist(&mut self, tracks: Vec<Track>) -> Result<(), String> {
        if tracks.is_empty() {
            return Ok(());
        }
        let first_new = self.controller.player().track_list.tracks.len();
        let nothing_selected = self.controller.player().current_track.is_none();
        self.controller.append_tracks(tracks);
        if nothing_selected {
            self.controller.select(first_new)?;
        }
        Ok(())
    }

    /// Удаляет из медиатеки треки файлов `paths`. Сами файлы и плейлист не трогает
    pub fn remove_from_library(&mut self, paths: &HashSet<PathBuf>) -> Result<(), String> {
        let indices = self
            .library
            .tracks()
            .iter()
            .enumerate()
            .filter(|(_, track)| paths.contains(&track.path))
            .map(|(index, _)| index)
            .collect();
        self.library.remove(&indices)
    }

    /// Пути треков медиатеки с номерами `indices` — чтобы спросить про их удаление:
    /// пока ждём ответа, номера могут измениться, а пути — нет
    pub fn track_paths(&self, indices: &[usize]) -> HashSet<PathBuf> {
        indices
            .iter()
            .filter_map(|&index| self.library.tracks().get(index))
            .map(|track| track.path.clone())
            .collect()
    }

    /// Сохраняет плейлист, если он изменился с прошлого сохранения. Сколько бы изменений
    /// ни было между вызовами (например, «Play all» и меняет треки, и выбирает текущий),
    /// файл пишется один раз
    pub fn save_playlist_if_changed(&mut self) -> Result<(), String> {
        let revision = self.controller.playlist_revision();
        if revision == self.saved_playlist_revision {
            return Ok(());
        }
        // Ревизия считается сохранённой и при ошибке: иначе запись повторялась бы при каждом
        // вызове. Следующее изменение попробует снова
        self.saved_playlist_revision = revision;
        self.playlist_store
            .save(&self.controller.player().track_list)
    }

    /// Сохраняет настройки, если они изменились, — вместе с текущей громкостью плеера
    pub fn save_settings_if_changed(&mut self) -> Result<(), String> {
        self.settings.update_volume(self.controller.volume());
        self.settings_store.save_if_changed(&self.settings)
    }
}

#[cfg(test)]
mod tests {
    use super::AppState;
    use crate::core::library::Track;
    use std::{fs, path::PathBuf};

    /// Состояние в новой пустой временной папке
    fn state() -> (AppState, PathBuf) {
        let dir = std::env::temp_dir().join(format!("rusty_player_state_{}", fastrand::u64(..)));
        let (state, error) = AppState::load(Some(dir.clone()), || {});
        assert!(error.is_none(), "{error:?}");
        (state, dir)
    }

    fn track(name: &str) -> Track {
        Track {
            path: PathBuf::from(format!("/music/{name}.mp3")),
            title: name.into(),
            artist: "Artist".into(),
            album_artist: String::new(),
            duration_secs: 60,
            album: "Album".into(),
            year: None,
            track_number: None,
            disc_number: None,
        }
    }

    fn playlist_titles(state: &AppState) -> Vec<String> {
        let list = &state.controller.player().track_list;
        list.tracks.iter().map(|t| t.title.clone()).collect()
    }

    fn current_title(state: &AppState) -> Option<String> {
        let player = state.controller.player();
        player.current_track.as_ref().map(|t| t.title.clone())
    }

    #[test]
    fn play_library_tracks_replaces_playlist() {
        let (mut state, dir) = state();
        state
            .library
            .add(vec![track("a"), track("b"), track("c")])
            .unwrap();
        state.add_library_tracks_to_playlist(&[0]).unwrap();

        // Файлов нет — сыграть не выйдет, но плейлист уже заменён и трек выбран
        let _ = state.play_library_tracks(&[2, 1]);
        assert_eq!(playlist_titles(&state), ["c", "b"]);
        assert_eq!(current_title(&state).as_deref(), Some("c"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn adding_to_empty_playlist_selects_first_track() {
        let (mut state, dir) = state();
        state.library.add(vec![track("a"), track("b")]).unwrap();
        state.add_library_tracks_to_playlist(&[1, 0]).unwrap();
        assert_eq!(playlist_titles(&state), ["b", "a"]);
        assert_eq!(current_title(&state).as_deref(), Some("b"));

        // Текущий трек уже есть — добавление его не меняет
        state.add_library_tracks_to_playlist(&[0]).unwrap();
        assert_eq!(playlist_titles(&state), ["b", "a", "a"]);
        assert_eq!(current_title(&state).as_deref(), Some("b"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn play_from_library_does_not_duplicate() {
        let (mut state, dir) = state();
        state.library.add(vec![track("a"), track("b")]).unwrap();
        let _ = state.play_from_library(1);
        let _ = state.play_from_library(0);
        let _ = state.play_from_library(1);
        assert_eq!(playlist_titles(&state), ["b", "a"]);
        assert_eq!(current_title(&state).as_deref(), Some("b"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn removing_from_library_keeps_playlist() {
        let (mut state, dir) = state();
        state.library.add(vec![track("a"), track("b")]).unwrap();
        state.add_library_tracks_to_playlist(&[0, 1]).unwrap();

        let paths = state.track_paths(&[0]);
        state.remove_from_library(&paths).unwrap();
        let titles: Vec<_> = state.library.tracks().iter().map(|t| &t.title).collect();
        assert_eq!(titles, ["b"]);
        assert_eq!(playlist_titles(&state), ["a", "b"]);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn playlist_and_settings_survive_restart() {
        let (mut state, dir) = state();
        state.library.add(vec![track("a"), track("b")]).unwrap();
        state.add_library_tracks_to_playlist(&[0, 1]).unwrap();
        state.controller.set_volume_level(0.3);
        state.save_playlist_if_changed().unwrap();
        state.save_settings_if_changed().unwrap();

        let (restored, _) = AppState::load(Some(dir.clone()), || {});
        assert_eq!(playlist_titles(&restored), ["a", "b"]);
        assert_eq!(current_title(&restored).as_deref(), Some("a"));
        assert_eq!(restored.library.tracks().len(), 2);
        assert_eq!(restored.controller.volume().level, 0.3);
        fs::remove_dir_all(dir).unwrap();
    }
}
