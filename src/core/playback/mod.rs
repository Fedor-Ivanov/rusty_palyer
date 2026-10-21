//! Воспроизведение: плейлист, управление плеером, аудиопоток. Без egui.

mod audio;
mod player;
mod playlist;
mod saved_playlist;

pub use player::{PlayerController, PlayerState, Volume};
pub use playlist::RepeatMode;
pub use saved_playlist::PlaylistStore;
