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

/// Режим повтора: включён может быть только один
#[derive(Serialize, Debug, Clone, Copy, PartialEq, Default)]
pub enum RepeatMode {
    #[default]
    Off,
    /// Текущий трек по кругу
    One,
    /// Весь список по кругу
    All,
}

#[derive(Serialize, Debug, Clone, Default)]
pub struct TrackList {
    pub tracks: VecDeque<Track>,
    pub current_index: Option<usize>,
    /// Случайный порядок воспроизведения
    pub shuffle: bool,
    pub repeat: RepeatMode,
    /// Порядок воспроизведения при `shuffle`: перестановка индексов `tracks`.
    /// Без `shuffle` треки идут по порядку, и этот список не используется
    #[serde(skip)]
    shuffle_order: Vec<usize>,
}

impl TrackList {
    /// Порядок, в котором играют треки: перемешанный или обычный
    fn play_order(&self) -> Vec<usize> {
        if self.shuffle {
            self.shuffle_order.clone()
        } else {
            (0..self.tracks.len()).collect()
        }
    }

    /// Трек, который идёт после текущего (`step` = 1) или перед ним (`step` = -1).
    /// На краю списка с `wrap` переходит на другой конец, без него — `None`.
    fn neighbor(&self, step: isize, wrap: bool) -> Option<usize> {
        let order = self.play_order();
        let len = order.len() as isize;
        if len == 0 {
            return None;
        }
        let Some(position) = self
            .current_index
            .and_then(|current| order.iter().position(|&index| index == current))
        else {
            return order.first().copied();
        };
        let target = position as isize + step;
        if (0..len).contains(&target) {
            Some(order[target as usize])
        } else if wrap {
            Some(order[target.rem_euclid(len) as usize])
        } else {
            None
        }
    }

    /// Перемешивает порядок заново. Текущий трек ставится первым, чтобы после
    /// включения shuffle сыграли все остальные, прежде чем он повторится
    fn reshuffle(&mut self) {
        let mut rest: Vec<usize> = (0..self.tracks.len())
            .filter(|&index| Some(index) != self.current_index)
            .collect();
        fastrand::shuffle(&mut rest);
        self.shuffle_order = self.current_index.into_iter().chain(rest).collect();
    }
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
        let list = &mut self.player.track_list;
        let first_new = list.tracks.len();
        list.tracks.extend(tracks);

        // Новые треки встают в случайном порядке в конец перемешанной очереди
        let mut added: Vec<usize> = (first_new..list.tracks.len()).collect();
        fastrand::shuffle(&mut added);
        list.shuffle_order.extend(added);
    }

    /// Включает или выключает случайный порядок
    pub fn toggle_shuffle(&mut self) {
        let list = &mut self.player.track_list;
        list.shuffle = !list.shuffle;
        if list.shuffle {
            list.reshuffle();
        }
    }

    /// Включает режим повтора; повторное нажатие на уже включённый — выключает
    pub fn toggle_repeat(&mut self, mode: RepeatMode) {
        let list = &mut self.player.track_list;
        list.repeat = if list.repeat == mode {
            RepeatMode::Off
        } else {
            mode
        };
    }

    /// Делает текущим трек из списка по его позиции.
    pub fn select(&mut self, index: usize) -> Result<(), String> {
        let track = self
            .player
            .track_list
            .tracks
            .get(index)
            .cloned()
            .ok_or("Трека с таким номером нет в списке")?;
        self.set_track(track)?;
        self.player.track_list.current_index = Some(index);
        Ok(())
    }

    /// Вызывается, когда трек доиграл. С повтором одного — запускает его снова,
    /// иначе — следующий по порядку (с учётом shuffle). Если список закончился и повтор
    /// всего списка выключен, делает текущим первый трек, но не запускает его.
    pub fn play_next(&mut self) -> Result<(), String> {
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
