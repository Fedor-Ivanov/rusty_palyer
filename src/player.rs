use serde::Serialize;
use std::{
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
}

impl Default for Player {
    fn default() -> Self {
        Self {
            current_track: None,
            state: PlayerState::Stopped,
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
    pub fn set_track(&mut self, track: Track) -> Result<(), String> {
        // Если что-то уже играло, останавливаем: выбран другой трек
        self.audio.send(AudioCmd::Stop).map_err(|e| e.to_string())?;

        self.position_ms.store(0, Ordering::Relaxed);

        self.player.current_track = Some(track);
        self.player.state = PlayerState::Stopped;

        Ok(())
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
