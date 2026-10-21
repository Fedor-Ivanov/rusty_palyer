//! Системные медиа-кнопки: ⏯ ⏮ ⏭ на клавиатуре Mac, кнопки наушников, «Пункт управления»
//! на macOS и панель мультимедиа на Windows. Через то же место система показывает, что
//! играет: название, исполнитель, обложка, позиция.
//!
//! macOS отдаёт медиа-кнопки приложению, которое последним рассказало ей, что играет
//! (MPNowPlayingInfoCenter). Поэтому текущий трек сообщается всегда, а не только ради
//! красоты: без этого кнопки уйдут в Music.app.

use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig,
    SeekDirection,
};
use std::{
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver},
    time::{Duration, Instant},
};

use super::{action::Action, app::PlayerApp, covers::cover_bytes};
use crate::core::playback::PlayerState;

/// Насколько позиция может разойтись с той, что знает система, прежде чем её поправить.
/// Так замечаются перемотки — ползунком, кнопкой «назад» в начало трека, повтором трека
const POSITION_DRIFT: Duration = Duration::from_millis(1500);
/// Шаг перемотки кнопками ⏪ ⏩ (они есть на Windows)
const SEEK_STEP: Duration = Duration::from_secs(10);
/// Файл обложки во временной папке: система принимает обложку только ссылкой
const COVER_FILE: &str = "lmp-now-playing-cover";

pub(crate) struct SystemMedia {
    /// `None` — система не дала подключиться: приложение работает и без медиа-кнопок
    controls: Option<MediaControls>,
    /// Нажатия кнопок: приходят из системы, разбираются в `logic`
    events: Receiver<MediaControlEvent>,
    /// Что уже сообщено системе; `None` — ещё ничего
    shown: Option<Shown>,
}

struct Shown {
    track: Option<PathBuf>,
    state: PlayerState,
    /// Позиция в момент сообщения — `at`. Дальше система двигает её сама, пока играет
    position: Duration,
    at: Instant,
}

impl Shown {
    /// Где, по мнению системы, позиция сейчас
    fn expected_position(&self, now: Instant) -> Duration {
        if self.state == PlayerState::Playing {
            self.position + now.duration_since(self.at)
        } else {
            self.position
        }
    }
}

impl SystemMedia {
    /// Подключается к системным медиа-кнопкам. Нажатие будит приложение (`request_repaint`):
    /// кнопки работают, и когда окно скрыто
    pub(crate) fn new(cc: &eframe::CreationContext) -> Self {
        let (sender, events) = mpsc::channel();
        let ctx = cc.egui_ctx.clone();
        let controls = MediaControls::new(PlatformConfig {
            display_name: crate::APP_NAME,
            dbus_name: "lmp",
            hwnd: window_handle(cc),
        })
        .and_then(|mut controls| {
            controls.attach(move |event| {
                let _ = sender.send(event);
                ctx.request_repaint();
            })?;
            Ok(controls)
        });
        let controls = match controls {
            Ok(controls) => Some(controls),
            Err(e) => {
                eprintln!("Could not connect to the system media controls: {e}");
                None
            }
        };
        Self {
            controls,
            events,
            shown: None,
        }
    }
}

/// Окно для панели мультимедиа Windows: она привязывается к окну. На macOS не нужно
fn window_handle(cc: &eframe::CreationContext) -> Option<*mut std::ffi::c_void> {
    #[cfg(target_os = "windows")]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        match cc.window_handle().ok()?.as_raw() {
            RawWindowHandle::Win32(handle) => Some(handle.hwnd.get() as *mut std::ffi::c_void),
            _ => None,
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = cc;
        None
    }
}

impl PlayerApp {
    /// Нажатия системных медиа-кнопок — в действия. Вызывается в `logic`
    pub(crate) fn poll_system_media(&mut self) {
        while let Ok(event) = self.system_media.events.try_recv() {
            if let Some(action) = self.media_action(event) {
                self.dispatch(action);
            }
        }
    }

    fn media_action(&self, event: MediaControlEvent) -> Option<Action> {
        let seek_by = |direction, step: Duration| {
            let position = Duration::from_millis(self.core.controller.position_ms());
            let target = match direction {
                SeekDirection::Forward => position + step,
                SeekDirection::Backward => position.saturating_sub(step),
            };
            Action::Seek(target.as_secs_f32())
        };
        Some(match event {
            MediaControlEvent::Toggle => Action::TogglePlay,
            MediaControlEvent::Play => Action::Play,
            MediaControlEvent::Pause | MediaControlEvent::Stop => Action::Pause,
            MediaControlEvent::Next => Action::Next,
            MediaControlEvent::Previous => Action::Previous,
            MediaControlEvent::SetPosition(MediaPosition(position)) => {
                Action::Seek(position.as_secs_f32())
            }
            MediaControlEvent::Seek(direction) => seek_by(direction, SEEK_STEP),
            MediaControlEvent::SeekBy(direction, step) => seek_by(direction, step),
            MediaControlEvent::SetVolume(level) => Action::SetVolume(level as f32),
            MediaControlEvent::OpenUri(_) | MediaControlEvent::Raise | MediaControlEvent::Quit => {
                return None;
            }
        })
    }

    /// Сообщает системе, что играет, если это изменилось: другой трек — название и обложку,
    /// пауза, play или перемотка — состояние и позицию. Вызывается в `logic` после действий
    pub(crate) fn sync_system_media(&mut self) {
        if self.system_media.controls.is_none() {
            return;
        }
        let player = self.core.controller.player();
        let track = player.current_track.clone();
        let state = player.state;
        let position = Duration::from_millis(self.core.controller.position_ms());
        let now = Instant::now();
        let path = track.as_ref().map(|track| track.path.clone());

        let (track_changed, playback_changed) = match &self.system_media.shown {
            None => (true, true),
            Some(shown) => {
                let expected = shown.expected_position(now);
                (
                    shown.track != path,
                    shown.state != state || expected.abs_diff(position) > POSITION_DRIFT,
                )
            }
        };
        if !track_changed && !playback_changed {
            return;
        }

        // Обложку ищем, пока `self` свободен: кандидатам нужны группы библиотеки
        let cover = match &track {
            Some(track) if track_changed => {
                let candidates = self.cover_candidates(track);
                cover_bytes(&candidates).and_then(|bytes| write_cover_file(&bytes))
            }
            _ => None,
        };
        let Some(controls) = &mut self.system_media.controls else {
            return;
        };
        // Ошибки не показываем: без системной панели плеер работает как обычно
        if track_changed {
            let metadata = match &track {
                Some(track) => MediaMetadata {
                    title: Some(&track.title),
                    artist: Some(&track.artist),
                    album: Some(&track.album),
                    duration: Some(Duration::from_secs(track.duration_secs)),
                    cover_url: cover.as_deref(),
                },
                None => MediaMetadata::default(),
            };
            if let Err(e) = controls.set_metadata(metadata) {
                eprintln!("Could not update the system media controls: {e}");
            }
        }
        // Новые сведения о треке стирают позицию — её отправляем и после них
        let progress = Some(MediaPosition(position));
        let playback = match (&track, state) {
            (None, _) => MediaPlayback::Stopped,
            (Some(_), PlayerState::Playing) => MediaPlayback::Playing { progress },
            // Остановленный трек (например, после запуска) — для системы на паузе:
            // так он виден в «Пункте управления» и запускается кнопкой ⏯
            (Some(_), PlayerState::Paused | PlayerState::Stopped) => {
                MediaPlayback::Paused { progress }
            }
        };
        if let Err(e) = controls.set_playback(playback) {
            eprintln!("Could not update the system media controls: {e}");
        }
        self.system_media.shown = Some(Shown {
            track: path,
            state,
            position,
            at: now,
        });
    }
}

/// Кладёт обложку во временный файл и возвращает ссылку на него
fn write_cover_file(bytes: &[u8]) -> Option<String> {
    let path = std::env::temp_dir().join(COVER_FILE);
    std::fs::write(&path, bytes).ok()?;
    file_url(&path)
}

/// `file://`-ссылка на файл. На macOS ссылка разбирается как URL, поэтому путь с символами,
/// которые пришлось бы кодировать, не годится — обложки тогда не будет. Временная папка
/// macOS (`/var/folders/…`) таких символов не содержит. Windows берёт путь после
/// `file://` как есть
fn file_url(path: &Path) -> Option<String> {
    let path = path.to_str()?;
    let plain = |c: char| c.is_ascii_alphanumeric() || "/._-".contains(c);
    if cfg!(target_os = "macos") && !path.chars().all(plain) {
        return None;
    }
    Some(format!("file://{path}"))
}
