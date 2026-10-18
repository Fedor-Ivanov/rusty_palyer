//! Управление воспроизведением: текущий трек, play/pause, переходы, связь с аудиопотоком.

use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    time::Duration,
};

use super::{
    audio::{self, AudioCmd, AudioEvent},
    playlist::{RepeatMode, TrackList},
};
use crate::library::Track;

#[derive(Serialize, Debug, Clone, Copy, PartialEq)]
pub enum PlayerState {
    Playing,
    Paused,
    Stopped,
}

#[derive(Serialize, Debug, Clone)]
pub struct Player {
    pub current_track: Option<Track>,
    pub state: PlayerState,
    pub track_list: TrackList,
}

impl Default for Player {
    fn default() -> Self {
        Self {
            current_track: None,
            state: PlayerState::Stopped,
            track_list: TrackList::default(),
        }
    }
}

/// Громкость: положение ползунка и mute. Запоминается между запусками
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub struct Volume {
    /// Положение ползунка: 0.0..=1.0
    pub level: f32,
    pub muted: bool,
}

impl Default for Volume {
    fn default() -> Self {
        Self {
            level: 1.0,
            muted: false,
        }
    }
}

impl Volume {
    /// Громкость, которую слышно: 0.0, если звук выключен или ползунок в нуле
    pub fn audible(self) -> f32 {
        if self.muted { 0.0 } else { self.level }
    }
}

/// Громкость, с которой включается звук, если ползунок стоял в нуле
const UNMUTE_LEVEL: f32 = 0.5;

/// Состояние плеера + связь с аудиопотоком. Аналог `AppState` и команд из Tauri-версии.
pub struct PlayerController {
    player: Player,
    audio: Sender<AudioCmd>,
    events: Receiver<AudioEvent>,
    position_ms: Arc<AtomicU64>,
    volume: Volume,
    /// Растёт при каждом изменении плейлиста (треки, текущий трек, shuffle, повтор):
    /// по нему видно, что плейлист пора сохранить
    playlist_revision: u64,
}

impl Default for PlayerController {
    fn default() -> Self {
        Self::new()
    }
}

impl PlayerController {
    pub fn new() -> Self {
        let position_ms = Arc::new(AtomicU64::new(0));
        let (audio, events) = audio::spawn_audio_thread(position_ms.clone());

        Self {
            player: Player::default(),
            audio,
            events,
            position_ms,
            volume: Volume::default(),
            playlist_revision: 0,
        }
    }

    pub fn player(&self) -> &Player {
        &self.player
    }

    pub fn position_ms(&self) -> u64 {
        self.position_ms.load(Ordering::Relaxed)
    }

    pub fn playlist_revision(&self) -> u64 {
        self.playlist_revision
    }

    pub fn volume(&self) -> Volume {
        self.volume
    }

    /// Ставит громкость целиком — например, сохранённую с прошлого запуска
    pub fn set_volume(&mut self, volume: Volume) {
        self.volume = Volume {
            level: volume.level.clamp(0.0, 1.0),
            muted: volume.muted,
        };
        // Слух воспринимает громкость примерно логарифмически: с квадратом ползунка
        // середина звучит как середина, а не почти как максимум
        let gain = self.volume.audible().powi(2);
        // Аудиопоток живёт, пока жив контроллер: отправка не может не дойти
        let _ = self.audio.send(AudioCmd::SetVolume(gain));
    }

    /// Ползунок громкости: двигать его — значит и включить звук, если он был выключен
    pub fn set_volume_level(&mut self, level: f32) {
        self.set_volume(Volume {
            level,
            muted: false,
        });
    }

    /// Кнопка mute. Включает звук, если он не слышен — выключен или ползунок в нуле
    /// (тогда ползунок встаёт на середину); иначе выключает, запоминая положение ползунка
    pub fn toggle_mute(&mut self) {
        let volume = self.volume;
        self.set_volume(if volume.audible() == 0.0 {
            Volume {
                level: if volume.level == 0.0 {
                    UNMUTE_LEVEL
                } else {
                    volume.level
                },
                muted: false,
            }
        } else {
            Volume {
                muted: true,
                ..volume
            }
        });
    }

    /// Обрабатывает события аудиопотока. Вызывать в начале каждого кадра.
    /// Возвращает `true`, если трек доиграл до конца.
    pub fn poll_events(&mut self) -> bool {
        let mut ended = false;

        while let Ok(event) = self.events.try_recv() {
            match event {
                AudioEvent::TrackEnded => {
                    self.player.state = PlayerState::Stopped;
                    ended = true;
                }
            }
        }

        ended
    }

    pub fn seek(&self, position_ms: u64) -> Result<(), String> {
        // Сразу, чтобы UI не показал старую позицию, пока аудиопоток не обработал команду
        self.position_ms.store(position_ms, Ordering::Relaxed);

        self.audio
            .send(AudioCmd::Seek(Duration::from_millis(position_ms)))
            .map_err(|e| e.to_string())
    }

    /// Делает трек текущим. Воспроизведение не начинается — для этого есть `play`.
    fn set_current_track(&mut self, track: Track) -> Result<(), String> {
        // Если что-то уже играло, останавливаем: выбран другой трек
        self.audio.send(AudioCmd::Stop).map_err(|e| e.to_string())?;

        self.position_ms.store(0, Ordering::Relaxed);

        self.player.current_track = Some(track);
        self.player.state = PlayerState::Stopped;

        Ok(())
    }

    /// Восстанавливает плейлист прошлого запуска: те же треки, режимы shuffle и повтора.
    /// Текущий трек выбран, но не играет — как будто его только что выбрали
    pub fn restore_track_list(&mut self, mut list: TrackList) {
        list.current_index = list
            .current_index
            .filter(|&index| index < list.tracks.len());
        self.player.current_track = list.current_index.map(|index| list.tracks[index].clone());
        self.player.state = PlayerState::Stopped;
        // Перемешанный порядок не сохраняется — перемешиваем заново
        if list.shuffle {
            list.reshuffle();
        }
        self.player.track_list = list;
    }

    /// Добавляет треки в конец списка. Текущий трек и воспроизведение не меняются.
    pub fn append_tracks(&mut self, tracks: impl IntoIterator<Item = Track>) {
        let list = &mut self.player.track_list;
        let first_new = list.tracks.len();
        list.tracks.extend(tracks);

        // Новые треки встают в случайном порядке в конец перемешанной очереди
        let mut added: Vec<usize> = (first_new..list.tracks.len()).collect();
        fastrand::shuffle(&mut added);
        list.shuffle_order.extend(added);
        self.playlist_revision += 1;
    }

    /// Заменяет весь плейлист треками `tracks` и запускает воспроизведение с первого из них,
    /// а при включённом shuffle — со случайного
    pub fn play_all(&mut self, tracks: Vec<Track>) -> Result<(), String> {
        let list = &mut self.player.track_list;
        list.tracks = tracks;
        list.current_index = None;
        list.reshuffle();
        self.playlist_revision += 1;
        // Без текущего трека «следующий» — первый в порядке воспроизведения
        let Some(first) = list.neighbor(1, true) else {
            self.player.current_track = None;
            self.player.state = PlayerState::Stopped;
            self.position_ms.store(0, Ordering::Relaxed);
            return self.audio.send(AudioCmd::Stop).map_err(|e| e.to_string());
        };
        self.select(first)?;
        self.play()
    }

    /// Удаляет треки из списка. Если среди них текущий, воспроизведение останавливается
    /// и текущего трека больше нет — плеер как до выбора трека
    pub fn remove_tracks(&mut self, indices: &BTreeSet<usize>) -> Result<(), String> {
        let removes_current = self
            .player
            .track_list
            .current_index
            .is_some_and(|current| indices.contains(&current));
        self.player.track_list.remove(indices);
        self.playlist_revision += 1;

        if removes_current {
            self.player.current_track = None;
            self.player.state = PlayerState::Stopped;
            self.position_ms.store(0, Ordering::Relaxed);
            self.audio.send(AudioCmd::Stop).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    /// Включает или выключает случайный порядок
    pub fn toggle_shuffle(&mut self) {
        let list = &mut self.player.track_list;
        list.shuffle = !list.shuffle;
        if list.shuffle {
            list.reshuffle();
        }
        self.playlist_revision += 1;
    }

    /// Включает режим повтора; повторное нажатие на уже включённый — выключает
    pub fn toggle_repeat(&mut self, mode: RepeatMode) {
        let list = &mut self.player.track_list;
        list.repeat = if list.repeat == mode {
            RepeatMode::Off
        } else {
            mode
        };
        self.playlist_revision += 1;
    }

    /// Делает текущим трек из списка по его позиции.
    pub fn select(&mut self, index: usize) -> Result<(), String> {
        let track = self
            .player
            .track_list
            .tracks
            .get(index)
            .cloned()
            .ok_or("There is no track with this number in the list")?;
        self.set_current_track(track)?;
        self.player.track_list.current_index = Some(index);
        self.playlist_revision += 1;
        Ok(())
    }

    /// Вызывается, когда трек доиграл. С повтором одного — запускает его снова,
    /// иначе — следующий по порядку (с учётом shuffle). Если список закончился и повтор
    /// всего списка выключен, делает текущим первый трек, но не запускает его.
    pub fn play_next(&mut self) -> Result<(), String> {
        // Текущий трек удалили из списка: «доиграл» мог прийти уже после остановки —
        // само ничего не запускаем
        if self.player.current_track.is_none() {
            return Ok(());
        }
        let list = &self.player.track_list;
        if list.repeat == RepeatMode::One
            && let Some(current) = list.current_index
        {
            self.select(current)?;
            return self.play();
        }

        match list.neighbor(1, list.repeat == RepeatMode::All) {
            Some(next) => {
                self.select(next)?;
                self.play()
            }
            None => match list.play_order().first() {
                Some(&first) => self.select(first),
                None => Ok(()),
            },
        }
    }

    /// Кнопка «вперёд»: следующий трек (с учётом shuffle), после последнего — первый.
    /// Если трек играл, новый тоже запускается.
    pub fn skip_next(&mut self) -> Result<(), String> {
        match self.player.track_list.neighbor(1, true) {
            Some(next) => self.switch_to(next),
            None => Ok(()),
        }
    }

    /// Кнопка «назад»: если трек играет дольше нескольких секунд, возвращает в его
    /// начало (как в большинстве плееров), иначе — предыдущий трек, перед первым — последний.
    pub fn skip_prev(&mut self) -> Result<(), String> {
        const RESTART_THRESHOLD_MS: u64 = 3000;

        if self.player.track_list.current_index.is_some()
            && self.position_ms() > RESTART_THRESHOLD_MS
        {
            return self.seek(0);
        }
        match self.player.track_list.neighbor(-1, true) {
            Some(prev) => self.switch_to(prev),
            None => Ok(()),
        }
    }

    /// Переключает на трек, сохраняя состояние: играло — играет и новый
    fn switch_to(&mut self, index: usize) -> Result<(), String> {
        let was_playing = self.player.state == PlayerState::Playing;
        self.select(index)?;
        if was_playing { self.play() } else { Ok(()) }
    }

    pub fn play(&mut self) -> Result<(), String> {
        match self.player.state {
            PlayerState::Playing => return Ok(()),

            PlayerState::Paused => {
                self.audio
                    .send(AudioCmd::Resume)
                    .map_err(|e| e.to_string())?;

                self.player.state = PlayerState::Playing;

                return Ok(());
            }

            PlayerState::Stopped => {}
        }

        let path = self
            .player
            .current_track
            .as_ref()
            .map(|t| t.path.clone())
            .ok_or("No track selected")?;

        let start_at = Duration::from_millis(self.position_ms());

        let (reply_tx, reply_rx) = mpsc::channel();

        self.audio
            .send(AudioCmd::Play {
                path,
                start_at,
                reply: reply_tx,
            })
            .map_err(|e| e.to_string())?;

        reply_rx.recv().map_err(|e| e.to_string())??;

        // `TrackEnded` от предыдущего трека, пришедший до запуска нового, уже неактуален
        while self.events.try_recv().is_ok() {}

        self.player.state = PlayerState::Playing;

        Ok(())
    }

    pub fn pause(&mut self) -> Result<(), String> {
        if self.player.state != PlayerState::Playing {
            return Ok(());
        }

        self.audio
            .send(AudioCmd::Pause)
            .map_err(|e| e.to_string())?;

        self.player.state = PlayerState::Paused;

        Ok(())
    }
}
