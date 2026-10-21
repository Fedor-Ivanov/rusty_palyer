//! Логика плеера без интерфейса: медиатека, воспроизведение, настройки и их сохранение
//! на диск — и `Core`, который владеет ими всеми. Ничего не знает об egui: интерфейс
//! (`crate::ui`) только читает отсюда данные и вызывает методы. Проверяется тестами без окна.

pub mod library;
pub mod playback;
pub mod settings;
pub mod storage;

use std::path::PathBuf;

use self::{
    library::{
        Library,
        groups::{self, AlbumGroup, ArtistGroup},
    },
    playback::{PlayerController, PlaylistStore},
    settings::{Settings, SettingsStore},
};

/// Ядро приложения: владеет медиатекой, плеером с плейлистом и настройками, загружает
/// и сохраняет их. Каждая из частей знает только себя и меняется своими методами;
/// здесь — операции между частями (треки медиатеки → в плеер) и сохранение
pub struct Core {
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

impl Core {
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

    /// Заменяет плейлист треками медиатеки `indices` и играет с первого
    pub fn play_library_tracks(&mut self, indices: &[usize]) -> Result<(), String> {
        let tracks = self.library.playable_tracks(indices);
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
        let tracks = self.library.playable_tracks(indices);
        self.controller.add_tracks(tracks)
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
    use super::Core;
    use crate::core::library::Track;
    use std::{fs, path::PathBuf};

    /// Состояние в новой пустой временной папке
    fn new_core() -> (Core, PathBuf) {
        let dir = std::env::temp_dir().join(format!("rusty_player_core_{}", fastrand::u64(..)));
        let (core, error) = Core::load(Some(dir.clone()), || {});
        assert!(error.is_none(), "{error:?}");
        (core, dir)
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

    fn playlist_titles(core: &Core) -> Vec<String> {
        let list = &core.controller.player().track_list;
        list.tracks.iter().map(|t| t.title.clone()).collect()
    }

    fn current_title(core: &Core) -> Option<String> {
        let player = core.controller.player();
        player.current_track.as_ref().map(|t| t.title.clone())
    }

    #[test]
    fn play_library_tracks_replaces_playlist() {
        let (mut core, dir) = new_core();
        core.library
            .add(vec![track("a"), track("b"), track("c")])
            .unwrap();
        core.add_library_tracks_to_playlist(&[0]).unwrap();

        // Файлов нет — сыграть не выйдет, но плейлист уже заменён и трек выбран
        let _ = core.play_library_tracks(&[2, 1]);
        assert_eq!(playlist_titles(&core), ["c", "b"]);
        assert_eq!(current_title(&core).as_deref(), Some("c"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn adding_to_empty_playlist_selects_first_track() {
        let (mut core, dir) = new_core();
        core.library.add(vec![track("a"), track("b")]).unwrap();
        core.add_library_tracks_to_playlist(&[1, 0]).unwrap();
        assert_eq!(playlist_titles(&core), ["b", "a"]);
        assert_eq!(current_title(&core).as_deref(), Some("b"));

        // Текущий трек уже есть — добавление его не меняет
        core.add_library_tracks_to_playlist(&[0]).unwrap();
        assert_eq!(playlist_titles(&core), ["b", "a", "a"]);
        assert_eq!(current_title(&core).as_deref(), Some("b"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn play_from_library_does_not_duplicate() {
        let (mut core, dir) = new_core();
        core.library.add(vec![track("a"), track("b")]).unwrap();
        let _ = core.play_from_library(1);
        let _ = core.play_from_library(0);
        let _ = core.play_from_library(1);
        assert_eq!(playlist_titles(&core), ["b", "a"]);
        assert_eq!(current_title(&core).as_deref(), Some("b"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn removing_from_library_keeps_playlist() {
        let (mut core, dir) = new_core();
        core.library.add(vec![track("a"), track("b")]).unwrap();
        core.add_library_tracks_to_playlist(&[0, 1]).unwrap();

        let paths = core.library.paths_of(&[0]);
        core.library.remove_paths(&paths).unwrap();
        let titles: Vec<_> = core.library.tracks().iter().map(|t| &t.title).collect();
        assert_eq!(titles, ["b"]);
        assert_eq!(playlist_titles(&core), ["a", "b"]);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn playlist_and_settings_survive_restart() {
        let (mut core, dir) = new_core();
        core.library.add(vec![track("a"), track("b")]).unwrap();
        core.add_library_tracks_to_playlist(&[0, 1]).unwrap();
        core.controller.set_volume_level(0.3);
        core.save_playlist_if_changed().unwrap();
        core.save_settings_if_changed().unwrap();

        let (restored, _) = Core::load(Some(dir.clone()), || {});
        assert_eq!(playlist_titles(&restored), ["a", "b"]);
        assert_eq!(current_title(&restored).as_deref(), Some("a"));
        assert_eq!(restored.library.tracks().len(), 2);
        assert_eq!(restored.controller.volume().level, 0.3);
        fs::remove_dir_all(dir).unwrap();
    }
}
