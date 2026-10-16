use serde::Serialize;
use std::{
    collections::VecDeque,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    time::Duration,
};

use crate::{
    audio::{self, AudioCmd, AudioEvent},
    track::Track,
};

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
    pub track_list: VecDeque<Track>,
    /// Позиция `current_track` в `track_list`
    pub current_index: Option<usize>,
}

impl Default for Player {
    fn default() -> Self {
        Self {
            current_track: None,
            state: PlayerState::Stopped,
            track_list: VecDeque::new(),
            current_index: None,
        }
    }
}

/// Состояние плеера + связь с аудиопотоком. Аналог `AppState` и команд из Tauri-версии.
pub struct PlayerController {
    player: Player,
    audio: Sender<AudioCmd>,
    events: Receiver<AudioEvent>,
    position_ms: Arc<AtomicU64>,
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
        }
    }

    pub fn player(&self) -> &Player {
        &self.player
    }

    pub fn position_ms(&self) -> u64 {
        self.position_ms.load(Ordering::Relaxed)
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
    fn set_track(&mut self, track: Track) -> Result<(), String> {
        // Если что-то уже играло, останавливаем: выбран другой трек
        self.audio.send(AudioCmd::Stop).map_err(|e| e.to_string())?;

        self.position_ms.store(0, Ordering::Relaxed);

        self.player.current_track = Some(track);
        self.player.state = PlayerState::Stopped;

        Ok(())
    }

    /// Добавляет треки в конец списка. Текущий трек и воспроизведение не меняются.
    pub fn append_tracks(&mut self, tracks: impl IntoIterator<Item = Track>) {
        self.player.track_list.extend(tracks);
    }

    /// Делает текущим трек из списка по его позиции.
    pub fn select(&mut self, index: usize) -> Result<(), String> {
        let track = self
            .player
            .track_list
            .get(index)
            .cloned()
            .ok_or("Трека с таким номером нет в списке")?;
        self.set_track(track)?;
        self.player.current_index = Some(index);
        Ok(())
    }

    /// Запускает трек, следующий за текущим в списке. Если текущий — последний,
    /// делает текущим первый трек списка, но не запускает его.
    pub fn play_next(&mut self) -> Result<(), String> {
        if self.player.track_list.is_empty() {
            return Ok(());
        }
        let next = self.player.current_index.map_or(0, |index| index + 1);
        if next >= self.player.track_list.len() {
            return self.select(0);
        }
        self.select(next)?;
        self.play()
    }

    /// Кнопка «вперёд»: следующий трек списка, после последнего — первый.
    /// Если трек играл, новый тоже запускается.
    pub fn skip_next(&mut self) -> Result<(), String> {
        let len = self.player.track_list.len();
        if len == 0 {
            return Ok(());
        }
        let next = self
            .player
            .current_index
            .map_or(0, |index| (index + 1) % len);
        self.switch_to(next)
    }

    /// Кнопка «назад»: если трек играет дольше нескольких секунд, возвращает в его
    /// начало (как в большинстве плееров), иначе — предыдущий трек, перед первым — последний.
    pub fn skip_prev(&mut self) -> Result<(), String> {
        const RESTART_THRESHOLD_MS: u64 = 3000;

        let len = self.player.track_list.len();
        if len == 0 {
            return Ok(());
        }
        if self.player.current_index.is_some() && self.position_ms() > RESTART_THRESHOLD_MS {
            return self.seek(0);
        }
        let prev = self
            .player
            .current_index
            .map_or(0, |index| (index + len - 1) % len);
        self.switch_to(prev)
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
            .ok_or("Трек не выбран")?;

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
