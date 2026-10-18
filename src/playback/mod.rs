//! Воспроизведение: плейлист, управление плеером, аудиопоток. Без egui.

mod audio;
mod player;
mod playlist;

pub use player::{PlayerController, PlayerState};
pub use playlist::RepeatMode;
