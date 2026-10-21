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
        settings::Language,
    },
    ui::{
        app::PlayerApp, dialogs::remove::GroupKind, i18n, loading::Destination, navigation::Route,
    },
};

pub(crate) enum Action {
    /// Заменить плейлист треками медиатеки (их номерами в медиатеке) и играть с первого
    PlayLibraryTracks(Vec<usize>),
    /// Сыграть трек медиатеки: если его нет в плейлисте, он встаёт в конец
    PlayFromLibrary(usize),
    /// Треки медиатеки — в конец плейлиста
    AddToPlaylist(Vec<usize>),
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
    /// Сыграть трек плейлиста с номером
    PlayPlaylistTrack(usize),
    /// Убрать из плейлиста треки с номерами
    RemoveFromPlaylist(BTreeSet<usize>),
    ToggleShuffle,
    ToggleRepeat(RepeatMode),

    // ---- плеер
    /// Играет — пауза, иначе — играть
    TogglePlay,
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

    fn apply(&mut self, action: Action) {
        match action {
            Action::PlayLibraryTracks(indices) => {
                // Номера треков плейлиста меняются целиком — старое выделение ни на что не указывает
                self.playlist_view.clear_selection();
                let result = self.state.play_library_tracks(&indices);
                self.set_result(result);
            }
            Action::PlayFromLibrary(index) => {
                let result = self.state.play_from_library(index);
                self.set_result(result);
            }
            Action::AddToPlaylist(indices) => {
                let result = self.state.add_library_tracks_to_playlist(&indices);
                self.set_result(result);
            }
            Action::RequestSongsRemoval(indices) => self.request_songs_removal(&indices),
            Action::RequestGroupRemoval {
                kind,
                names,
                tracks,
            } => self.request_group_removal(kind, names, &tracks),
            Action::RemoveFromLibrary(paths) => {
                let result = self.state.remove_from_library(&paths);
                self.set_result(result);
                // Номера треков медиатеки сдвинулись — выделение в списках ни на что не указывает
                self.library_view.clear_selection();
                self.album_tracks_view.clear_selection();
            }
            Action::PlayPlaylistTrack(index) => {
                let result = self
                    .state
                    .controller
                    .select(index)
                    .and_then(|()| self.state.controller.play());
                self.set_result(result);
            }
            Action::RemoveFromPlaylist(indices) => {
                let result = self.state.controller.remove_tracks(&indices);
                self.set_result(result);
                // Номера оставшихся треков сдвинулись
                self.playlist_view.clear_selection();
            }
            Action::ToggleShuffle => self.state.controller.toggle_shuffle(),
            Action::ToggleRepeat(mode) => self.state.controller.toggle_repeat(mode),

            Action::TogglePlay => {
                let result = if self.state.controller.player().state == PlayerState::Playing {
                    self.state.controller.pause()
                } else {
                    self.state.controller.play()
                };
                self.set_result(result);
            }
            Action::Previous => {
                let result = self.state.controller.skip_prev();
                self.set_result(result);
            }
            Action::Next => {
                let result = self.state.controller.skip_next();
                self.set_result(result);
            }
            Action::Seek(secs) => {
                let result = self.state.controller.seek((secs.max(0.0) * 1000.0) as u64);
                self.set_result(result);
            }
            Action::SetVolume(level) => self.state.controller.set_volume_level(level),
            Action::ToggleMute => self.state.controller.toggle_mute(),
            Action::TrackEnded => {
                let result = self.state.controller.play_next();
                self.set_result(result);
            }

            Action::AddFiles(destination) => self.add_files(destination),
            Action::AddFolder(destination) => self.add_folder(destination),
            Action::TracksLoaded {
                finished,
                destination,
            } => self.apply_loaded(finished, destination),
            Action::CancelLoading => self.cancel_loading(),

            Action::SetLanguage(language) => {
                self.state.settings.update_language(language);
                i18n::set_language(language);
            }

            Action::Navigate(route) => self.navigate(route),
        }
    }
}
