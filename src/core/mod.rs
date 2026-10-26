//! Логика плеера без интерфейса: медиатека, воспроизведение, настройки и их сохранение
//! на диск — и `Core`, который владеет ими всеми. Ничего не знает об egui: интерфейс
//! (`crate::ui`) только читает отсюда данные и вызывает методы. Проверяется тестами без окна.

pub mod artist_images;
pub mod library;
pub mod lyrics;
pub mod net;
pub mod playback;
pub mod playlists;
pub mod settings;
pub mod storage;

use std::{path::PathBuf, sync::Arc};

use self::{
    artist_images::{ArtistImage, ArtistImages, ArtistJob},
    library::{
        Library, Track,
        groups::{self, AlbumGroup, ArtistGroup},
    },
    lyrics::LyricsFinder,
    playback::{PlayerController, QueueStore},
    playlists::{PlaylistId, Playlists},
    settings::{Settings, SettingsStore},
};

/// Ядро приложения: владеет медиатекой, плеером с плейлистом и настройками, загружает
/// и сохраняет их. Каждая из частей знает только себя и меняется своими методами;
/// здесь — операции между частями (треки медиатеки → в плеер) и сохранение
pub struct Core {
    /// Треки, сохранённые между запусками
    pub library: Library,
    /// Сохранённые плейлисты
    pub playlists: Playlists,
    /// Плеер и очередь воспроизведения
    pub controller: PlayerController,
    /// Настройки. Громкость живёт в плеере, в настройки она копируется перед сохранением
    pub settings: Settings,
    /// Исполнители и альбомы медиатеки — пересчитываются (`refresh_groups`), только когда
    /// медиатека изменилась
    pub groups: LibraryGroups,
    /// Тексты песен: свои и из LRCLIB, с кешем ответов
    pub lyrics: LyricsFinder,
    /// Картинки исполнителей: свои и из Deezer, ищутся в фоне
    pub artist_images: ArtistImages,
    queue_store: QueueStore,
    /// Ревизия плейлиста, которая уже сохранена на диск
    saved_queue_revision: u64,
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
    /// из фоновых потоков: трек доиграл (см. `PlayerController::new`), пришёл текст песни.
    /// Вторым значением — первая из ошибок загрузки, для показа пользователю
    pub fn load(
        dir: Option<PathBuf>,
        wake: impl Fn() + Send + Sync + 'static,
    ) -> (Self, Option<String>) {
        let wake: Arc<dyn Fn() + Send + Sync> = Arc::new(wake);
        let (mut library, mut error) = Library::load(dir.clone());
        library.check_missing_in_background({
            let wake = Arc::clone(&wake);
            move || wake()
        });
        let (settings_store, settings, settings_error) = SettingsStore::load(dir.clone());
        let (playlists, playlists_error) = Playlists::load(dir.clone());
        for e in [settings_error, playlists_error].into_iter().flatten() {
            error.get_or_insert(e);
        }
        let mut lyrics = LyricsFinder::new(dir.clone(), Arc::clone(&wake));
        let mut artist_images = ArtistImages::new(dir.clone(), Arc::clone(&wake));
        lyrics.set_online(settings.online_lookups());
        artist_images.set_online(settings.online_lookups());
        let queue_store = QueueStore::new(dir);
        let saved_queue = queue_store.load().unwrap_or_else(|e| {
            error.get_or_insert(e);
            None
        });

        let mut controller = PlayerController::new(move || wake());
        if let Some(list) = saved_queue {
            controller.restore_track_list(list);
        }
        controller.set_volume(settings.volume());
        let state = Self {
            library,
            playlists,
            // Восстановленный плейлист и так лежит на диске
            saved_queue_revision: controller.queue_revision(),
            controller,
            settings,
            groups: LibraryGroups::default(),
            lyrics,
            artist_images,
            queue_store,
            settings_store,
        };
        (state, error)
    }

    /// Забирает результаты фоновой работы: какие файлы медиатеки пропали с диска, готовые
    /// картинки исполнителей. Вызывать
    /// каждый кадр. Возвращает ошибку фоновой записи, если была, — для показа пользователю
    pub fn poll_background(&mut self) -> Option<String> {
        self.library.poll_missing();
        self.artist_images.poll();
        self.library.take_save_error()
    }

    /// Ждёт фоновой записи медиатеки — перед выходом, чтобы последние изменения не потерялись
    pub fn flush(&self) {
        self.library.flush();
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
            // Исполнители изменились — картинки новых ищутся в фоне
            self.prefetch_artist_images();
        }
    }

    /// Включает или выключает поиск в интернете: фото исполнителей и тексты песен.
    /// После включения фоновый поиск продолжается с теми, кого не искали
    pub fn set_online_lookups(&mut self, online: bool) {
        self.settings.update_online_lookups(online);
        self.lyrics.set_online(online);
        self.artist_images.set_online(online);
        if online && self.groups.revision.is_some() {
            self.prefetch_artist_images();
        }
    }

    /// Ставит всех исполнителей медиатеки в фоновую очередь картинок — известных
    /// очередь пропустит
    fn prefetch_artist_images(&mut self) {
        let jobs: Vec<ArtistJob> = self
            .groups
            .artists
            .iter()
            .map(|artist| ArtistJob {
                name: artist.name.clone(),
                music_dirs: music_dirs(&self.library, &artist.tracks),
            })
            .collect();
        self.artist_images.prefetch(jobs);
    }

    /// Альбом трека в медиатеке — ищется так же, как при группировке: без учёта регистра.
    /// У трека без альбома или исполнителя в тегах альбома нет: он в «Unknown Album» вместе
    /// с чужими треками без тегов (см. `groups::has_known_album`)
    pub fn album_of(&mut self, track: &Track) -> Option<&AlbumGroup> {
        if !groups::has_known_album(track) {
            return None;
        }
        self.refresh_groups();
        let title = track.album.trim().to_lowercase();
        let artist = groups::album_artist(track).to_lowercase();
        self.groups.albums.iter().find(|album| {
            album.title.to_lowercase() == title && album.artist.to_lowercase() == artist
        })
    }

    /// Картинка исполнителя `name` для показа: готова, ищется или её нет. Не готова —
    /// исполнитель встаёт в начало фоновой очереди
    pub fn artist_image(&mut self, name: &str) -> ArtistImage {
        let (images, library, groups) = (&mut self.artist_images, &self.library, &self.groups);
        images.image(name, || {
            groups
                .artists
                .iter()
                .find(|artist| artist.name == name)
                .map(|artist| music_dirs(library, &artist.tracks))
                .unwrap_or_default()
        })
    }

    /// Заменяет плейлист треками медиатеки `indices` и играет с первого
    pub fn play_library_tracks(&mut self, indices: &[usize]) -> Result<(), String> {
        let tracks = self.library.playable_tracks(indices);
        self.controller.play_all(tracks)
    }

    /// Трек медиатеки сразу играет — см. `play_track`
    pub fn play_from_library(&mut self, index: usize) -> Result<(), String> {
        match self.library.tracks().get(index).cloned() {
            Some(track) => self.play_track(track),
            None => Ok(()),
        }
    }

    /// Трек сохранённого плейлиста `id` с номером `index` сразу играет — см. `play_track`
    pub fn play_from_playlist(&mut self, id: PlaylistId, index: usize) -> Result<(), String> {
        let track = self
            .playlists
            .get(id)
            .and_then(|playlist| playlist.tracks.get(index))
            .cloned();
        match track {
            Some(track) => self.play_track(track),
            None => Ok(()),
        }
    }

    /// Трек сразу играет. Если он уже есть в очереди, играет оттуда, иначе встаёт в конец —
    /// повторный запуск не плодит копии в очереди
    fn play_track(&mut self, track: Track) -> Result<(), String> {
        let queue = &self.controller.player().track_list.tracks;
        let queue_index = match queue.iter().position(|t| t.path == track.path) {
            Some(existing) => existing,
            None => {
                self.controller.append_tracks([track]);
                self.controller.player().track_list.tracks.len() - 1
            }
        };
        self.controller.select(queue_index)?;
        self.controller.play()
    }

    /// Заменяет очередь треками плейлиста `id` и играет с первого. Очередь связывается
    /// с плейлистом: поменяли её — «сохранить» запишет изменения в этот плейлист
    pub fn play_playlist(&mut self, id: PlaylistId) -> Result<(), String> {
        let tracks = self.playlists.playable_tracks(id);
        let result = self.controller.play_all(tracks);
        self.controller.link_playlist(Some(id));
        result
    }

    /// Плейлист, с которым связана очередь, — если он ещё существует
    pub fn queue_playlist(&self) -> Option<PlaylistId> {
        let id = self.controller.player().track_list.playlist?;
        self.playlists.get(id).map(|playlist| playlist.id)
    }

    /// Очередь связана с плейлистом и с последнего сохранения не менялась: в ней те же
    /// треки в том же порядке, что в плейлисте. Сохранять тогда нечего
    pub fn queue_matches_playlist(&self) -> bool {
        let Some(playlist) = self.queue_playlist().and_then(|id| self.playlists.get(id)) else {
            return false;
        };
        let queue = &self.controller.player().track_list.tracks;
        queue.len() == playlist.tracks.len()
            && queue
                .iter()
                .zip(&playlist.tracks)
                .all(|(a, b)| a.path == b.path)
    }

    /// Перезаписывает плейлист `id` треками очереди
    pub fn save_queue_to_playlist(&mut self, id: PlaylistId) -> Result<(), String> {
        let tracks = self.controller.player().track_list.tracks.clone();
        self.playlists.set_tracks(id, tracks)
    }

    /// Треки плейлиста `id` с номерами `indices` — в конец очереди (без пропавших файлов)
    pub fn add_playlist_tracks_to_queue(
        &mut self,
        id: PlaylistId,
        indices: &[usize],
    ) -> Result<(), String> {
        let Some(playlist) = self.playlists.get(id) else {
            return Ok(());
        };
        let tracks = indices
            .iter()
            .filter_map(|&index| playlist.tracks.get(index))
            .filter(|track| !self.playlists.is_missing(track))
            .cloned()
            .collect();
        self.controller.add_tracks(tracks)
    }

    /// Треки плейлиста `id` с номерами `indices` — в медиатеку. Те, что уже в ней, не
    /// задваиваются
    pub fn add_playlist_tracks_to_library(&mut self, id: PlaylistId, indices: &[usize]) {
        let Some(playlist) = self.playlists.get(id) else {
            return;
        };
        let tracks = indices
            .iter()
            .filter_map(|&index| playlist.tracks.get(index).cloned())
            .collect();
        self.library.add(tracks);
    }

    /// Треки очереди с номерами `indices` — в медиатеку. Те, что уже в ней, не задваиваются
    pub fn add_queue_tracks_to_library(&mut self, indices: &[usize]) {
        let queue = &self.controller.player().track_list.tracks;
        let tracks = indices
            .iter()
            .filter_map(|&index| queue.get(index).cloned())
            .collect();
        self.library.add(tracks);
    }

    /// Сохраняет очередь как новый плейлист `name` и связывает её с ним: следующее
    /// «сохранить» перезапишет этот плейлист, а не создаст ещё один
    pub fn save_queue_as_playlist(&mut self, name: &str) -> Result<PlaylistId, String> {
        let tracks = self.controller.player().track_list.tracks.clone();
        let id = self.playlists.create(name, tracks)?;
        self.controller.link_playlist(Some(id));
        Ok(id)
    }

    /// Треки медиатеки `indices` — в конец плейлиста `id`. Пропавшие файлы тоже: плейлист
    /// хранит то, что выбрали, а сыграть пропущенное не даст воспроизведение
    pub fn add_library_tracks_to_playlist(
        &mut self,
        id: PlaylistId,
        indices: &[usize],
    ) -> Result<(), String> {
        let tracks = self.library_tracks(indices);
        self.playlists.add_tracks(id, tracks)
    }

    /// Новый плейлист `name` из треков медиатеки `indices`
    pub fn create_playlist_from_library(
        &mut self,
        name: &str,
        indices: &[usize],
    ) -> Result<PlaylistId, String> {
        let tracks = self.library_tracks(indices);
        self.playlists.create(name, tracks)
    }

    fn library_tracks(&self, indices: &[usize]) -> Vec<Track> {
        indices
            .iter()
            .filter_map(|&index| self.library.tracks().get(index).cloned())
            .collect()
    }

    /// Треки медиатеки `indices` — в конец плейлиста
    pub fn add_library_tracks_to_queue(&mut self, indices: &[usize]) -> Result<(), String> {
        let tracks = self.library.playable_tracks(indices);
        self.controller.add_tracks(tracks)
    }

    /// Сохраняет плейлист, если он изменился с прошлого сохранения. Сколько бы изменений
    /// ни было между вызовами (например, «Play» и меняет треки, и выбирает текущий),
    /// файл пишется один раз
    pub fn save_queue_if_changed(&mut self) -> Result<(), String> {
        let revision = self.controller.queue_revision();
        if revision == self.saved_queue_revision {
            return Ok(());
        }
        // Ревизия считается сохранённой и при ошибке: иначе запись повторялась бы при каждом
        // вызове. Следующее изменение попробует снова
        self.saved_queue_revision = revision;
        self.queue_store.save(&self.controller.player().track_list)
    }

    /// Сохраняет настройки, если они изменились, — вместе с текущей громкостью плеера
    pub fn save_settings_if_changed(&mut self) -> Result<(), String> {
        self.settings.update_volume(self.controller.volume());
        self.settings_store.save_if_changed(&self.settings)
    }
}

/// Папки, где искать свою картинку исполнителя: папки его треков (альбомов) и папки
/// над ними (исполнителя). Хватает первых треков — остальные обычно в тех же папках
fn music_dirs(library: &Library, tracks: &[usize]) -> Vec<PathBuf> {
    const TRACKS_TO_LOOK_AT: usize = 20;
    let mut dirs: Vec<PathBuf> = Vec::new();
    for index in tracks.iter().take(TRACKS_TO_LOOK_AT) {
        let Some(album_dir) = library
            .tracks()
            .get(*index)
            .and_then(|track| track.path.parent())
        else {
            continue;
        };
        for dir in [Some(album_dir), album_dir.parent()].into_iter().flatten() {
            if !dirs.iter().any(|known| known == dir) {
                dirs.push(dir.to_path_buf());
            }
        }
    }
    dirs
}

#[cfg(test)]
mod tests {
    use super::Core;
    use crate::core::library::Track;
    use std::{fs, path::PathBuf};

    /// Состояние в новой пустой временной папке
    fn new_core() -> (Core, PathBuf) {
        let dir = std::env::temp_dir().join(format!("lmp_core_{}", fastrand::u64(..)));
        let (core, error) = Core::load(Some(dir.clone()), || {});
        assert!(error.is_none(), "{error:?}");
        (core, dir)
    }

    /// Конец теста: дожидается фоновой записи медиатеки (`Drop`) и убирает папку данных.
    /// Папки может и не быть — тест ничего не сохранил
    fn finish(core: Core, dir: PathBuf) {
        drop(core);
        let _ = fs::remove_dir_all(dir);
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
        core.library.add(vec![track("a"), track("b"), track("c")]);
        core.add_library_tracks_to_queue(&[0]).unwrap();

        // Файлов нет — сыграть не выйдет, но плейлист уже заменён и трек выбран
        let _ = core.play_library_tracks(&[2, 1]);
        assert_eq!(playlist_titles(&core), ["c", "b"]);
        assert_eq!(current_title(&core).as_deref(), Some("c"));
        finish(core, dir);
    }

    #[test]
    fn adding_to_empty_playlist_selects_first_track() {
        let (mut core, dir) = new_core();
        core.library.add(vec![track("a"), track("b")]);
        core.add_library_tracks_to_queue(&[1, 0]).unwrap();
        assert_eq!(playlist_titles(&core), ["b", "a"]);
        assert_eq!(current_title(&core).as_deref(), Some("b"));

        // Текущий трек уже есть — добавление его не меняет
        core.add_library_tracks_to_queue(&[0]).unwrap();
        assert_eq!(playlist_titles(&core), ["b", "a", "a"]);
        assert_eq!(current_title(&core).as_deref(), Some("b"));
        finish(core, dir);
    }

    #[test]
    fn play_from_library_does_not_duplicate() {
        let (mut core, dir) = new_core();
        core.library.add(vec![track("a"), track("b")]);
        let _ = core.play_from_library(1);
        let _ = core.play_from_library(0);
        let _ = core.play_from_library(1);
        assert_eq!(playlist_titles(&core), ["b", "a"]);
        assert_eq!(current_title(&core).as_deref(), Some("b"));
        finish(core, dir);
    }

    #[test]
    fn removing_from_library_keeps_playlist() {
        let (mut core, dir) = new_core();
        core.library.add(vec![track("a"), track("b")]);
        core.add_library_tracks_to_queue(&[0, 1]).unwrap();

        let paths = core.library.paths_of(&[0]);
        core.library.remove_paths(&paths);
        let titles: Vec<_> = core.library.tracks().iter().map(|t| &t.title).collect();
        assert_eq!(titles, ["b"]);
        assert_eq!(playlist_titles(&core), ["a", "b"]);
        finish(core, dir);
    }

    #[test]
    fn playlist_and_settings_survive_restart() {
        let (mut core, dir) = new_core();
        core.library.add(vec![track("a"), track("b")]);
        core.add_library_tracks_to_queue(&[0, 1]).unwrap();
        core.controller.set_volume_level(0.3);
        core.save_queue_if_changed().unwrap();
        core.save_settings_if_changed().unwrap();

        let (restored, _) = Core::load(Some(dir.clone()), || {});
        assert_eq!(playlist_titles(&restored), ["a", "b"]);
        assert_eq!(current_title(&restored).as_deref(), Some("a"));
        assert_eq!(restored.library.tracks().len(), 2);
        assert_eq!(restored.controller.volume().level, 0.3);
        finish(core, dir);
    }

    #[test]
    fn queue_saved_as_playlist_plays_again_after_clear() {
        let (mut core, dir) = new_core();
        core.library.add(vec![track("a"), track("b")]);
        core.add_library_tracks_to_queue(&[1, 0]).unwrap();
        let id = core.save_queue_as_playlist("Mix").unwrap();

        core.controller.clear().unwrap();
        assert!(playlist_titles(&core).is_empty());
        assert!(current_title(&core).is_none());

        let _ = core.play_playlist(id);
        assert_eq!(playlist_titles(&core), ["b", "a"]);
        assert_eq!(current_title(&core).as_deref(), Some("b"));

        // Трек из медиатеки — в плейлист, и плейлист переживает перезапуск
        core.add_library_tracks_to_playlist(id, &[0]).unwrap();
        let (restored, _) = Core::load(Some(dir.clone()), || {});
        let saved = restored.playlists.get(id).unwrap();
        assert_eq!(saved.name, "Mix");
        let titles: Vec<_> = saved.tracks.iter().map(|t| t.title.as_str()).collect();
        assert_eq!(titles, ["b", "a", "a"]);
        finish(core, dir);
    }

    #[test]
    fn saving_linked_queue_updates_its_playlist() {
        let (mut core, dir) = new_core();
        core.library.add(vec![track("a"), track("b"), track("c")]);
        core.add_library_tracks_to_queue(&[0]).unwrap();
        let id = core.save_queue_as_playlist("Mix").unwrap();
        assert_eq!(core.queue_playlist(), Some(id));
        // Только что сохранили — сохранять нечего
        assert!(core.queue_matches_playlist());

        // Дополнили очередь — сохранение перезаписывает тот же плейлист
        core.add_library_tracks_to_queue(&[1]).unwrap();
        assert!(!core.queue_matches_playlist());
        core.save_queue_to_playlist(id).unwrap();
        assert!(core.queue_matches_playlist());
        assert_eq!(core.playlists.all().len(), 1);
        let titles: Vec<_> = core
            .playlists
            .get(id)
            .unwrap()
            .tracks
            .iter()
            .map(|t| t.title.clone())
            .collect();
        assert_eq!(titles, ["a", "b"]);

        // Связь переживает перезапуск
        core.save_queue_if_changed().unwrap();
        let (mut restored, _) = Core::load(Some(dir.clone()), || {});
        assert_eq!(restored.queue_playlist(), Some(id));

        // Очередь заменили другим содержимым — связи больше нет; запустили плейлист — снова есть
        let _ = restored.play_library_tracks(&[2]);
        assert_eq!(restored.queue_playlist(), None);
        let _ = restored.play_playlist(id);
        assert_eq!(restored.queue_playlist(), Some(id));
        restored.controller.clear().unwrap();
        assert_eq!(restored.queue_playlist(), None);
        finish(core, dir);
    }

    #[test]
    fn playlist_tracks_outside_library_can_be_added_to_it() {
        let (mut core, dir) = new_core();
        core.library.add(vec![track("a")]);
        // «b» добавлен в очередь мимо медиатеки и сохранён в плейлист вместе с «a»
        core.add_library_tracks_to_queue(&[0]).unwrap();
        core.controller.add_tracks(vec![track("b")]).unwrap();
        let id = core.save_queue_as_playlist("Mix").unwrap();
        assert_eq!(core.library.tracks().len(), 1);

        core.add_playlist_tracks_to_library(id, &[0, 1]);
        let titles: Vec<_> = core
            .library
            .tracks()
            .iter()
            .map(|t| t.title.as_str())
            .collect();
        assert_eq!(titles, ["a", "b"]);
        finish(core, dir);
    }
}
