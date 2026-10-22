//! Действия пользователя с данными приложения — и единственное место, где они выполняются.
//!
//! Страницы и панели не меняют медиатеку, плеер и плейлист сами: они отправляют `Action`
//! (`dispatch`), а в конце кадра `apply_actions` выполняет накопленное по порядку. Так всё,
//! что умеет приложение, видно в одном перечислении, а каждое действие реализовано один
//! раз — откуда бы его ни вызвали: из меню, кнопкой или с клавиатуры.
//!
//! Состояние самого интерфейса (выделение, прокрутка, текст поиска) сюда не относится —
//! его по-прежнему меняют компоненты.

use eframe::egui;
use std::{
    collections::{BTreeSet, HashSet},
    path::PathBuf,
};

use crate::{
    core::{
        library::loader::Finished,
        playback::{PlayerState, RepeatMode},
        playlists::PlaylistId,
        settings::{Glass, Language},
    },
    ui::{
        app::PlayerApp,
        dialogs::{
            playlist::{NameDialog, NamePurpose},
            remove::GroupKind,
        },
        i18n,
        loading::Destination,
        navigation::Route,
    },
};

pub(crate) enum Action {
    /// Заменить очередь треками медиатеки (их номерами в медиатеке) и играть с первого
    PlayLibraryTracks(Vec<usize>),
    /// Сыграть трек медиатеки: если его нет в очереди, он встаёт в конец
    PlayFromLibrary(usize),
    /// Треки медиатеки — в конец очереди
    AddToQueue(Vec<usize>),
    /// Спросить, удалить ли треки медиатеки (см. `dialogs::remove`)
    RequestSongsRemoval(Vec<usize>),
    /// Спросить, удалить ли альбомы или исполнителей `names` — их треки медиатеки `tracks`
    RequestGroupRemoval {
        kind: GroupKind,
        names: Vec<String>,
        tracks: Vec<usize>,
    },
    /// Удалить из медиатеки треки этих файлов — подтверждено в диалоге. Сами файлы
    /// остаются на диске
    RemoveFromLibrary(HashSet<PathBuf>),
    /// Сыграть трек очереди с номером
    PlayQueueTrack(usize),
    /// Убрать из очереди треки с номерами
    RemoveFromQueue(BTreeSet<usize>),
    /// Очистить очередь: воспроизведение останавливается
    ClearQueue,
    /// Треки очереди с номерами — в медиатеку (те, что не из неё)
    AddQueueTracksToLibrary(Vec<usize>),
    ToggleShuffle,
    ToggleRepeat(RepeatMode),

    // ---- сохранённые плейлисты
    /// Открыть окно названия, чтобы сохранить очередь как плейлист
    RequestSaveQueue,
    SaveQueueAsPlaylist(String),
    /// Перезаписать связанный с очередью плейлист её треками
    SaveQueueToPlaylist(PlaylistId),
    /// Открыть окно названия для нового плейлиста из треков медиатеки
    RequestNewPlaylist(Vec<usize>),
    CreatePlaylist {
        name: String,
        tracks: Vec<usize>,
    },
    /// Треки медиатеки — в конец плейлиста
    AddToPlaylist {
        id: PlaylistId,
        tracks: Vec<usize>,
    },
    /// Заменить очередь треками плейлиста и играть
    PlayPlaylist(PlaylistId),
    /// Сыграть трек плейлиста: если его нет в очереди, он встаёт в конец
    PlayFromPlaylist {
        id: PlaylistId,
        index: usize,
    },
    /// Треки плейлиста с номерами — в конец очереди
    AddPlaylistTracksToQueue {
        id: PlaylistId,
        indices: Vec<usize>,
    },
    /// Треки плейлиста с номерами — в медиатеку (те, что не из неё)
    AddPlaylistTracksToLibrary {
        id: PlaylistId,
        indices: Vec<usize>,
    },
    /// Убрать из плейлиста треки с номерами
    RemoveFromPlaylist {
        id: PlaylistId,
        indices: BTreeSet<usize>,
    },
    RequestRenamePlaylist(PlaylistId),
    RenamePlaylist {
        id: PlaylistId,
        name: String,
    },
    /// Спросить, удалить ли плейлисты (см. `dialogs::playlist`)
    RequestPlaylistDeletion(Vec<PlaylistId>),
    DeletePlaylists(Vec<PlaylistId>),

    // ---- плеер
    /// Играет — пауза, иначе — играть
    TogglePlay,
    /// Играть, если не играет (системная кнопка «play», не переключатель)
    Play,
    Pause,
    Previous,
    Next,
    /// Перемотать текущий трек на позицию в секундах
    Seek(f32),
    /// Громкость 0.0..=1.0 — положение ползунка. Включает звук, если он был выключен
    SetVolume(f32),
    ToggleMute,
    /// Трек доиграл — событие аудиопотока, а не пользователя: следующий трек по порядку
    TrackEnded,

    // ---- добавление файлов
    /// Выбрать файлы в системном окне и добавить — в медиатеку или в плейлист
    AddFiles(Destination),
    /// Выбрать папку и добавить все аудиофайлы в ней
    AddFolder(Destination),
    /// Фоновое добавление закончилось: положить прочитанное по назначению
    TracksLoaded {
        finished: Finished,
        destination: Destination,
    },
    /// Остановить текущее добавление: уже прочитанное не добавляется
    CancelLoading,

    // ---- настройки
    /// Язык интерфейса — со следующего кадра весь интерфейс на нём
    SetLanguage(Language),
    /// Оттенок и прозрачность стекла окна
    SetGlass(Glass),

    /// Показать или спрятать текст песни на странице текущего трека
    ToggleLyrics,

    // ---- навигация
    /// Открыть страницу (см. `navigation`)
    Navigate(Route),
}

impl PlayerApp {
    /// Отправляет действие: выполнится в конце кадра
    pub(crate) fn dispatch(&mut self, action: Action) {
        self.actions.push(action);
    }

    /// Выполняет действия, накопленные за кадр. Вызывается в конце `ui` и в конце `logic`:
    /// при скрытом окне `ui` не вызывается, а события (трек доиграл, файлы добавились)
    /// случаются и тогда
    pub(crate) fn apply_actions(&mut self, ctx: &egui::Context) {
        if self.actions.is_empty() {
            return;
        }
        for action in std::mem::take(&mut self.actions) {
            self.apply(action);
        }
        // Кадр уже нарисован до действий — следующий покажет их результат сразу,
        // не дожидаясь движения мыши
        ctx.request_repaint();
    }

    /// Играть текущий трек. Трека нет — ничего не делает: пробел или системная кнопка
    /// при пустой очереди не повод для ошибки
    fn play(&mut self) -> Result<(), String> {
        if self.core.controller.player().current_track.is_none() {
            return Ok(());
        }
        self.core.controller.play()
    }

    fn apply(&mut self, action: Action) {
        match action {
            Action::PlayLibraryTracks(indices) => {
                // Номера треков плейлиста меняются целиком — старое выделение ни на что не указывает
                self.queue_view.clear_selection();
                let result = self.core.play_library_tracks(&indices);
                self.report_error(result);
            }
            Action::PlayFromLibrary(index) => {
                let result = self.core.play_from_library(index);
                self.report_error(result);
            }
            Action::AddToQueue(indices) => {
                let result = self.core.add_library_tracks_to_queue(&indices);
                self.report_error(result);
            }
            Action::RequestSongsRemoval(indices) => self.request_songs_removal(&indices),
            Action::RequestGroupRemoval {
                kind,
                names,
                tracks,
            } => self.request_group_removal(kind, names, &tracks),
            Action::RemoveFromLibrary(paths) => {
                self.core.library.remove_paths(&paths);
                // Номера треков медиатеки сдвинулись — выделение в списках ни на что не указывает
                self.library_view.clear_selection();
                self.album_tracks_view.clear_selection();
            }
            Action::PlayQueueTrack(index) => {
                let result = self
                    .core
                    .controller
                    .select(index)
                    .and_then(|()| self.core.controller.play());
                self.report_error(result);
            }
            Action::RemoveFromQueue(indices) => {
                let result = self.core.controller.remove_tracks(&indices);
                self.report_error(result);
                // Номера оставшихся треков сдвинулись
                self.queue_view.clear_selection();
            }
            Action::ClearQueue => {
                let result = self.core.controller.clear();
                self.report_error(result);
                self.queue_view.clear_selection();
            }

            Action::AddQueueTracksToLibrary(indices) => {
                self.core.add_queue_tracks_to_library(&indices);
            }

            Action::RequestSaveQueue => {
                self.name_dialog = Some(NameDialog::new(NamePurpose::SaveQueue, String::new()));
            }
            Action::SaveQueueAsPlaylist(name) => {
                let result = self.core.save_queue_as_playlist(&name).map(drop);
                self.report_error(result);
            }
            Action::SaveQueueToPlaylist(id) => {
                let result = self.core.save_queue_to_playlist(id);
                self.report_error(result);
            }
            Action::RequestNewPlaylist(tracks) => {
                self.name_dialog = Some(NameDialog::new(
                    NamePurpose::NewPlaylist(tracks),
                    String::new(),
                ));
            }
            Action::CreatePlaylist { name, tracks } => {
                let result = self
                    .core
                    .create_playlist_from_library(&name, &tracks)
                    .map(drop);
                self.report_error(result);
            }
            Action::AddToPlaylist { id, tracks } => {
                let result = self.core.add_library_tracks_to_playlist(id, &tracks);
                self.report_error(result);
            }
            Action::PlayPlaylist(id) => {
                self.queue_view.clear_selection();
                let result = self.core.play_playlist(id);
                self.report_error(result);
            }
            Action::PlayFromPlaylist { id, index } => {
                let result = self.core.play_from_playlist(id, index);
                self.report_error(result);
            }
            Action::AddPlaylistTracksToQueue { id, indices } => {
                let result = self.core.add_playlist_tracks_to_queue(id, &indices);
                self.report_error(result);
            }
            Action::AddPlaylistTracksToLibrary { id, indices } => {
                self.core.add_playlist_tracks_to_library(id, &indices);
            }
            Action::RemoveFromPlaylist { id, indices } => {
                let result = self.core.playlists.remove_tracks(id, &indices);
                self.report_error(result);
                // Номера оставшихся треков сдвинулись
                self.playlist_tracks_view.clear_selection();
            }
            Action::RequestRenamePlaylist(id) => {
                let name = self
                    .core
                    .playlists
                    .get(id)
                    .map(|playlist| playlist.name.clone())
                    .unwrap_or_default();
                self.name_dialog = Some(NameDialog::new(NamePurpose::Rename(id), name));
            }
            Action::RenamePlaylist { id, name } => {
                let result = self.core.playlists.rename(id, &name);
                self.report_error(result);
            }
            Action::RequestPlaylistDeletion(ids) => self.pending_playlist_deletion = ids,
            Action::DeletePlaylists(ids) => {
                let result = self.core.playlists.delete(&ids);
                self.report_error(result);
                // Была открыта страница удалённого — к списку плейлистов
                if let Route::Playlist(id) = self.route
                    && ids.contains(&id)
                {
                    self.navigate(Route::Playlists);
                }
            }

            Action::ToggleShuffle => self.core.controller.toggle_shuffle(),
            Action::ToggleRepeat(mode) => self.core.controller.toggle_repeat(mode),

            Action::TogglePlay => {
                let result = if self.core.controller.player().state == PlayerState::Playing {
                    self.core.controller.pause()
                } else {
                    self.play()
                };
                self.report_error(result);
            }
            Action::Play => {
                let result = self.play();
                self.report_error(result);
            }
            Action::Pause => {
                let result = self.core.controller.pause();
                self.report_error(result);
            }
            Action::Previous => {
                let result = self.core.controller.skip_prev();
                self.report_error(result);
            }
            Action::Next => {
                let result = self.core.controller.skip_next();
                self.report_error(result);
            }
            Action::Seek(secs) => {
                let result = self.core.controller.seek((secs.max(0.0) * 1000.0) as u64);
                self.report_error(result);
            }
            Action::SetVolume(level) => self.core.controller.set_volume_level(level),
            Action::ToggleMute => self.core.controller.toggle_mute(),
            Action::TrackEnded => {
                let result = self.core.controller.play_next();
                self.report_error(result);
            }

            Action::AddFiles(destination) => self.add_files(destination),
            Action::AddFolder(destination) => self.add_folder(destination),
            Action::TracksLoaded {
                finished,
                destination,
            } => self.apply_loaded(finished, destination),
            Action::CancelLoading => self.cancel_loading(),

            Action::SetLanguage(language) => {
                self.core.settings.update_language(language);
                i18n::set_language(language);
            }

            Action::SetGlass(glass) => self.core.settings.update_glass(glass),

            Action::ToggleLyrics => self.lyrics_open = !self.lyrics_open,

            Action::Navigate(route) => self.navigate(route),
        }
    }
}
